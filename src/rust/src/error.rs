use std::io;
use std::path::PathBuf;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;

use crate::types_module;

pub struct IgnoreError(ignore::Error);

impl From<ignore::Error> for IgnoreError {
    fn from(error: ignore::Error) -> Self {
        Self(error)
    }
}

impl From<Box<ignore::Error>> for IgnoreError {
    fn from(error: Box<ignore::Error>) -> Self {
        Self(*error)
    }
}

impl From<IgnoreError> for PyErr {
    fn from(error: IgnoreError) -> Self {
        Python::attach(|py| match to_exception(py, error.0, None) {
            Ok(exc) => PyErr::from_value(exc),
            Err(err) => err,
        })
    }
}

/// Convert an `ignore::Error` into an exception instance. `path` is from an
/// enclosing `ignore::Error::WithPath`.
fn to_exception(
    py: Python<'_>,
    error: ignore::Error,
    path: Option<PathBuf>,
) -> PyResult<Bound<'_, PyAny>> {
    let types = types_module(py)?;
    let exc = match error {
        // OSError needs the path when it's created, or pickling loses it on
        // Windows.
        ignore::Error::Io(err) => return os_error(py, err, path),
        ignore::Error::Glob { err, glob } => {
            let kwargs = [("glob", glob)].into_py_dict(py)?;
            types.getattr("GlobError")?.call((err,), Some(&kwargs))?
        }
        ignore::Error::Loop { ancestor, child } => {
            let kwargs = [
                ("ancestor", ancestor.into_os_string()),
                ("child", child.into_os_string()),
            ]
            .into_py_dict(py)?;
            types.getattr("LoopError")?.call((), Some(&kwargs))?
        }
        ignore::Error::WithDepth { err, depth } => {
            let exc = to_exception(py, *err, path)?;
            exc.setattr("depth", depth)?;
            return Ok(exc);
        }
        ignore::Error::WithLineNumber { err, line } => {
            let exc = to_exception(py, *err, path)?;
            exc.setattr("line", line)?;
            return Ok(exc);
        }
        ignore::Error::WithPath { err, path: inner } => {
            // The outermost path wins.
            return to_exception(py, *err, path.or(Some(inner)));
        }
        ignore::Error::Partial(errors) => {
            let errors = errors
                .into_iter()
                .map(|err| to_exception(py, err, None))
                .collect::<PyResult<Vec<_>>>()?;
            types.getattr("PartialError")?.call1((errors,))?
        }
        ignore::Error::InvalidDefinition => types.getattr("InvalidDefinitionError")?.call0()?,
        ignore::Error::UnrecognizedFileType(name) => {
            let kwargs = [("name", name)].into_py_dict(py)?;
            types
                .getattr("UnrecognizedFileTypeError")?
                .call((), Some(&kwargs))?
        }
    };
    if let Some(path) = path {
        exc.setattr("path", path.into_os_string())?;
    }
    Ok(exc)
}

/// Create an `OSError` like Python would, so that the subclass, `errno`,
/// `strerror`, and `filename` are set.
fn os_error(
    py: Python<'_>,
    err: io::Error,
    filename: Option<PathBuf>,
) -> PyResult<Bound<'_, PyAny>> {
    let filename = filename.map(PathBuf::into_os_string);
    let Some(code) = raw_os_error(&err) else {
        let exc = PyErr::from(err).into_value(py).into_bound(py).into_any();
        if let Some(filename) = filename {
            exc.setattr("filename", filename)?;
        }
        return Ok(exc);
    };
    let message = io::Error::from_raw_os_error(code).to_string();
    let strerror = message
        .strip_suffix(&format!(" (os error {code})"))
        .unwrap_or(&message);
    let os_error = py.get_type::<PyOSError>();
    if cfg!(windows) {
        // Like CPython, which removes the trailing period from Windows error
        // messages and derives errno from winerror.
        let strerror = strerror.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
        os_error.call1((0, strerror, filename, code))
    } else if let Some(filename) = filename {
        os_error.call1((code, strerror, filename))
    } else {
        os_error.call1((code, strerror))
    }
}

/// The OS error code for `err`, which the `ignore` crate sometimes wraps in
/// another error (e.g. from `walkdir`).
fn raw_os_error(err: &io::Error) -> Option<i32> {
    let mut source = Some(err as &(dyn std::error::Error + 'static));
    while let Some(err) = source {
        if let Some(code) = err
            .downcast_ref::<io::Error>()
            .and_then(io::Error::raw_os_error)
        {
            return Some(code);
        }
        source = err.source();
    }
    None
}
