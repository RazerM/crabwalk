use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyString};
use pyo3::{PyTraverseError, PyVisit, ffi};

use crate::OS_STAT;
use crate::util::{fsdecode, lock, try_lock};

// Frozen, which makes creating one for every entry cheaper. Walk.__next__
// also untracks them from the garbage collector, since they can't be part of
// a reference cycle until stat() caches a result.
#[pyclass(module = "crabwalk", frozen)]
pub(crate) struct DirEntry {
    inner: ignore::DirEntry,
    #[pyo3(get)]
    follow_symlinks: bool,
    // The cached stat result. A mutex rather than a once lock so that
    // __clear__ can drop it.
    stat: Mutex<Option<Py<PyAny>>>,
}

impl DirEntry {
    pub(crate) fn new(dir_entry: ignore::DirEntry, follow_symlinks: bool) -> Self {
        Self {
            inner: dir_entry,
            follow_symlinks,
            stat: Mutex::new(None),
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
    fn inode(slf: &Bound<'_, Self>) -> PyResult<u64> {
        let py = slf.py();
        Ok(Self::stat(slf)?.getattr(py, "st_ino")?.extract(py)?)
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

    fn stat(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let this = slf.get();
        if let Some(stat) = &*lock(&this.stat) {
            return Ok(stat.clone_ref(py));
        }
        let kwargs = [("follow_symlinks", this.follow_symlinks)].into_py_dict(py)?;
        let path = this.inner.path().as_os_str();
        let stat = OS_STAT
            .get(py)
            .unwrap()
            .bind(py)
            .call((path,), Some(&kwargs))?
            .unbind();

        let mut cached = lock(&this.stat);
        if let Some(existing) = &*cached {
            // Another thread cached a result first.
            let existing = existing.clone_ref(py);
            drop(cached);
            // Drop outside the lock, since it may run Python code.
            drop(stat);
            return Ok(existing);
        }
        *cached = Some(stat.clone_ref(py));
        // os.stat may have been replaced to return anything, so the entry
        // could now be part of a cycle. Only one thread can get here before
        // the entry is tracked, so it can't be tracked twice, which is a fatal
        // error.
        // SAFETY: slf is a valid object of a type that supports GC, and its
        // traverse function is valid.
        unsafe {
            if ffi::PyObject_GC_IsTracked(slf.as_ptr()) == 0 {
                ffi::PyObject_GC_Track(slf.as_ptr().cast());
            }
        }
        Ok(stat)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(stat) = try_lock(&self.stat)
            && let Some(stat) = &*stat
        {
            visit.call(stat)?;
        }
        Ok(())
    }

    fn __clear__(&self) {
        let stat = lock(&self.stat).take();
        // Drop outside the lock, since it may run Python code.
        drop(stat);
    }
}
