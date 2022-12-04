use std::ffi::OsString;

use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::{ffi, AsPyPointer};

pub fn fspath(path: &PyAny) -> PyResult<&PyAny> {
    let py = path.py();
    let path: &PyAny = unsafe {
        let ptr = ffi::PyOS_FSPath(path.as_ptr());
        py.from_owned_ptr_or_err(ptr)?
    };
    Ok(path)
}

pub fn fspath_list(paths: &PyList) -> PyResult<Vec<OsString>> {
    paths
        .iter()
        .map(|path| fspath(path).and_then(PyAny::extract))
        .collect()
}

/// Similar to Option but the pyo3 conversion traits are not implemented for it, so we can use
/// it as a default argument and know that it wasn't passed.
#[derive(Clone, Debug)]
pub enum Maybe<T> {
    Some(T),
    Missing,
}
