use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{IntoPyDict, PyString};

use crate::OS_STAT;
use crate::util::fsdecode;

// Frozen and without __traverse__ (os.stat_result can't form a reference
// cycle), which makes creating one for every entry cheaper.
#[pyclass(module = "crabwalk", frozen)]
pub(crate) struct DirEntry {
    inner: ignore::DirEntry,
    #[pyo3(get)]
    follow_symlinks: bool,
    stat: PyOnceLock<Py<PyAny>>,
}

impl DirEntry {
    pub(crate) fn new(dir_entry: ignore::DirEntry, follow_symlinks: bool) -> Self {
        Self {
            inner: dir_entry,
            follow_symlinks,
            stat: PyOnceLock::new(),
        }
    }
}

#[pymethods]
impl DirEntry {
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        // Convert path to a PyString and use repr so the output
        // contains surrogate escapes.
        let path = fsdecode(py, self.inner.path().as_os_str())?;
        Ok(format!("<DirEntry {}>", path.repr()?))
    }

    #[getter]
    fn path<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        fsdecode(py, self.inner.path().as_os_str())
    }

    #[cfg(unix)]
    fn inode(&self) -> u64 {
        self.inner.ino().expect("DirEntry is not Stdin")
    }

    #[cfg(not(unix))]
    fn inode(&self, py: Python<'_>) -> PyResult<u64> {
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
    fn name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        fsdecode(py, self.inner.file_name())
    }

    #[getter]
    fn depth(&self) -> usize {
        self.inner.depth()
    }

    fn __fspath__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        fsdecode(py, self.inner.path().as_os_str())
    }

    fn stat(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.stat
            .get_or_try_init(py, || {
                let kwargs = [("follow_symlinks", self.follow_symlinks)].into_py_dict(py)?;
                let path = self.inner.path().as_os_str();
                let stat = OS_STAT
                    .get(py)
                    .unwrap()
                    .bind(py)
                    .call((path,), Some(&kwargs))?;
                Ok::<_, PyErr>(stat.unbind())
            })
            .map(|stat| stat.clone_ref(py))
    }
}
