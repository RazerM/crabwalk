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
        Python::attach(|py| {
            let types_mod = match types_module(py) {
                Ok(types_mod) => types_mod,
                Err(err) => return err,
            };
            match error.0 {
                ignore::Error::Io(err) => err.into(),
                ignore::Error::Glob { err, glob } => {
                    let loop_error_type = match types_mod.getattr("GlobError") {
                        Err(err) => return err,
                        Ok(error_type) => error_type,
                    };
                    let Ok(glob) = glob.into_pyobject(py);
                    let kwargs = [("glob", glob)];
                    match loop_error_type.call((err,), Some(&kwargs.into_py_dict(py).unwrap())) {
                        Err(err) => err,
                        Ok(err) => PyErr::from_value(err),
                    }
                }
                ignore::Error::Loop { ancestor, child } => {
                    let loop_error_type = match types_mod.getattr("LoopError") {
                        Err(err) => return err,
                        Ok(error_type) => error_type,
                    };
                    let kwargs = [
                        ("ancestor", ancestor.into_os_string()),
                        ("child", child.into_os_string()),
                    ];
                    match loop_error_type.call((), Some(&kwargs.into_py_dict(py).unwrap())) {
                        Err(err) => err,
                        Ok(err) => PyErr::from_value(err),
                    }
                }
                ignore::Error::WithDepth { err, depth } => {
                    let py_err = PyErr::from(IgnoreError::from(err));
                    let value = py_err.into_value(py);
                    if let Err(seterr) = value.setattr(py, "depth", depth) {
                        return seterr;
                    }
                    PyErr::from_value(value.extract(py).unwrap())
                }
                ignore::Error::WithLineNumber { err, line } => {
                    let py_err = PyErr::from(IgnoreError::from(err));
                    let value = py_err.into_value(py);
                    if let Err(seterr) = value.setattr(py, "line", line) {
                        return seterr;
                    }
                    PyErr::from_value(value.extract(py).unwrap())
                }
                ignore::Error::WithPath { err, path } => {
                    let py_err = PyErr::from(IgnoreError::from(err));
                    let value = py_err.into_value(py);
                    if let Err(seterr) = value.setattr(py, "path", path.into_os_string()) {
                        return seterr;
                    }
                    PyErr::from_value(value.extract(py).unwrap())
                }
                ignore::Error::Partial(errors) => {
                    let partial_error_type = match types_mod.getattr("PartialError") {
                        Err(err) => return err,
                        Ok(error_type) => error_type,
                    };
                    let py_errors: Vec<PyErr> = errors
                        .into_iter()
                        .map(|err| IgnoreError(err).into())
                        .collect();
                    match partial_error_type.call1((py_errors,)) {
                        Err(err) => err,
                        Ok(err) => PyErr::from_value(err),
                    }
                }
                ignore::Error::InvalidDefinition => {
                    let invalid_definition_type = match types_mod.getattr("InvalidDefinitionError")
                    {
                        Err(err) => return err,
                        Ok(error_type) => error_type,
                    };
                    match invalid_definition_type.call0() {
                        Err(err) => err,
                        Ok(err) => PyErr::from_value(err),
                    }
                }
                ignore::Error::UnrecognizedFileType(name) => {
                    let unrecognized_file_type =
                        match types_mod.getattr("UnrecognizedFileTypeError") {
                            Err(err) => return err,
                            Ok(error_type) => error_type,
                        };
                    let kwargs = [("name", name)];
                    match unrecognized_file_type.call((), Some(&kwargs.into_py_dict(py).unwrap())) {
                        Err(err) => err,
                        Ok(err) => PyErr::from_value(err),
                    }
                }
            }
        })
    }
}
