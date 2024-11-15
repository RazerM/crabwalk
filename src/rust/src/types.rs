use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedStr;
use pyo3::types::{PyDict, PyIterator, PyList, PyMapping, PySequence, PyString, PyTuple};
use pyo3::{PyTraverseError, PyVisit};
use regex::Regex;

use crate::error::IntoPyErr;
use crate::util::Maybe;
use crate::{ITEMS_VIEW_TYPE, KEYS_VIEW_TYPE, VALUES_VIEW_TYPE};

impl<'py> FromPyObject<'py> for Maybe<Bound<'py, PyAny>> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(Maybe::Some(ob.extract()?))
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
    #[pyo3(signature = (initial=None, /, **kwargs))]
    fn new(
        py: Python<'_>,
        initial: Option<Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let instance = Self {
            types: Some(PyDict::new_bound(py).unbind()),
            selections: Vec::new(),
        };
        instance.update(py, initial, kwargs)?;
        Ok(instance)
    }

    pub fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        name: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let types: &Bound<'py, PyAny> = self.types.as_ref().unwrap().bind(py);
        types
            .get_item(name)?
            .downcast::<PyList>()?
            .as_sequence()
            .to_tuple()
    }

    #[pyo3(signature = (key, default=None, /))]
    pub fn get<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
        default: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.__getitem__(py, key) {
            Ok(globs) => Ok(globs.into_any()),
            Err(err) if err.is_instance_bound(py, &py.get_type_bound::<PyKeyError>()) => {
                Ok(default.to_object(py).into_bound(py))
            }
            Err(err) => Err(err),
        }
    }

    pub fn __contains__(&self, py: Python<'_>, name: &Bound<'_, PyAny>) -> PyResult<bool> {
        match self.__getitem__(py, name) {
            Ok(_) => Ok(true),
            Err(err) if err.is_instance_bound(py, &py.get_type_bound::<PyKeyError>()) => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub fn keys(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        KEYS_VIEW_TYPE
            .get(py)
            .unwrap()
            .call1(py, (self_,))
            .map(Into::into)
    }

    pub fn items(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        ITEMS_VIEW_TYPE
            .get(py)
            .unwrap()
            .call1(py, (self_,))
            .map(Into::into)
    }

    pub fn values(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        VALUES_VIEW_TYPE
            .get(py)
            .unwrap()
            .call1(py, (self_,))
            .map(Into::into)
    }

    pub fn __richcmp__<'py>(
        self_: PyRef<'_, Self>,
        py: Python<'py>,
        other: &Bound<'py, PyMapping>,
        op: CompareOp,
    ) -> PyResult<PyObject> {
        let equal = match op {
            CompareOp::Eq => true,
            CompareOp::Ne => false,
            _ => return Ok(py.NotImplemented()),
        };
        let notequal = !equal;

        let self_obj = self_.into_py(py);
        let self_mapping = self_obj.bind(py).downcast::<PyMapping>()?;

        if self_mapping.len()? != other.len()? {
            return Ok(notequal.to_object(py));
        }

        let items: PyResult<Vec<(Bound<'_, PyString>, Bound<'_, PySequence>)>> = self_mapping
            .items()?
            .iter()?
            .map(|i| i.and_then(|t| t.extract()))
            .collect();

        for (name, globs) in items? {
            let other_globs = match other.get_item(name) {
                Ok(globs) => globs,
                Err(err) if err.is_instance_bound(py, &py.get_type_bound::<PyKeyError>()) => {
                    return Ok(notequal.to_object(py))
                }
                Err(err) => return Err(err),
            };
            let other_globs: Bound<'_, PySequence> = match other_globs.extract() {
                Ok(globs) => globs,
                Err(_) => return Ok(notequal.to_object(py)),
            };
            println!("{} != {}", globs, other_globs);
            if globs.ne(other_globs)? {
                return Ok(notequal.to_object(py));
            }
        }
        Ok(equal.to_object(py))
    }

    pub fn __len__(&self, py: Python<'_>) -> usize {
        self.types.as_ref().unwrap().bind(py).len()
    }

    pub fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyIterator>> {
        PyIterator::from_bound_object(self.types.as_ref().unwrap().bind(py)).map(Into::into)
    }

    pub fn __delitem__(&self, py: Python<'_>, name: &Bound<'_, PyAny>) -> PyResult<()> {
        self.types.as_ref().unwrap().bind(py).del_item(name)
    }

    pub fn __setitem__(
        &self,
        py: Python<'_>,
        name: &str,
        globs: &Bound<'_, PySequence>,
    ) -> PyResult<()> {
        for glob in globs.iter()? {
            self.add(py, name, glob?.downcast()?)?;
        }
        Ok(())
    }

    #[pyo3(signature = (key, default=Maybe::Missing, /))]
    pub fn pop(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Maybe<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        match self.__getitem__(py, key) {
            Ok(globs) => {
                self.__delitem__(py, key)?;
                Ok(globs.to_object(py))
            }
            Err(err) if err.is_instance_bound(py, &py.get_type_bound::<PyKeyError>()) => {
                match default {
                    Maybe::Some(default) => Ok(default.to_object(py)),
                    Maybe::Missing => Err(err),
                }
            }
            Err(err) => Err(err),
        }
    }

    pub fn popitem<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyString>, Bound<'py, PyTuple>)> {
        let (name, globs): (Bound<'_, _>, Bound<'_, PyList>) = self
            .types
            .as_ref()
            .unwrap()
            .bind(py)
            .call_method0("popitem")?
            .extract()?;
        Ok((name, globs.as_sequence().to_tuple()?))
    }

    pub fn clear(&self, py: Python<'_>) {
        self.types.as_ref().unwrap().bind(py).clear()
    }

    #[pyo3(
        signature = (other=None, /, **kwargs),
        text_signature = "($self, other=(), /, **kwargs)"
    )]
    pub fn update(
        &self,
        py: Python<'_>,
        other: Option<Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let other = other.unwrap_or_else(|| PyTuple::empty_bound(py).into_any());
        if let Ok(other) = other.downcast::<PyMapping>() {
            for name in other.iter()? {
                let name = &name?;
                self.__setitem__(py, &*name.extract::<PyBackedStr>()?, other.get_item(name)?.downcast()?)?;
            }
        } else if other.hasattr("keys")? {
            for name in other.call_method0("keys")?.iter()? {
                let name = &name?;
                self.__setitem__(py, &*name.extract::<PyBackedStr>()?, other.get_item(name)?.downcast()?)?;
            }
        } else {
            for item in other.iter()? {
                let (name, globs): (PyBackedStr, _) = item?.extract()?;
                self.__setitem__(py, &name, &globs)?;
            }
        }
        if let Some(kwargs) = kwargs {
            for (name, globs) in kwargs.iter() {
                self.__setitem__(py, &*name.extract::<PyBackedStr>()?, &globs.extract()?)?;
            }
        }
        Ok(())
    }

    #[pyo3(
        signature = (key, default=None, /),
        text_signature = "($self, key, default=(), /)"
    )]
    pub fn setdefault<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
        default: Option<Bound<'py, PySequence>>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let default = default.unwrap_or_else(|| PyTuple::empty_bound(py).extract().unwrap());
        match self.__getitem__(py, key) {
            Ok(globs) => Ok(globs),
            Err(err) if err.is_instance_bound(py, &py.get_type_bound::<PyKeyError>()) => {
                self.__setitem__(py, &*key.extract::<PyBackedStr>()?, &default)?;
                Ok(default.to_tuple()?)
            }
            Err(err) => Err(err),
        }
    }

    pub fn add(&self, py: Python<'_>, name: &str, glob: &Bound<'_, PyString>) -> PyResult<()> {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"^[\pL\pN]+$").unwrap();
        }
        if name == "all" || !RE.is_match(name) {
            return Err(ignore::Error::InvalidDefinition.into_py_err(py));
        }
        let types = self.types.as_ref().unwrap().bind(py);
        let globs: Bound<'_, PyList> = match types.get_item(name)? {
            Some(globs) => globs.downcast_into()?,
            None => {
                let globs = PyList::empty_bound(py);
                types.set_item(name, &globs)?;
                globs
            }
        };
        globs.append(glob)?;
        Ok(())
    }

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
                    glob.to_object(py).bind(py).downcast()?,
                )?
            }
        }
        Ok(())
    }

    /// Select the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are selected.
    pub fn select(&mut self, py: Python<'_>, name: &str) {
        if name == "all" {
            for name in self.types.as_ref().unwrap().bind(py).keys() {
                self.selections.push(Selection::Select(name.to_string()));
            }
        } else {
            self.selections.push(Selection::Select(name.to_string()));
        }
    }

    /// Ignore the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are negated.
    pub fn negate(&mut self, py: Python<'_>, name: &str) {
        if name == "all" {
            for name in self.types.as_ref().unwrap().bind(py).keys() {
                self.selections.push(Selection::Negate(name.to_string()));
            }
        } else {
            self.selections.push(Selection::Negate(name.to_string()));
        }
    }

    fn __getnewargs__(&self) -> (Py<PyDict>,) {
        (self.types.clone().unwrap(),)
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let selections: Vec<_> = self
            .selections
            .iter()
            .map(|selection| match selection {
                Selection::Select(name) => ("select", name),
                Selection::Negate(name) => ("negate", name),
            })
            .collect();
        let state = PyDict::new_bound(py);
        state.set_item("selections", selections.to_object(py))?;
        Ok(state.into())
    }

    fn __setstate__(&mut self, py: Python<'_>, state: Py<PyDict>) -> PyResult<()> {
        let state = state.bind(py);
        let selections = <Bound<'_, PyAny>>::get_item(state, "selections")?;
        let selections: Vec<(String, String)> = selections.extract()?;
        self.selections = selections
            .into_iter()
            .map(|(case, name)| match case.as_ref() {
                "select" => Ok(Selection::Select(name)),
                "negate" => Ok(Selection::Negate(name)),
                _ => Err(PyValueError::new_err("Invalid state")),
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(())
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
