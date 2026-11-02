use std::cmp::Ordering;
use std::collections::HashMap;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use pyo3::{PyTraverseError, PyVisit};

use crate::callback::{CallbackError, SharedCallback};
use crate::util::{lock, try_lock};

/// Compares paths using a Python key function, like `sorted(key=...)`.
///
/// The `ignore` crate sorts one directory at a time using a comparison
/// function, so keys for the current directory are cached to call the key
/// function once per entry rather than twice per comparison.
///
/// The Walk shares this with the walker so that its `__traverse__` and
/// `__clear__` account for the key function and cached keys.
pub struct KeyComparator {
    key: Arc<SharedCallback>,
    cache: Mutex<KeyCache>,
    error: CallbackError,
}

#[derive(Default)]
struct KeyCache {
    parent: Option<PathBuf>,
    keys: HashMap<PathBuf, Py<PyAny>>,
}

impl KeyComparator {
    pub fn new(key: Py<PyAny>, error: CallbackError) -> Arc<Self> {
        Arc::new(Self {
            key: SharedCallback::new(key),
            cache: Mutex::default(),
            error,
        })
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

    pub fn traverse(&self, visit: &PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.key.traverse(visit)?;
        if let Some(cache) = try_lock(&self.cache) {
            for key in cache.keys.values() {
                visit.call(key)?;
            }
        }
        Ok(())
    }

    pub fn clear(&self) {
        self.key.clear();
        let keys = mem::take(&mut lock(&self.cache).keys);
        // Drop outside the lock, since it may run Python code.
        drop(keys);
    }

    fn try_compare(&self, py: Python<'_>, a: &Path, b: &Path) -> PyResult<Ordering> {
        let Some(key) = self.key.get(py) else {
            // The Walk was cleared by the garbage collector.
            return Ok(a.cmp(b));
        };
        let a = self.key(py, &key, a)?;
        let b = self.key(py, &key, b)?;
        // Like sorted(), only use <.
        Ok(if a.lt(&b)? {
            Ordering::Less
        } else if b.lt(&a)? {
            Ordering::Greater
        } else {
            Ordering::Equal
        })
    }

    /// The key for `path`, from the cache if possible. The cache isn't locked
    /// while calling Python, which could run the garbage collector.
    fn key<'py>(
        &self,
        py: Python<'py>,
        key: &Py<PyAny>,
        path: &Path,
    ) -> PyResult<Bound<'py, PyAny>> {
        let stale = {
            let mut cache = lock(&self.cache);
            if cache.parent.as_deref() == path.parent() {
                if let Some(value) = cache.keys.get(path) {
                    return Ok(value.bind(py).clone());
                }
                None
            } else {
                cache.parent = path.parent().map(Path::to_path_buf);
                Some(mem::take(&mut cache.keys))
            }
        };
        drop(stale);

        let value = key.bind(py).call1((path.as_os_str(),))?;

        let replaced = {
            let mut cache = lock(&self.cache);
            if cache.parent.as_deref() == path.parent() {
                cache
                    .keys
                    .insert(path.to_path_buf(), value.clone().unbind())
            } else {
                None
            }
        };
        drop(replaced);
        Ok(value)
    }
}
