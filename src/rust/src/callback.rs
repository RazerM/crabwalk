use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pyo3::prelude::*;
use pyo3::{PyTraverseError, PyVisit, ffi};

use crate::util::{lock, try_lock};

/// The first exception raised by a Python callback (a sort key or
/// `filter_entry`) while the `ignore` crate is walking.
///
/// The callbacks can't return errors to the `ignore` crate, so they store them
/// here and `Walk.__next__` raises them. Once set, callbacks shouldn't call
/// Python again until the error has been taken.
#[derive(Clone, Default)]
pub struct CallbackError(Arc<Inner>);

#[derive(Default)]
struct Inner {
    // Checked for every entry, so avoid locking the mutex for that.
    is_set: AtomicBool,
    err: Mutex<Option<PyErr>>,
}

impl CallbackError {
    pub fn is_set(&self) -> bool {
        self.0.is_set.load(Ordering::Acquire)
    }

    pub fn set(&self, err: PyErr) {
        let mut slot = self.lock();
        if slot.is_none() {
            *slot = Some(err);
            self.0.is_set.store(true, Ordering::Release);
        }
    }

    pub fn take(&self) -> Option<PyErr> {
        if !self.is_set() {
            return None;
        }
        let mut slot = self.lock();
        // Reset the flag while locked, so that it can't hide an exception
        // set by another thread in between.
        self.0.is_set.store(false, Ordering::Release);
        slot.take()
    }

    fn lock(&self) -> MutexGuard<'_, Option<PyErr>> {
        // Python is never called while this is locked, so poisoning would only
        // happen if a PyErr panicked on drop.
        self.0.err.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Prepare the current thread for calling Python callbacks.
///
/// `Python::attach` creates and destroys a thread state each time it's
/// called from a thread that Python didn't create, like the parallel walker's
/// threads. That's very slow, so keep one thread state until
/// `release_thread_state`.
pub fn keep_thread_state() {
    THREAD_STATE.with(|state| {
        let mut state = state.borrow_mut();
        // SAFETY: always safe to call.
        if state.is_none() && unsafe { ffi::PyGILState_GetThisThreadState() }.is_null() {
            *state = Some(ThreadState::new());
        }
    });
}

/// Destroy the thread state kept by `keep_thread_state`, if any.
///
/// Call this before the thread exits. Thread-local destructors are too late on
/// musl, which clears CPython's thread-local storage first, so releasing the
/// thread state there aborts.
pub fn release_thread_state() {
    let state = THREAD_STATE.with(|state| state.borrow_mut().take());
    drop(state);
}

thread_local! {
    static THREAD_STATE: RefCell<Option<ThreadState>> = const { RefCell::new(None) };
}

/// A thread state for this thread, which is detached except while a callback
/// attaches to it.
struct ThreadState {
    gstate: ffi::PyGILState_STATE,
    tstate: *mut ffi::PyThreadState,
}

impl ThreadState {
    fn new() -> Self {
        // SAFETY: this thread doesn't have a thread state, so this creates one
        // and attaches it, then detaches it again without destroying it.
        unsafe {
            let gstate = ffi::PyGILState_Ensure();
            let tstate = ffi::PyEval_SaveThread();
            Self { gstate, tstate }
        }
    }
}

impl Drop for ThreadState {
    fn drop(&mut self) {
        // SAFETY: this runs in the thread that created the state, which isn't
        // attached outside of callbacks.
        unsafe {
            ffi::PyEval_RestoreThread(self.tstate);
            ffi::PyGILState_Release(self.gstate);
        }
    }
}

/// A Python callback that the walker's closures share with the Walk, so that
/// the Walk's `__traverse__` and `__clear__` account for the reference.
/// Otherwise a callback that references its Walk would form a cycle the
/// garbage collector can't see while walking.
pub struct SharedCallback(Mutex<Option<Py<PyAny>>>);

impl SharedCallback {
    pub fn new(callback: Py<PyAny>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Some(callback))))
    }

    /// The callback, or `None` if the garbage collector cleared the Walk.
    pub fn get(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        lock(&self.0)
            .as_ref()
            .map(|callback| callback.clone_ref(py))
    }

    pub fn traverse(&self, visit: &PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(callback) = try_lock(&self.0)
            && let Some(callback) = &*callback
        {
            visit.call(callback)?;
        }
        Ok(())
    }

    pub fn clear(&self) {
        let callback = lock(&self.0).take();
        // Drop outside the lock, since it may run Python code.
        drop(callback);
    }
}
