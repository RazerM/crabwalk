use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pyo3::PyErr;

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
        let err = self.lock().take();
        self.0.is_set.store(false, Ordering::Release);
        err
    }

    fn lock(&self) -> MutexGuard<'_, Option<PyErr>> {
        // Python is never called while this is locked, so poisoning would only
        // happen if a PyErr panicked on drop.
        self.0.err.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
