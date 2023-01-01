use std::ffi::OsStr;
use std::path::Path;

use pyo3::prelude::*;
#[cfg(not(unix))] use pyo3::types::IntoPyDict;

#[pyclass(module = "crabwalk")]
pub(crate) struct DirEntry {
    inner: ignore::DirEntry,
    #[cfg(not(unix))] follow_links: bool,
    #[cfg(not(unix))] file_index: Option<u64>
}

impl DirEntry {
    #[cfg_attr(unix, allow(unused_variables))]
    pub(crate) fn new(dir_entry: ignore::DirEntry, follow_links: bool) -> Self {
        Self {
            inner: dir_entry,
            #[cfg(not(unix))] follow_links,
            #[cfg(not(unix))] file_index: None,
        }
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
    fn inode(&mut self, py: Python<'_>) -> PyResult<u64> {
        match self.file_index {
            Some(file_index) => Ok(file_index),
            None => {
                let kwargs = [("follow_symlinks", self.follow_links)];
                let file_index = py.import("os")?
                    .getattr("stat")?
                    .call((self.inner.path(),), Some(kwargs.into_py_dict(py)))?
                    .getattr("st_ino")?
                    .extract()?;
                self.file_index = Some(file_index);
                Ok(file_index)
            }
        }
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
