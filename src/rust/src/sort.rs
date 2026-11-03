use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use pyo3::prelude::*;
use pyo3::sync::MutexExt;

use crate::callback::CallbackError;

/// Compares paths using a Python key function, like `sorted(key=...)`.
///
/// The `ignore` crate sorts one directory at a time using a comparison
/// function, so keys for the current directory are cached to call the key
/// function once per entry rather than twice per comparison.
pub struct KeyComparator {
    key: Py<PyAny>,
    cache: Mutex<KeyCache>,
    error: CallbackError,
}

#[derive(Default)]
struct KeyCache {
    parent: Option<PathBuf>,
    keys: HashMap<PathBuf, Py<PyAny>>,
}

impl KeyComparator {
    pub fn new(key: Py<PyAny>, error: CallbackError) -> Self {
        Self {
            key,
            cache: Mutex::default(),
            error,
        }
    }

    pub fn compare(&self, a: &Path, b: &Path) -> Ordering {
        if self.error.is_set() {
            return a.cmp(b);
        }
        Python::attach(|py| {
            self.try_compare(py, a, b).unwrap_or_else(|err| {
                self.error.set(err);
                a.cmp(b)
            })
        })
    }

    fn try_compare(&self, py: Python<'_>, a: &Path, b: &Path) -> PyResult<Ordering> {
        let (a, b) = {
            let mut cache = self
                .cache
                .lock_py_attached(py)
                .unwrap_or_else(PoisonError::into_inner);
            (cache.key(py, &self.key, a)?, cache.key(py, &self.key, b)?)
        };
        // Like sorted(), only use <.
        Ok(if a.lt(&b)? {
            Ordering::Less
        } else if b.lt(&a)? {
            Ordering::Greater
        } else {
            Ordering::Equal
        })
    }
}

impl KeyCache {
    fn key<'py>(
        &mut self,
        py: Python<'py>,
        key: &Py<PyAny>,
        path: &Path,
    ) -> PyResult<Bound<'py, PyAny>> {
        if self.parent.as_deref() != path.parent() {
            self.parent = path.parent().map(Path::to_path_buf);
            self.keys.clear();
        }
        if let Some(value) = self.keys.get(path) {
            return Ok(value.bind(py).clone());
        }
        let value = key.bind(py).call1((path.as_os_str(),))?;
        self.keys.insert(path.to_path_buf(), value.clone().unbind());
        Ok(value)
    }
}
