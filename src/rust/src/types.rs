use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedStr;
use pyo3::types::{PyDict, PyIterator, PyList, PyMapping, PySequence, PyString, PyTuple};
use pyo3::{intern, DowncastError, PyTraverseError, PyTypeInfo, PyVisit};
use regex::Regex;
use std::iter::zip;
use std::sync::Arc;

use crate::error::IgnoreError;
use crate::util::Maybe;
use crate::{ITEMS_VIEW_TYPE, KEYS_VIEW_TYPE, VALUES_VIEW_TYPE};

impl<'py> FromPyObject<'py> for Maybe<Bound<'py, PyAny>> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(Maybe::Some(ob.extract()?))
    }
}

#[pyclass(module = "crabwalk", frozen)]
pub struct Select {
    name: Py<PyString>,
}

#[pymethods]
impl Select {
    #[classattr]
    fn __match_args__() -> (String,) {
        ("name".to_string(),)
    }

    #[new]
    #[pyo3(signature = (name, /))]
    fn py_new(name: Py<PyString>) -> Self {
        Self { name }
    }

    #[getter]
    fn get_name(&self, py: Python<'_>) -> Py<PyString> {
        self.name.clone_ref(py)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let name = self.name.bind_borrowed(py);
        Ok(format!("<Select {}>", name.repr()?))
    }

    fn __getnewargs__(&self, py: Python<'_>) -> (Py<PyString>,) {
        (self.name.clone_ref(py),)
    }

    fn __eq__(&self, py: Python<'_>, other: &Self) -> PyResult<bool> {
        self.name.bind(py).as_any().eq(other.name.bind(py))
    }

    fn __ne__(&self, py: Python<'_>, other: &Self) -> PyResult<bool> {
        self.name.bind(py).as_any().ne(other.name.bind(py))
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        let elements = [
            &Self::type_object(py).into_any(),
            self.name.bind(py).as_any(),
        ];
        PyTuple::new(py, elements)?.hash()
    }
}

impl Select {
    pub fn new(name: Py<PyString>) -> Self {
        Self { name }
    }

    pub fn name(&self, py: Python<'_>) -> PyResult<PyBackedStr> {
        self.name.bind_borrowed(py).extract::<PyBackedStr>()
    }
}

#[pyclass(module = "crabwalk", frozen)]
pub struct Negate {
    pub name: Py<PyString>,
}

#[pymethods]
impl Negate {
    #[classattr]
    fn __match_args__() -> (String,) {
        ("name".to_string(),)
    }

    #[new]
    #[pyo3(signature = (name, /))]
    fn py_new(name: Py<PyString>) -> Self {
        Self { name }
    }

    #[getter]
    fn get_name(&self, py: Python<'_>) -> Py<PyString> {
        self.name.clone_ref(py)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let name = self.name.bind_borrowed(py);
        Ok(format!("<Negate {}>", name.repr()?))
    }

    fn __getnewargs__(&self, py: Python<'_>) -> (Py<PyString>,) {
        (self.name.clone_ref(py),)
    }

    fn __eq__(&self, py: Python<'_>, other: &Self) -> PyResult<bool> {
        self.name.bind(py).as_any().eq(other.name.bind(py))
    }

    fn __ne__(&self, py: Python<'_>, other: &Self) -> PyResult<bool> {
        self.name.bind(py).as_any().ne(other.name.bind(py))
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        let elements = [
            &Self::type_object(py).into_any(),
            self.name.bind(py).as_any(),
        ];
        PyTuple::new(py, elements)?.hash()
    }
}

impl Negate {
    pub fn new(name: Py<PyString>) -> Self {
        Self { name }
    }

    pub fn name(&self, py: Python<'_>) -> PyResult<PyBackedStr> {
        self.name.bind_borrowed(py).extract::<PyBackedStr>()
    }
}

#[derive(IntoPyObject, IntoPyObjectRef)]
pub enum Selection {
    #[pyo3(transparent)]
    Select(Py<Select>),
    #[pyo3(transparent)]
    Negate(Py<Negate>),
}

impl Selection {
    fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Selection::Select(select) => Selection::Select(select.clone_ref(py)),
            Selection::Negate(negate) => Selection::Negate(negate.clone_ref(py)),
        }
    }
}

impl FromPyObject<'_> for Selection {
    fn extract_bound(ob: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(select) = ob.downcast::<Select>() {
            Ok(Selection::Select(select.clone().unbind()))
        } else if let Ok(negate) = ob.downcast::<Negate>() {
            Ok(Selection::Negate(negate.clone().unbind()))
        } else {
            Err(DowncastError::new(ob, "Select | Negate").into())
        }
    }
}

impl Selection {
    fn select(name: Bound<'_, PyString>) -> PyResult<Self> {
        let select = Py::new(name.py(), Select::new(name.unbind()))?;
        Ok(Self::Select(select))
    }

    fn negate(name: Bound<'_, PyString>) -> PyResult<Self> {
        let negate = Py::new(name.py(), Negate::new(name.unbind()))?;
        Ok(Self::Negate(negate))
    }
}

#[pyclass(module = "crabwalk")]
struct SelectionsView {
    iter: Box<dyn Iterator<Item = Selection> + Send + Sync>,
}

impl SelectionsView {
    fn new(selections: Vec<Selection>) -> Self {
        Self {
            iter: Box::new(selections.into_iter()),
        }
    }
}

#[pymethods]
impl SelectionsView {
    fn __iter__(self_: PyRef<'_, Self>) -> PyRef<'_, Self> {
        self_
    }
    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        Ok(match self.iter.next() {
            Some(item) => Some(item.into_pyobject(py)?.unbind()),
            None => None,
        })
    }

    fn __repr__<'py>(&self, py: Python<'py>) -> &Bound<'py, PyString> {
        intern!(py, "<SelectionsView>")
    }
}

#[pyclass(module = "crabwalk", mapping)]
pub struct Types {
    types: Arc<Option<Py<PyDict>>>,
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
            types: Arc::new(Some(PyDict::new(py).unbind())),
            selections: Vec::new(),
        };
        instance.update(py, initial, kwargs)?;
        Ok(instance)
    }

    fn selections<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, SelectionsView>> {
        let selections: Vec<Selection> = self
            .selections
            .iter()
            .map(|selection| selection.clone_ref(py))
            .collect();
        Bound::new(py, SelectionsView::new(selections))
    }

    pub fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        name: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let types: &Bound<'py, PyAny> = self.types.as_ref().as_ref().unwrap().bind(py);
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
            Err(err) if err.is_instance(py, &py.get_type::<PyKeyError>()) => {
                Ok(default.into_pyobject(py)?)
            }
            Err(err) => Err(err),
        }
    }

    pub fn __contains__(&self, py: Python<'_>, name: &Bound<'_, PyAny>) -> PyResult<bool> {
        match self.__getitem__(py, name) {
            Ok(_) => Ok(true),
            Err(err) if err.is_instance(py, &py.get_type::<PyKeyError>()) => Ok(false),
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

    fn __eq__(&self, py: Python<'_>, other: &Self) -> PyResult<bool> {
        if self.selections.len() != other.selections.len() {
            return Ok(false)
        }

        let types = self.types.as_ref().as_ref().unwrap().bind(py);
        let other_types = other.types.as_ref().as_ref().unwrap().bind(py);
        if types.ne(other_types)? {
            return Ok(false);
        }

        for (selection, other_selection) in zip(&self.selections, &other.selections) {
            use Selection::{Negate, Select};
            match (selection, other_selection) {
                (Select(_), Negate(_)) => return Ok(false),
                (Negate(_), Select(_)) => return Ok(false),
                (selection, other_selection) => {
                    let selection = selection.into_pyobject(py)?;
                    let other_selection = other_selection.into_pyobject(py)?;
                    if selection.ne(other_selection)? {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    pub fn __len__(&self, py: Python<'_>) -> usize {
        self.types.as_ref().as_ref().unwrap().bind(py).len()
    }

    pub fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyIterator>> {
        PyIterator::from_object(self.types.as_ref().as_ref().unwrap().bind(py)).map(Into::into)
    }

    pub fn __delitem__(&self, py: Python<'_>, name: &Bound<'_, PyAny>) -> PyResult<()> {
        self.types
            .as_ref()
            .as_ref()
            .unwrap()
            .bind(py)
            .del_item(name)
    }

    pub fn __setitem__(
        &self,
        py: Python<'_>,
        name: &str,
        globs: &Bound<'_, PySequence>,
    ) -> PyResult<()> {
        if globs.len()? == 0 {
            let types = self.types.as_ref().as_ref().unwrap().bind(py);
            let globs = PyList::empty(py);
            types.set_item(name, &globs)?;
        } else {
            for glob in globs.try_iter()? {
                self.add(py, name, glob?.downcast()?)?;
            }
        }
        Ok(())
    }

    #[pyo3(signature = (key, default=Maybe::Missing, /))]
    pub fn pop<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
        default: Maybe<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.__getitem__(py, key) {
            Ok(globs) => {
                self.__delitem__(py, key)?;
                Ok(globs.into_pyobject(py)?.into_any())
            }
            Err(err) if err.is_instance(py, &py.get_type::<PyKeyError>()) => match default {
                Maybe::Some(default) => Ok(default.into_pyobject(py)?),
                Maybe::Missing => Err(err),
            },
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
            .as_ref()
            .unwrap()
            .bind(py)
            .call_method0("popitem")?
            .extract()?;
        Ok((name, globs.as_sequence().to_tuple()?))
    }

    pub fn clear(&self, py: Python<'_>) {
        self.types.as_ref().as_ref().unwrap().bind(py).clear()
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
        let other = other.unwrap_or_else(|| PyTuple::empty(py).into_any());
        if let Ok(other) = other.downcast::<PyMapping>() {
            for name in other.try_iter()? {
                let name = &name?;
                self.__setitem__(
                    py,
                    &name.extract::<PyBackedStr>()?,
                    other.get_item(name)?.downcast()?,
                )?;
            }
        } else if other.hasattr("keys")? {
            for name in other.call_method0("keys")?.try_iter()? {
                let name = &name?;
                self.__setitem__(
                    py,
                    &name.extract::<PyBackedStr>()?,
                    other.get_item(name)?.downcast()?,
                )?;
            }
        } else {
            for item in other.try_iter()? {
                let (name, globs): (PyBackedStr, _) = item?.extract()?;
                self.__setitem__(py, &name, &globs)?;
            }
        }
        if let Some(kwargs) = kwargs {
            for (name, globs) in kwargs.iter() {
                self.__setitem__(py, &name.extract::<PyBackedStr>()?, &globs.extract()?)?;
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
        let default = default.unwrap_or_else(|| PyTuple::empty(py).extract().unwrap());
        match self.__getitem__(py, key) {
            Ok(globs) => Ok(globs),
            Err(err) if err.is_instance(py, &py.get_type::<PyKeyError>()) => {
                self.__setitem__(py, &key.extract::<PyBackedStr>()?, &default)?;
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
            return Err(IgnoreError::from(ignore::Error::InvalidDefinition).into());
        }
        let types = self.types.as_ref().as_ref().unwrap().bind(py);
        let globs: Bound<'_, PyList> = match types.get_item(name)? {
            Some(globs) => globs.downcast_into()?,
            None => {
                let globs = PyList::empty(py);
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
                self.add(py, definition.name(), glob.into_pyobject(py)?.downcast()?)?
            }
        }
        Ok(())
    }

    /// Select the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are selected.
    pub fn select(&mut self, py: Python<'_>, name: Bound<'_, PyString>) -> PyResult<()> {
        if name == "all" {
            for name in self.types.as_ref().as_ref().unwrap().bind(py).keys() {
                self.selections
                    .push(Selection::select(name.downcast_into()?)?);
            }
        } else {
            self.selections.push(Selection::select(name)?);
        }
        Ok(())
    }

    /// Ignore the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are negated.
    pub fn negate(&mut self, py: Python<'_>, name: Bound<'_, PyString>) -> PyResult<()> {
        if name == "all" {
            for name in self.types.as_ref().as_ref().unwrap().bind(py).keys() {
                self.selections
                    .push(Selection::negate(name.downcast_into()?)?);
            }
        } else {
            self.selections.push(Selection::negate(name)?);
        }
        Ok(())
    }

    fn __getnewargs__(&self, py: Python<'_>) -> (Py<PyDict>,) {
        (self.types.as_ref().as_ref().unwrap().clone_ref(py),)
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let state = PyDict::new(py);
        let selections = PyList::new(py, &self.selections)?;
        state.set_item(intern!(py, "selections"), selections)?;
        Ok(state.unbind())
    }

    fn __setstate__(&mut self, state: Bound<'_, PyDict>) -> PyResult<()> {
        let selections = <Bound<'_, PyAny>>::get_item(&state, "selections")?;
        self.selections = selections.extract()?;
        Ok(())
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(types) = self.types.as_ref() {
            visit.call(types)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.types = Arc::new(None);
    }
}
