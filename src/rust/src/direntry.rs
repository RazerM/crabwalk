use std::ffi::OsStr;
use std::path::Path;

use pyo3::prelude::*;

#[pyclass(module = "crabwalk")]
pub(crate) struct DirEntry {
    inner: ignore::DirEntry,
}

impl DirEntry {
    pub(crate) fn new(dir_entry: ignore::DirEntry) -> Self {
        Self { inner: dir_entry }
    }
}

#[pymethods]
impl DirEntry {
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        // Convert path to a PyString and use repr so the output
        // contains surrogate escapes.
        let path = self.inner.path().as_os_str().to_object(py);
        Ok(format!("<DirEntry {}>", path.as_ref(py).repr()?))
    }

    #[getter]
    fn path(&self) -> &Path {
        self.inner.path()
    }

    #[cfg(unix)]
    fn inode(&self) -> u64 {
        self.inner.ino().expect("DirEntry is not Stdin")
    }

    #[cfg(not(unix))]
    fn inode(&self, py: Python<'_>) -> PyObject {
        py.None()
    }

    fn is_dir(&self) -> bool {
        self.inner
            .file_type()
            .expect("DirEntry is not Stdin")
            .is_dir()
    }

    fn is_file(&self) -> bool {
        self.inner
            .file_type()
            .expect("DirEntry is not Stdin")
            .is_file()
    }

    fn is_symlink(&self) -> bool {
        self.inner.path_is_symlink()
    }

    #[getter]
    fn name(&self) -> &OsStr {
        self.inner.file_name()
    }

    #[getter]
    fn depth(&self) -> usize {
        self.inner.depth()
    }

    fn __fspath__(&self) -> &Path {
        self.inner.path()
    }
}
