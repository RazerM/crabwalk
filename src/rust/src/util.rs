use std::ffi::{OsStr, OsString};

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
