use std::ffi::OsString;

use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyList;

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

/// Similar to Option but the pyo3 conversion traits are not implemented for it, so we can use
/// it as a default argument and know that it wasn't passed.
#[derive(Clone, Debug)]
pub enum Maybe<T> {
    Some(T),
    Missing,
}
