use std::any::Any;
use std::io;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::process;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use ignore::{ParallelVisitor, ParallelVisitorBuilder, WalkParallel, WalkState};
use pyo3::Python;

use crate::callback::{CallbackError, release_thread_state};

/// How many results to read from the walker each time the GIL is released.
const BATCH_SIZE: usize = 128;

/// How many results producer threads send at once. Larger batches mean fewer
/// thread wakeups.
const THREAD_BATCH_SIZE: usize = 1024;

/// How many results to read inline before moving a sequential walk to a
/// producer thread, so that small walks don't pay for starting a thread.
const BACKGROUND_AFTER: usize = 1024;

/// How many batches producer threads can get ahead of the consumer.
const QUEUE_BATCHES: usize = 16;

pub type WalkResult = Result<ignore::DirEntry, ignore::Error>;
pub type Panic = Box<dyn Any + Send>;

/// Results from the walker, read a batch at a time so that `Walk.__next__`
/// only detaches from the interpreter once per batch.
pub struct Entries {
    batch: std::vec::IntoIter<WalkResult>,
    source: Source,
}

enum Source {
    /// Walk in the consumer's thread. If `background` is true, the walk moves
    /// to a producer thread after `BACKGROUND_AFTER` results.
    Inline {
        walk: Box<ignore::Walk>,
        background: bool,
        read: usize,
    },
    /// Producer threads send batches through a channel.
    Threads {
        // Receiver isn't Sync, which pyclasses must be. It's only used through
        // &mut, so the mutex is never locked.
        receiver: Mutex<Receiver<Vec<WalkResult>>>,
        handle: JoinHandle<()>,
        /// The process the threads were started in.
        pid: u32,
    },
    Finished,
}

impl Entries {
    /// Walk sequentially. With `background`, larger walks continue in another
    /// thread, so that the walk overlaps with the consumer. Only use that without Python callbacks, which would compete
    /// with the consumer for the GIL.
    pub fn sequential(walk: ignore::Walk, background: bool) -> Self {
        Self {
            batch: Vec::new().into_iter(),
            source: Source::Inline {
                walk: Box::new(walk),
                background,
                read: 0,
            },
        }
    }

    /// Walk using the `ignore` crate's parallel walker. Entries arrive in an
    /// arbitrary order.
    pub fn parallel(walk: WalkParallel) -> io::Result<Self> {
        Ok(Self {
            batch: Vec::new().into_iter(),
            source: Source::spawn(move |sender| walk.visit(&mut SenderBuilder(sender)))?,
        })
    }

    pub fn pop(&mut self) -> Option<WalkResult> {
        self.batch.next()
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.source, Source::Finished) && self.batch.len() == 0
    }

    /// Read the next batch of results, returning the payload if the walker
    /// panicked.
    ///
    /// Call this while detached from the interpreter. Python callbacks attach
    /// to it themselves.
    pub fn fill(&mut self, callback_error: &CallbackError) -> Result<(), Panic> {
        match &mut self.source {
            Source::Inline {
                walk,
                background,
                read,
            } => {
                let mut batch = Vec::with_capacity(BATCH_SIZE);
                let finished = panic::catch_unwind(AssertUnwindSafe(|| {
                    fill_batch(walk, &mut batch, BATCH_SIZE, || callback_error.is_set())
                }))?;
                *read += batch.len();
                self.batch = batch.into_iter();
                if finished {
                    self.source = Source::Finished;
                } else if *background && *read >= BACKGROUND_AFTER {
                    let Source::Inline { walk, .. } =
                        mem::replace(&mut self.source, Source::Finished)
                    else {
                        unreachable!()
                    };
                    // Carry on inline if the thread can't be spawned.
                    let (tx, rx) = mpsc::sync_channel::<Box<ignore::Walk>>(1);
                    let spawned = Source::spawn(move |sender| {
                        let Ok(mut walk) = rx.recv() else { return };
                        loop {
                            let mut batch = Vec::with_capacity(THREAD_BATCH_SIZE);
                            let finished =
                                fill_batch(&mut walk, &mut batch, THREAD_BATCH_SIZE, || false);
                            if (!batch.is_empty() && sender.send(batch).is_err()) || finished {
                                return;
                            }
                        }
                    });
                    self.source = match spawned {
                        Ok(source) => {
                            let _ = tx.send(walk);
                            source
                        }
                        Err(_) => Source::Inline {
                            walk,
                            background: false,
                            read: 0,
                        },
                    };
                }
            }
            Source::Threads { receiver, .. } => match receiver
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner)
                .recv()
            {
                Ok(batch) => self.batch = batch.into_iter(),
                // All producers are done.
                Err(_) => {
                    if let Source::Threads { handle, .. } =
                        mem::replace(&mut self.source, Source::Finished)
                    {
                        handle.join()?;
                    }
                }
            },
            Source::Finished => {}
        }
        Ok(())
    }

    /// Whether this process was forked after producer threads started, so
    /// they don't exist here. The channel and thread handle then refer to
    /// threads in the parent process and must not be used, which would hang
    /// or crash.
    pub fn is_forked(&self) -> bool {
        matches!(self.source, Source::Threads { pid, .. } if pid != process::id())
    }

    /// Stop any producer threads and wait for them to finish.
    pub fn close(mut self, py: Python<'_>) {
        if self.is_forked() {
            // Leave it to Drop.
            return;
        }
        if let Source::Threads {
            receiver, handle, ..
        } = mem::replace(&mut self.source, Source::Finished)
        {
            // Producers stop once they can't send.
            drop(receiver);
            // Detach in case producers are waiting to call Python callbacks.
            let _ = py.detach(|| handle.join());
        }
    }
}

impl Drop for Entries {
    fn drop(&mut self) {
        if self.is_forked() {
            // Leak the channel and thread handle, see is_forked.
            mem::forget(mem::replace(&mut self.source, Source::Finished));
        }
    }
}

impl Source {
    /// Start a producer thread which sends batches of results.
    fn spawn<F>(produce: F) -> io::Result<Self>
    where
        F: FnOnce(SyncSender<Vec<WalkResult>>) + Send + 'static,
    {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_BATCHES);
        let handle = thread::Builder::new()
            .name("crabwalk".to_owned())
            .spawn(move || produce(sender))?;
        Ok(Source::Threads {
            receiver: Mutex::new(receiver),
            handle,
            pid: process::id(),
        })
    }
}

/// Fill `batch` from `walk` up to `size` results, returning true if the walk
/// is finished.
fn fill_batch(
    walk: &mut ignore::Walk,
    batch: &mut Vec<WalkResult>,
    size: usize,
    stop: impl Fn() -> bool,
) -> bool {
    while batch.len() < size {
        let Some(result) = walk.next() else {
            return true;
        };
        if stop() {
            // A callback raised an exception. Stop so that it's raised after
            // the entries before it. The entry being processed when the
            // callback failed is skipped.
            return false;
        }
        batch.push(result);
    }
    false
}

/// Sends batches of results from each of the parallel walker's threads.
struct Sender {
    sender: SyncSender<Vec<WalkResult>>,
    batch: Vec<WalkResult>,
}

impl Sender {
    fn flush(&mut self) -> bool {
        if self.batch.is_empty() {
            return true;
        }
        let batch = mem::replace(&mut self.batch, Vec::with_capacity(THREAD_BATCH_SIZE));
        self.sender.send(batch).is_ok()
    }
}

impl ParallelVisitor for Sender {
    fn visit(&mut self, result: WalkResult) -> WalkState {
        self.batch.push(result);
        if self.batch.len() < THREAD_BATCH_SIZE || self.flush() {
            WalkState::Continue
        } else {
            // The consumer has gone away.
            WalkState::Quit
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.flush();
        // Each of the walker's threads drops its visitor when it finishes.
        release_thread_state();
    }
}

struct SenderBuilder(SyncSender<Vec<WalkResult>>);

impl<'s> ParallelVisitorBuilder<'s> for SenderBuilder {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
        Box::new(Sender {
            sender: self.0.clone(),
            batch: Vec::with_capacity(THREAD_BATCH_SIZE),
        })
    }
}
