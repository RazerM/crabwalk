use ignore::types::TypesBuilder;
use pyo3::prelude::*;

/// Return whether `name` is a valid file type name.
///
/// This defers to the `ignore` crate so that the rules match exactly.
#[pyfunction]
pub fn is_valid_type_name(name: &str) -> bool {
    TypesBuilder::new().add(name, "").is_ok()
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
