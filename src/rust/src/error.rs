use pyo3::prelude::*;
use pyo3::types::IntoPyDict;

use crate::TYPES_MODULE;

pub trait IntoPyErr {
    fn into_py_err(self, py: Python<'_>) -> PyErr;
}

impl IntoPyErr for ignore::Error {
    fn into_py_err(self, py: Python<'_>) -> PyErr {
        let types_mod = TYPES_MODULE.get(py).unwrap().bind(py);
        match self {
            ignore::Error::Io(err) => err.into(),
            ignore::Error::Glob { err, glob } => {
                let loop_error_type = match types_mod.getattr("GlobError") {
                    Err(err) => return err,
                    Ok(error_type) => error_type,
                };
                let kwargs = [("glob", glob.into_py(py))];
                match loop_error_type.call((err,), Some(&kwargs.into_py_dict_bound(py))) {
                    Err(err) => err,
                    Ok(err) => PyErr::from_value_bound(err),
                }
            }
            ignore::Error::Loop { ancestor, child } => {
                let loop_error_type = match types_mod.getattr("LoopError") {
                    Err(err) => return err,
                    Ok(error_type) => error_type,
                };
                let kwargs = [("ancestor", ancestor), ("child", child)];
                match loop_error_type.call((), Some(&kwargs.into_py_dict_bound(py))) {
                    Err(err) => err,
                    Ok(err) => PyErr::from_value_bound(err),
                }
            }
            ignore::Error::WithDepth { err, depth } => {
                let py_err = err.into_py_err(py);
                Python::with_gil(|py| {
                    let value = py_err.into_value(py);
                    if let Err(seterr) = value.setattr(py, "depth", depth) {
                        return seterr;
                    }
                    PyErr::from_value_bound(value.extract(py).unwrap())
                })
            }
            ignore::Error::WithLineNumber { err, line } => {
                let py_err = err.into_py_err(py);
                Python::with_gil(|py| {
                    let value = py_err.into_value(py);
                    if let Err(seterr) = value.setattr(py, "line", line) {
                        return seterr;
                    }
                    PyErr::from_value_bound(value.extract(py).unwrap())
                })
            }
            ignore::Error::WithPath { err, path } => {
                let py_err = err.into_py_err(py);
                Python::with_gil(|py| {
                    let value = py_err.into_value(py);
                    if let Err(seterr) = value.setattr(py, "path", path) {
                        return seterr;
                    }
                    PyErr::from_value_bound(value.extract(py).unwrap())
                })
            }
            ignore::Error::Partial(errors) => {
                let partial_error_type = match types_mod.getattr("PartialError") {
                    Err(err) => return err,
                    Ok(error_type) => error_type,
                };
                let py_errors: Vec<PyErr> =
                    errors.into_iter().map(|err| err.into_py_err(py)).collect();
                match partial_error_type.call1((py_errors,)) {
                    Err(err) => err,
                    Ok(err) => PyErr::from_value_bound(err),
                }
            }
            ignore::Error::InvalidDefinition => {
                let invalid_definition_type = match types_mod.getattr("InvalidDefinitionError") {
                    Err(err) => return err,
                    Ok(error_type) => error_type,
                };
                match invalid_definition_type.call0() {
                    Err(err) => err,
                    Ok(err) => PyErr::from_value_bound(err),
                }
            }
            ignore::Error::UnrecognizedFileType(name) => {
                let unrecognized_file_type = match types_mod.getattr("UnrecognizedFileTypeError") {
                    Err(err) => return err,
                    Ok(error_type) => error_type,
                };
                let kwargs = [("name", name)];
                match unrecognized_file_type.call((), Some(&kwargs.into_py_dict_bound(py))) {
                    Err(err) => err,
                    Ok(err) => PyErr::from_value_bound(err),
                }
            }
        }
    }
}
