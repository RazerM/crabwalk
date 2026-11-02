use std::any::Any;
use std::io;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use pyo3::Python;

use crate::callback::CallbackError;

/// How many results to read from the walker each time the GIL is released.
const BATCH_SIZE: usize = 128;

/// How many results the producer thread sends at once. Larger batches mean
/// fewer thread wakeups.
const THREAD_BATCH_SIZE: usize = 1024;

/// How many results to read inline before moving a sequential walk to a
/// producer thread, so that small walks don't pay for starting a thread.
const BACKGROUND_AFTER: usize = 1024;

/// How many batches the producer thread can get ahead of the consumer.
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

    /// Stop any producer threads and wait for them to finish.
    pub fn close(self, py: Python<'_>) {
        if let Source::Threads { receiver, handle } = self.source {
            // Producers stop once they can't send.
            drop(receiver);
            // Detach in case producers are waiting to call Python callbacks.
            let _ = py.detach(|| handle.join());
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
