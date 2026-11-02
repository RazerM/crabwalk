use ignore::types::TypesBuilder;
use pyo3::prelude::*;

use crate::error::IgnoreError;

/// Raise `InvalidDefinitionError` if `name` isn't a valid file type name.
///
/// This defers to the `ignore` crate so that the rules match exactly.
#[pyfunction]
pub fn validate_type_name(name: &str) -> PyResult<()> {
    TypesBuilder::new()
        .add(name, "")
        .map_err(|err| IgnoreError::from(err).into())
}

/// Return the default file type definitions as `(name, globs)` pairs.
#[pyfunction]
pub fn default_types() -> Vec<(String, Vec<String>)> {
    TypesBuilder::new()
        .add_defaults()
        .definitions()
        .into_iter()
        .map(|def| (def.name().to_owned(), def.globs().to_vec()))
        .collect()
}
