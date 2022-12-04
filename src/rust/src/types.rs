use ignore;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyIterator, PyList, PyMapping, PySequence, PyString, PyTuple};
use pyo3::{PyTraverseError, PyVisit};
use regex::Regex;

use crate::error::IntoPyErr;
use crate::{ITEMS_VIEW_TYPE, KEYS_VIEW_TYPE, VALUES_VIEW_TYPE};

#[derive(Clone, Debug)]
pub enum Maybe<T> {
    Some(T),
    Missing,
}

impl<'source> FromPyObject<'source> for Maybe<&'source PyAny> {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        Ok(Maybe::Some(ob))
    }
}

#[derive(Clone, Debug)]
pub enum Selection {
    Select(String),
    Negate(String),
}

#[pyclass(module = "crabwalk", mapping)]
#[derive(Clone)]
pub struct Types {
    types: Option<Py<PyDict>>,
    pub selections: Vec<Selection>,
}

#[pymethods]
impl Types {
    #[new]
    fn new(py: Python<'_>) -> Self {
        Self {
            types: Some(PyDict::new(py).into_py(py)),
            selections: Vec::new(),
        }
    }

    pub fn __getitem__<'py>(&'py self, py: Python<'py>, name: &PyAny) -> PyResult<&PyTuple> {
        let types: &PyAny = self.types.as_ref().unwrap().as_ref(py);
        Ok(types
            .get_item(name)?
            .downcast::<PyList>()?
            .as_sequence()
            .tuple()?)
    }

    #[args(key, default = "None", "/")]
    #[pyo3(text_signature = "(key, default=None, /)")]
    pub fn get<'py>(
        &'py self,
        py: Python<'py>,
        key: &PyAny,
        default: Option<&'py PyAny>,
    ) -> PyResult<PyObject> {
        match self.__getitem__(py, key) {
            Ok(globs) => Ok(globs.to_object(py)),
            Err(err) if err.is_instance(py, py.get_type::<PyKeyError>()) => {
                Ok(default.to_object(py))
            }
            Err(err) => Err(err),
        }
    }

    pub fn __contains__(&self, py: Python<'_>, name: &PyAny) -> PyResult<bool> {
        match self.__getitem__(py, name) {
            Ok(_) => Ok(true),
            Err(err) if err.is_instance(py, py.get_type::<PyKeyError>()) => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub fn keys<'py>(self_: PyRef<'py, Self>, py: Python<'py>) -> PyResult<&'py PyAny> {
        Ok(KEYS_VIEW_TYPE.get(py).unwrap().as_ref(py).call1((self_,))?)
    }

    pub fn items<'py>(self_: PyRef<'py, Self>, py: Python<'py>) -> PyResult<&'py PyAny> {
        Ok(ITEMS_VIEW_TYPE
            .get(py)
            .unwrap()
            .as_ref(py)
            .call1((self_,))?)
    }

    pub fn values<'py>(self_: PyRef<'py, Self>, py: Python<'py>) -> PyResult<&'py PyAny> {
        Ok(VALUES_VIEW_TYPE
            .get(py)
            .unwrap()
            .as_ref(py)
            .call1((self_,))?)
    }

    pub fn __richcmp__(
        self_: PyRef<'_, Self>,
        py: Python<'_>,
        other: &PyMapping,
        op: CompareOp,
    ) -> PyResult<PyObject> {
        let equal = match op {
            CompareOp::Eq => true,
            CompareOp::Ne => false,
            _ => return Ok(py.NotImplemented()),
        };
        let notequal = !equal;

        let self_obj = self_.into_py(py);
        let self_mapping: &PyMapping = self_obj.as_ref(py).downcast()?;

        if self_mapping.len()? != other.len()? {
            return Ok(notequal.to_object(py));
        }

        let items: PyResult<Vec<(String, Vec<String>)>> = self_mapping
            .items()?
            .iter()?
            .map(|i| i.and_then(PyAny::extract))
            .collect();

        for (name, globs) in items? {
            let other_globs = match other.get_item(name) {
                Ok(globs) => globs,
                Err(err) if err.is_instance(py, py.get_type::<PyKeyError>()) => {
                    return Ok(notequal.to_object(py))
                }
                Err(err) => return Err(err),
            };
            let other_globs: Vec<String> = match other_globs.extract() {
                Ok(globs) => globs,
                Err(_) => return Ok(notequal.to_object(py)),
            };
            if globs != other_globs {
                return Ok(notequal.to_object(py));
            }
        }
        Ok(equal.to_object(py))
    }

    pub fn __len__(&self, py: Python<'_>) -> usize {
        self.types.as_ref().unwrap().as_ref(py).len()
    }

    pub fn __iter__<'py>(&'py self, py: Python<'py>) -> PyResult<&PyIterator> {
        PyIterator::from_object(py, self.types.as_ref().unwrap().as_ref(py))
    }

    pub fn __delitem__(&self, py: Python<'_>, name: &PyAny) -> PyResult<()> {
        self.types.as_ref().unwrap().as_ref(py).del_item(name)
    }

    pub fn __setitem__(&self, py: Python<'_>, name: &str, globs: &PySequence) -> PyResult<()> {
        for glob in globs.iter()? {
            self.add(py, name, glob?.downcast()?)?;
        }
        Ok(())
    }

    // I wanted to do the following, but then pyo3 doesn't
    // #[args(key, default = "Maybe::Missing", "/")]
    // #[pyo3(text_signature = "(key, default=<missing>, /)")]
    #[args(key, "/", default = "Maybe::Missing")]
    #[pyo3(text_signature = "(key, /, default=<missing>)")]
    pub fn pop<'py>(
        &'py self,
        py: Python<'py>,
        key: &PyAny,
        default: Maybe<&'py PyAny>,
    ) -> PyResult<PyObject> {
        match self.__getitem__(py, key) {
            Ok(globs) => {
                self.__delitem__(py, key)?;
                Ok(globs.to_object(py))
            }
            Err(err) if err.is_instance(py, py.get_type::<PyKeyError>()) => match default {
                Maybe::Some(default) => Ok(default.to_object(py)),
                Maybe::Missing => Err(err),
            },
            Err(err) => Err(err),
        }
    }

    #[pyo3(text_signature = "()")]
    pub fn popitem<'py>(&'py self, py: Python<'py>) -> PyResult<(&PyString, &PyTuple)> {
        let (name, globs): (&PyString, &PyList) = self
            .types
            .as_ref()
            .unwrap()
            .as_ref(py)
            .call_method0("popitem")?
            .extract()?;
        Ok((name, globs.as_sequence().tuple()?))
    }

    #[pyo3(text_signature = "()")]
    pub fn clear(&self, py: Python<'_>) {
        self.types.as_ref().unwrap().as_ref(py).clear()
    }

    #[args(other = "None", "/", kwargs = "**")]
    #[pyo3(text_signature = "(other, /, **kwargs)")]
    pub fn update(
        &self,
        py: Python<'_>,
        other: Option<&PyAny>,
        kwargs: Option<&PyDict>,
    ) -> PyResult<()> {
        let other = other.unwrap_or_else(|| PyTuple::empty(py));
        if let Ok(other) = other.downcast::<PyMapping>() {
            for name in other.iter()? {
                let name = name?;
                self.__setitem__(py, name.extract()?, other.get_item(name)?.downcast()?)?;
            }
        } else if other.hasattr("keys")? {
            for name in other.call_method0("keys")?.iter()? {
                let name = name?;
                self.__setitem__(py, name.extract()?, other.get_item(name)?.downcast()?)?;
            }
        } else {
            for item in other.iter()? {
                let (name, globs) = item?.extract()?;
                self.__setitem__(py, name, globs)?;
            }
        }
        if let Some(kwargs) = kwargs {
            for (name, globs) in kwargs.iter() {
                self.__setitem__(py, name.extract()?, globs.extract()?)?;
            }
        }
        Ok(())
    }

    #[args(key, default = "None", "/")]
    #[pyo3(text_signature = "(key, default=(), /)")]
    pub fn setdefault<'py>(
        &'py self,
        py: Python<'py>,
        key: &PyAny,
        default: Option<&'py PyAny>,
    ) -> PyResult<&PyTuple> {
        let default: &PySequence = default.unwrap_or_else(|| PyTuple::empty(py)).downcast()?;
        match self.__getitem__(py, key) {
            Ok(globs) => Ok(globs),
            Err(err) if err.is_instance(py, py.get_type::<PyKeyError>()) => {
                self.__setitem__(py, key.extract()?, default)?;
                Ok(default.tuple()?)
            }
            Err(err) => Err(err),
        }
    }

    #[pyo3(text_signature = "(name, glob)")]
    pub fn add(&self, py: Python<'_>, name: &str, glob: &PyString) -> PyResult<()> {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"^[\pL\pN]+$").unwrap();
        }
        if name == "all" || !RE.is_match(name) {
            return Err(ignore::Error::InvalidDefinition.into_py_err(py));
        }
        let types = self.types.as_ref().unwrap().as_ref(py);
        let globs: &PyList = match types.get_item(name) {
            Some(globs) => globs.downcast()?,
            None => {
                let globs = PyList::empty(py);
                types.set_item(name, globs)?;
                globs
            }
        };
        globs.append(glob)?;
        Ok(())
    }

    #[pyo3(text_signature = "()")]
    pub fn add_defaults(&self, py: Python<'_>) -> PyResult<()> {
        lazy_static::lazy_static! {
            static ref DEFAULT_TYPES: Vec<ignore::types::FileTypeDef> =
                ignore::types::TypesBuilder::new().add_defaults().definitions();
        }
        for definition in DEFAULT_TYPES.iter() {
            for glob in definition.globs() {
                self.add(
                    py,
                    definition.name(),
                    glob.to_object(py).as_ref(py).downcast()?,
                )?
            }
        }
        Ok(())
    }

    /// Select the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are selected.
    #[pyo3(text_signature = "(name)")]
    pub fn select(&mut self, py: Python<'_>, name: &str) {
        if name == "all" {
            for name in self.types.as_ref().unwrap().as_ref(py).keys() {
                self.selections.push(Selection::Select(name.to_string()));
            }
        } else {
            self.selections.push(Selection::Select(name.to_string()));
        }
    }

    /// Ignore the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are negated.
    #[pyo3(text_signature = "(name)")]
    pub fn negate(&mut self, py: Python<'_>, name: &str) {
        if name == "all" {
            for name in self.types.as_ref().unwrap().as_ref(py).keys() {
                self.selections.push(Selection::Negate(name.to_string()));
            }
        } else {
            self.selections.push(Selection::Negate(name.to_string()));
        }
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(types) = &self.types {
            visit.call(types)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.types = None;
    }
}