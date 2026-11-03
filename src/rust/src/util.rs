use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};

use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};

pub fn fspath<'py>(path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = path.py();
    let path = unsafe {
        let ptr = ffi::PyOS_FSPath(path.as_ptr());
        Bound::from_owned_ptr_or_err(py, ptr)?
    };
    Ok(path)
}

pub fn fspath_list(paths: &Bound<'_, PyList>) -> PyResult<Vec<OsString>> {
    paths
        .iter()
        .map(|path| fspath(&path).and_then(|p| p.extract()))
        .collect()
}

/// Convert an OS string to a Python string like `os.fsdecode`, which is what
/// `os.DirEntry` does.
///
/// On Unix, this decodes once instead of pyo3's conversion, which checks for
/// UTF-8 before creating the string.
#[cfg(unix)]
pub fn fsdecode<'py>(py: Python<'py>, s: &OsStr) -> PyResult<Bound<'py, PyString>> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = s.as_bytes();
    // SAFETY: the pointer and length are valid for the call, which returns a
    // new reference to a str or NULL with an exception set.
    unsafe {
        let ptr = ffi::PyUnicode_DecodeFSDefaultAndSize(
            bytes.as_ptr().cast(),
            bytes.len() as ffi::Py_ssize_t,
        );
        Ok(Bound::from_owned_ptr_or_err(py, ptr)?.cast_into_unchecked())
    }
}

#[cfg(not(unix))]
pub fn fsdecode<'py>(py: Python<'py>, s: &OsStr) -> PyResult<Bound<'py, PyString>> {
    Ok(s.into_pyobject(py)?)
}

/// Lock a mutex that guards Python objects.
///
/// These mutexes are only held briefly while attached to the interpreter, and
/// never while calling Python or detaching, so blocking while attached can't
/// deadlock. Don't use `MutexExt::lock_py_attached` for them: it re-attaches
/// while holding the lock, so the holder could be waiting for the GIL while a
/// thread in `__clear__`, which can't detach, waits for the lock.
///
/// Poisoning is ignored, since every critical section leaves the data valid
/// even if it panics, and a poisoned mutex stays poisoned.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Lock a mutex that guards Python objects if it isn't held by another thread,
/// for `__traverse__`, which must never block.
///
/// When this returns `None`, `__traverse__` skips the references behind the
/// lock. That only makes the garbage collector treat the objects as reachable
/// until a later collection, whereas visiting references that don't exist
/// could free objects that are still in use. Poisoning is ignored like in
/// `lock`, or the references would be skipped forever.
pub fn try_lock<T>(mutex: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    match mutex.try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(err)) => Some(err.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}
