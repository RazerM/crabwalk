#![deny(rust_2018_idioms)]

use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::Path;

use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;
use pyo3::exceptions::{PyException, PyResourceWarning, PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedStr;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyList, PySequence, PyTraceback, PyTuple, PyType};
use pyo3::{BoundObject, PyTraverseError, PyVisit};

use crate::direntry::DirEntry;
use crate::error::IgnoreError;
use crate::util::{fspath, fspath_list};

mod direntry;
mod error;
mod types;
mod util;

static TYPES_MODULE: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static OS_STAT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The `crabwalk._types` module, imported on first use to avoid a circular
/// import (it imports this module).
fn types_module(py: Python<'_>) -> PyResult<&Bound<'_, PyModule>> {
    TYPES_MODULE
        .get_or_try_init(py, || Ok(py.import("crabwalk._types")?.unbind()))
        .map(|module| module.bind(py))
}

fn check_instance(value: &Bound<'_, PyAny>, type_name: &str, msg: &'static str) -> PyResult<()> {
    let expected = types_module(value.py())?.getattr(type_name)?;
    if !value.is_instance(&expected)? {
        return Err(PyTypeError::new_err(msg));
    }
    Ok(())
}

enum State {
    Unopened,
    Opened,
    Started(Box<ignore::Walk>),
    Closed,
}

#[pyclass(module = "crabwalk")]
pub struct Walk {
    state: State,
    paths: Option<Py<PyList>>, // Only None after tp_clear
    max_depth: Option<usize>,
    follow_symlinks: bool,
    max_filesize: Option<u64>,
    global_ignore_files: Option<Py<PyList>>, // Only None after tp_clear
    custom_ignore_filenames: Option<Py<PyList>>, // Only None after tp_clear
    overrides: Option<Py<PyAny>>,
    types: Option<Py<PyAny>>,
    hidden: bool,
    parents: bool,
    ignore: bool,
    git_global: bool,
    git_ignore: bool,
    git_exclude: bool,
    require_git: bool,
    ignore_case_insensitive: bool,
    sort: SortValue,
    same_file_system: bool,
    skip_stdout: bool,
    filter_entry: Option<Py<PyAny>>,
    onerror: Option<Py<PyAny>>,
}

#[pymethods]
impl Walk {
    #[new]
    #[pyo3(
        signature = (
            *paths,
            max_depth = None,
            follow_symlinks = false,
            max_filesize = None,
            global_ignore_files = None,
            custom_ignore_filenames = None,
            overrides = None,
            types = None,
            hidden = true,
            parents = true,
            ignore = true,
            git_global = true,
            git_ignore = true,
            git_exclude = true,
            require_git = true,
            ignore_case_insensitive = false,
            sort = None,
            same_file_system = false,
            skip_stdout = false,
            filter_entry = None,
            onerror = None
        )
    )]
    #[allow(clippy::too_many_arguments)]
    fn new<'py>(
        py: Python<'py>,
        paths: &Bound<'py, PyTuple>,
        max_depth: Option<usize>,
        follow_symlinks: bool,
        max_filesize: Option<u64>,
        global_ignore_files: Option<&Bound<'py, PySequence>>,
        custom_ignore_filenames: Option<&Bound<'py, PySequence>>,
        overrides: Option<Bound<'py, PyAny>>,
        types: Option<Bound<'py, PyAny>>,
        hidden: bool,
        parents: bool,
        ignore: bool,
        git_global: bool,
        git_ignore: bool,
        git_exclude: bool,
        require_git: bool,
        ignore_case_insensitive: bool,
        sort: Option<Bound<'py, PyAny>>,
        same_file_system: bool,
        skip_stdout: bool,
        filter_entry: Option<Py<PyAny>>,
        onerror: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let paths = PyList::new(py, paths)?;
        let global_ignore_files = match global_ignore_files {
            Some(seq) => Some(seq.to_list()?.unbind()),
            None => Some(PyList::empty(py).unbind()),
        };
        let custom_ignore_filenames = match custom_ignore_filenames {
            Some(seq) => Some(seq.to_list()?.unbind()),
            None => Some(PyList::empty(py).unbind()),
        };
        let sort = match sort {
            Some(sort) => SortValue::new(sort)?,
            None => SortValue::Bool(false),
        };
        let mut instance = Self {
            state: State::Unopened,
            paths: Some(paths.unbind()),
            max_depth,
            follow_symlinks,
            max_filesize,
            global_ignore_files,
            custom_ignore_filenames,
            overrides: None,
            types: None,
            hidden,
            parents,
            ignore,
            git_global,
            git_ignore,
            git_exclude,
            require_git,
            ignore_case_insensitive,
            sort,
            same_file_system,
            skip_stdout,
            filter_entry,
            onerror,
        };
        instance.set_overrides(overrides)?;
        instance.set_types(types)?;
        Ok(instance)
    }

    fn disable_standard_filters(&mut self) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.hidden = false;
        self.parents = false;
        self.ignore = false;
        self.git_ignore = false;
        self.git_global = false;
        self.git_exclude = false;
        Ok(())
    }

    fn enable_standard_filters(&mut self) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.hidden = true;
        self.parents = true;
        self.ignore = true;
        self.git_ignore = true;
        self.git_global = true;
        self.git_exclude = true;
        Ok(())
    }

    #[getter]
    fn paths(&self, py: Python<'_>) -> Py<PyList> {
        self.paths
            .as_ref()
            .map(|paths| paths.clone_ref(py))
            .unwrap()
    }

    #[getter]
    fn max_depth(&self) -> Option<usize> {
        self.max_depth
    }

    #[setter]
    fn set_max_depth(&mut self, value: Option<usize>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.max_depth = value;
        Ok(())
    }

    #[getter]
    fn follow_symlinks(&self) -> bool {
        self.follow_symlinks
    }

    #[setter]
    fn set_follow_symlinks(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.follow_symlinks = value;
        Ok(())
    }

    #[getter]
    fn max_filesize(&self) -> Option<u64> {
        self.max_filesize
    }

    #[setter]
    fn set_max_filesize(&mut self, value: Option<u64>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.max_filesize = value;
        Ok(())
    }

    #[getter]
    fn global_ignore_files(&self, py: Python<'_>) -> Py<PyList> {
        self.global_ignore_files
            .as_ref()
            .map(|global_ignore_files| global_ignore_files.clone_ref(py))
            .unwrap()
    }

    #[getter]
    fn custom_ignore_filenames(&self, py: Python<'_>) -> Py<PyList> {
        self.custom_ignore_filenames
            .as_ref()
            .map(|custom_ignore_filenames| custom_ignore_filenames.clone_ref(py))
            .unwrap()
    }

    #[getter]
    fn overrides<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.overrides.as_ref().into_pyobject(py)?)
    }

    #[setter]
    fn set_overrides(&mut self, overrides: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        self.check_not_started_setter()?;
        if let Some(overrides) = &overrides {
            check_instance(
                overrides,
                "Overrides",
                "overrides must be an Overrides instance",
            )?;
        }
        self.overrides = overrides.map(Bound::unbind);
        Ok(())
    }

    #[getter]
    fn types<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.types.as_ref().into_pyobject(py)?)
    }

    #[setter]
    fn set_types(&mut self, types: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        self.check_not_started_setter()?;
        if let Some(types) = &types {
            check_instance(types, "Types", "types must be a Types instance")?;
        }
        self.types = types.map(Bound::unbind);
        Ok(())
    }

    #[getter]
    fn hidden(&self) -> bool {
        self.hidden
    }

    #[setter]
    fn set_hidden(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.hidden = value;
        Ok(())
    }

    #[getter]
    fn parents(&self) -> bool {
        self.parents
    }

    #[setter]
    fn set_parents(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.parents = value;
        Ok(())
    }

    #[getter]
    fn ignore(&self) -> bool {
        self.ignore
    }

    #[setter]
    fn set_ignore(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.ignore = value;
        Ok(())
    }

    #[getter]
    fn git_global(&self) -> bool {
        self.git_global
    }

    #[setter]
    fn set_git_global(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.git_global = value;
        Ok(())
    }

    #[getter]
    fn git_ignore(&self) -> bool {
        self.git_ignore
    }

    #[setter]
    fn set_git_ignore(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.git_ignore = value;
        Ok(())
    }

    #[getter]
    fn git_exclude(&self) -> bool {
        self.git_exclude
    }

    #[setter]
    fn set_git_exclude(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.git_exclude = value;
        Ok(())
    }

    #[getter]
    fn require_git(&self) -> bool {
        self.require_git
    }

    #[setter]
    fn set_require_git(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.require_git = value;
        Ok(())
    }

    #[getter]
    fn ignore_case_insensitive(&self) -> bool {
        self.ignore_case_insensitive
    }

    #[setter]
    fn set_ignore_case_insensitive(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.ignore_case_insensitive = value;
        Ok(())
    }

    #[getter]
    fn sort(&self) -> &SortValue {
        &self.sort
    }

    #[setter]
    fn set_sort(&mut self, value: Bound<'_, PyAny>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.sort = SortValue::new(value)?;
        Ok(())
    }

    #[getter]
    fn same_file_system(&self) -> bool {
        self.same_file_system
    }

    #[setter]
    fn set_same_file_system(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.same_file_system = value;
        Ok(())
    }

    #[getter]
    fn skip_stdout(&self) -> bool {
        self.skip_stdout
    }

    #[setter]
    fn set_skip_stdout(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.skip_stdout = value;
        Ok(())
    }

    #[getter]
    fn filter_entry(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.filter_entry.as_ref().map(|f| f.clone_ref(py))
    }

    #[setter]
    fn set_filter_entry(&mut self, value: Option<Py<PyAny>>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.filter_entry = value;
        Ok(())
    }

    #[getter]
    fn onerror(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.onerror.as_ref().map(|onerror| onerror.clone_ref(py))
    }

    #[setter]
    fn set_onerror(&mut self, value: Option<Py<PyAny>>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.onerror = value;
        Ok(())
    }

    fn __enter__(mut self_: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        self_.state = match self_.state {
            State::Unopened => State::Opened,
            State::Opened | State::Started(_) => {
                return Err(PyRuntimeError::new_err(
                    "Walk context manager is not reentrant",
                ))
            }
            State::Closed => return Err(PyRuntimeError::new_err("Walk is closed")),
        };
        Ok(self_)
    }

    /// Close the iterator and free acquired resources
    ///
    /// It is recommended to use a ``with`` statement instead.
    fn close(&mut self) {
        self.state = State::Closed;
    }

    #[pyo3(signature = (_exc_type, _exc_val, _exc_tb))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyType>>,
        _exc_val: Option<&Bound<'_, PyException>>,
        _exc_tb: Option<&Bound<'_, PyTraceback>>,
    ) {
        self.close();
    }

    fn __iter__(self_: PyRef<'_, Self>) -> PyRef<'_, Self> {
        self_
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<DirEntry>> {
        self.ensure_started(py)?;
        let walk = match &mut self.state {
            State::Started(walk) => walk,
            _ => unreachable!(),
        };

        while let Some(dent) = py.detach(|| walk.next()) {
            if let Some(err) = PyErr::take(py) {
                // Don't pass user-caused errors through onerror, raise directly
                return Err(err);
            }

            match dent {
                Ok(dent) => {
                    if let Some(err) = dent.error() {
                        self.convert_and_call_onerror(py, err.clone())?;
                    }
                    return Ok(Some(DirEntry::new(dent, self.follow_symlinks)));
                }
                Err(err) => {
                    if let Some(onerror) = self.onerror.as_ref() {
                        convert_and_call_onerror(onerror.bind(py), err)?;
                    }
                }
            }
        }

        if let Some(err) = PyErr::take(py) {
            // Don't pass user-caused errors through onerror, raise directly
            return Err(err);
        }

        Ok(None)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(paths) = &self.paths {
            visit.call(paths)?;
        }
        if let Some(global_ignore_files) = &self.global_ignore_files {
            visit.call(global_ignore_files)?;
        }
        if let Some(custom_ignore_filenames) = &self.custom_ignore_filenames {
            visit.call(custom_ignore_filenames)?;
        }
        if let Some(overrides) = &self.overrides {
            visit.call(overrides)?;
        }
        if let Some(types) = &self.types {
            visit.call(types)?;
        }
        if let SortValue::Callable(sort) = &self.sort {
            visit.call(sort)?;
        }
        if let Some(filter_entry) = &self.filter_entry {
            visit.call(filter_entry)?;
        }
        if let Some(onerror) = &self.onerror {
            visit.call(onerror)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.paths = None;
        self.global_ignore_files = None;
        self.custom_ignore_filenames = None;
        self.overrides = None;
        self.types = None;
        self.sort = SortValue::Bool(false);
        self.filter_entry = None;
        self.onerror = None;
    }
}

impl Walk {
    fn check_not_started_setter(&self) -> PyResult<()> {
        if matches!(self.state, State::Started(_) | State::Closed) {
            return Err(PyRuntimeError::new_err(
                "This property is read-only once iteration has started",
            ));
        }
        Ok(())
    }

    fn build(&mut self, py: Python<'_>) -> PyResult<ignore::Walk> {
        let paths = self.paths.as_ref().unwrap().bind(py);
        if paths.is_empty() {
            return Err(PyTypeError::new_err("Must specify at least one path"));
        }
        let paths = fspath_list(paths)?;

        // The ignore crate treats "-" specially and just returns "<stdin>" if you try to walk it.
        let stdin = Path::new("-");

        for path in &paths {
            // This is ugly, can we fix this upstream?
            if path == stdin {
                return Err(PyTypeError::new_err(
                    "path cannot be '-', use './-' if you need this.",
                ));
            }
        }

        let mut builder = ignore::WalkBuilder::from_iter(&paths);

        builder
            .max_depth(self.max_depth)
            .follow_links(self.follow_symlinks)
            .max_filesize(self.max_filesize)
            .hidden(self.hidden)
            .parents(self.parents)
            .ignore(self.ignore)
            .git_global(self.git_global)
            .git_ignore(self.git_ignore)
            .git_exclude(self.git_exclude)
            .require_git(self.require_git)
            .ignore_case_insensitive(self.ignore_case_insensitive)
            .same_file_system(self.same_file_system)
            .skip_stdout(self.skip_stdout);

        for path in fspath_list(self.global_ignore_files.as_ref().unwrap().bind(py))? {
            if let Some(err) = builder.add_ignore(path) {
                self.convert_and_call_onerror(py, err)?;
            }
        }

        for path in self
            .custom_ignore_filenames
            .as_ref()
            .unwrap()
            .bind(py)
            .iter()
        {
            builder.add_custom_ignore_filename(path.extract::<OsString>()?);
        }

        if let Some(filter_entry) = self.filter_entry.as_ref().map(|f| f.clone_ref(py)) {
            let follow_symlinks = self.follow_symlinks;
            builder.filter_entry(move |dent| {
                let py_dent = DirEntry::new(dent.clone(), follow_symlinks);
                Python::attach(|py| {
                    filter_entry
                        .call1(py, (py_dent,))
                        .and_then(|result| result.is_truthy(py))
                        .unwrap_or_else(|err| {
                            if !PyErr::occurred(py) {
                                err.restore(py);
                            }

                            // Return true so that we reach the __next__ method where we can return
                            // the error can be raised
                            true
                        })
                })
            });
        }

        match &self.sort {
            SortValue::Callable(sort) => {
                let sort = sort.clone_ref(py);
                builder.sort_by_file_path(move |a, b| {
                    fn inner(sort_key: &Py<PyAny>, a: &Path, b: &Path) -> PyResult<Ordering> {
                        Python::attach(|py| {
                            let a = sort_key.call1(py, (a.as_os_str(),))?;
                            let b = sort_key.call1(py, (b.as_os_str(),))?;

                            let ra = a.bind(py);
                            let rb = b.bind(py);

                            Ok(match ra.gt(rb)? as i8 - ra.lt(rb)? as i8 {
                                -1 => Ordering::Less,
                                0 => Ordering::Equal,
                                1 => Ordering::Greater,
                                _ => unreachable!(),
                            })
                        })
                    }

                    inner(&sort, a, b).unwrap_or_else(|err| {
                        Python::attach(|py| {
                            if !PyErr::occurred(py) {
                                err.restore(py);
                            }
                        });
                        a.cmp(b)
                    })
                });
            }
            SortValue::Bool(true) => {
                builder.sort_by_file_path(|a, b| a.cmp(b));
            }
            SortValue::Bool(false) => {}
        }

        if let Some(overrides) = &self.overrides {
            let overrides = overrides.bind(py);
            let path: OsString = fspath(&overrides.getattr("path")?)?.extract()?;
            let mut overrides_builder = OverrideBuilder::new(path);
            for override_ in overrides.try_iter()? {
                let override_ = override_?;
                let x = override_.get_item(0)?;
                let glob = &*x.extract::<PyBackedStr>()?;
                let case_insensitive = override_.get_item(1)?.extract()?;
                overrides_builder
                    .case_insensitive(case_insensitive)
                    .map_err(IgnoreError::from)?;
                overrides_builder.add(glob).map_err(IgnoreError::from)?;
            }
            let overrides = overrides_builder.build().map_err(IgnoreError::from)?;
            builder.overrides(overrides);
        }

        if let Some(types) = &self.types {
            let types = types.bind(py);
            let mut types_builder = TypesBuilder::new();
            for item in types.call_method0("items")?.try_iter()? {
                let (name, globs): (PyBackedStr, Vec<PyBackedStr>) = item?.extract()?;
                for glob in &globs {
                    types_builder.add(&name, glob).map_err(IgnoreError::from)?;
                }
            }
            let select_type = types_module(py)?.getattr("Select")?;
            for selection in types.call_method0("selections")?.try_iter()? {
                let selection = selection?;
                let name: PyBackedStr = selection.getattr("name")?.extract()?;
                if selection.is_instance(&select_type)? {
                    types_builder.select(&name);
                } else {
                    types_builder.negate(&name);
                }
            }
            let types = types_builder.build().map_err(IgnoreError::from)?;
            builder.types(types);
        }

        Ok(builder.build())
    }

    fn ensure_started(&mut self, py: Python<'_>) -> PyResult<()> {
        match &self.state {
            State::Unopened | State::Opened => {
                self.state = State::Started(Box::new(self.build(py)?));
            }
            State::Closed => return Err(PyRuntimeError::new_err("Walk is closed")),
            State::Started(_) => (),
        };
        debug_assert!(matches!(self.state, State::Started(_)));
        Ok(())
    }

    fn convert_and_call_onerror(&self, py: Python<'_>, err: ignore::Error) -> PyResult<()> {
        if let Some(onerror) = self.onerror.as_ref().map(|onerror| onerror.clone_ref(py)) {
            convert_and_call_onerror(onerror.bind(py), err)?;
        }
        Ok(())
    }
}

impl Drop for Walk {
    fn drop(&mut self) {
        if matches!(self.state, State::Started(_)) {
            Python::attach(|py| {
                // Note: Until pyo3 lets us implement tp_finalize, we can't use
                // PyErr_ResourceWarning, include the repr in the message, or pass the instance to
                // PyErr_WriteUnraisable for additional context.
                let category = py.get_type::<PyResourceWarning>();
                if let Err(err) = PyErr::warn(py, &category, c"Unclosed Walk iterator", 1) {
                    err.write_unraisable(py, None);
                }
            });
        }
    }
}

enum SortValue {
    Callable(Py<PyAny>),
    Bool(bool),
}

impl SortValue {
    fn new(value: Bound<'_, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyBool>() {
            Ok(Self::Bool(value.extract()?))
        } else if value.is_callable() {
            Ok(Self::Callable(value.unbind()))
        } else {
            Err(PyTypeError::new_err("sort must be bool or Callable"))
        }
    }
}

impl<'py> IntoPyObject<'py> for SortValue {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self {
            Self::Callable(sort) => Ok(sort.into_bound(py)),
            Self::Bool(sort) => Ok(PyBool::new(py, sort).to_owned().into_any()),
        }
    }
}

impl<'a, 'py: 'a> IntoPyObject<'py> for &'a SortValue {
    type Target = PyAny;
    type Output = Borrowed<'a, 'py, Self::Target>;
    type Error = std::convert::Infallible;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self {
            SortValue::Callable(sort) => Ok(sort.bind_borrowed(py)),
            SortValue::Bool(sort) => Ok(PyBool::new(py, *sort).into_any()),
        }
    }
}

fn convert_and_call_onerror(onerror: &Bound<'_, PyAny>, err: ignore::Error) -> PyResult<()> {
    let _ = onerror.call1((PyErr::from(IgnoreError::from(err)),))?;
    Ok(())
}

#[pymodule]
fn _lib(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Walk>()?;
    m.add_class::<DirEntry>()?;
    m.add_function(wrap_pyfunction!(types::default_types, m)?)?;
    m.add_function(wrap_pyfunction!(types::validate_type_name, m)?)?;

    let _ = OS_STAT.set(py, py.import("os")?.getattr("stat")?.into());

    Ok(())
}
