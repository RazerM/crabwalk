use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pyo3::PyErr;

/// The first exception raised by a Python callback (a sort key or
/// `filter_entry`) while the `ignore` crate is walking.
///
/// The callbacks can't return errors to the `ignore` crate, so they store them
/// here and `Walk.__next__` raises them. Once set, callbacks shouldn't call
/// Python again until the error has been taken.
#[derive(Clone, Default)]
pub struct CallbackError(Arc<Mutex<Option<PyErr>>>);

impl CallbackError {
    pub fn is_set(&self) -> bool {
        self.lock().is_some()
    }

    pub fn set(&self, err: PyErr) {
        self.lock().get_or_insert(err);
    }

    pub fn take(&self) -> Option<PyErr> {
        self.lock().take()
    }

    fn lock(&self) -> MutexGuard<'_, Option<PyErr>> {
        // Python is never called while this is locked, so poisoning would only
        // happen if a PyErr panicked on drop.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
