use std::ffi::OsStr;
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use pyo3::{PyTraverseError, PyVisit};

use crate::OS_STAT;

#[pyclass(module = "crabwalk")]
pub(crate) struct DirEntry {
    inner: ignore::DirEntry,
    #[pyo3(get)]
    follow_symlinks: bool,
    stat: Option<PyObject>,
}

impl DirEntry {
    pub(crate) fn new(dir_entry: ignore::DirEntry, follow_symlinks: bool) -> Self {
        Self {
            inner: dir_entry,
            follow_symlinks,
            stat: None,
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
        Ok(self.stat(py)?.getattr(py, "st_ino")?.extract(py)?)
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

    fn stat(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        match &self.stat {
            Some(stat) => Ok(stat.clone_ref(py)),
            None => {
                let kwargs = [("follow_symlinks", self.follow_symlinks)];
                let stat = OS_STAT
                    .get(py)
                    .unwrap()
                    .as_ref(py)
                    .call((self.inner.path(),), Some(kwargs.into_py_dict(py)))?;
                self.stat = Some(stat.into());
                Ok(stat.into())
            }
        }
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(stat) = &self.stat {
            visit.call(stat)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.stat = None;
    }
}
