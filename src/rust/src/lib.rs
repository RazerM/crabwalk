#![deny(rust_2018_idioms)]

use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::Path;
use std::ptr;

use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;
use pyo3::exceptions::{PyException, PyRuntimeError, PyTypeError};
use pyo3::once_cell::GILOnceCell;
use pyo3::prelude::*;
use pyo3::types::{PyList, PySequence, PyString, PyTraceback, PyTuple, PyType};
use pyo3::{ffi, AsPyPointer, PyTraverseError, PyTypeInfo, PyVisit};

use crate::direntry::DirEntry;
use crate::error::IntoPyErr;
use crate::types::{Selection, Types};
use crate::util::{fspath, fspath_list};

mod direntry;
mod error;
mod types;
mod util;

static TYPES_MODULE: GILOnceCell<Py<PyModule>> = GILOnceCell::new();
static KEYS_VIEW_TYPE: GILOnceCell<Py<PyType>> = GILOnceCell::new();
static ITEMS_VIEW_TYPE: GILOnceCell<Py<PyType>> = GILOnceCell::new();
static VALUES_VIEW_TYPE: GILOnceCell<Py<PyType>> = GILOnceCell::new();

enum State {
    Unopened,
    Opened,
    Started(Box<ignore::Walk>),
    Closed,
}

/// Walk is a recursive directory iterator over file paths in one or more directories.
#[pyclass(
    module = "crabwalk",
    text_signature = "(\
*paths, \
max_depth=None, \
follow_links=False, \
max_filesize=None, \
global_ignore_files=None, \
custom_ignore_filenames=None, \
overrides=None, \
types=None, \
hidden=True, \
parents=True, \
ignore=True, \
git_global=True, \
git_ignore=True, \
git_exclude=True, \
require_git=True, \
ignore_case_insensitive=False, \
sort=False, \
same_file_system=False, \
skip_stdout=False, \
filter_entry=None, \
onerror=None, \
)"
)]
pub struct Walk {
    state: State,
    paths: Option<Py<PyList>>, // Only None after tp_clear
    max_depth: Option<usize>,
    follow_links: bool,
    max_filesize: Option<u64>,
    global_ignore_files: Option<Py<PyList>>, // Only None after tp_clear
    custom_ignore_filenames: Option<Py<PyList>>, // Only None after tp_clear
    overrides: Option<PyObject>,
    types: Option<Py<Types>>,
    hidden: bool,
    parents: bool,
    ignore: bool,
    git_global: bool,
    git_ignore: bool,
    git_exclude: bool,
    require_git: bool,
    ignore_case_insensitive: bool,
    sort: Option<PyObject>,
    same_file_system: bool,
    skip_stdout: bool,
    filter_entry: Option<PyObject>,
    onerror: Option<PyObject>,
}

#[pymethods]
impl Walk {
    #[new]
    #[args(
        paths = "*",
        max_depth = "None",
        follow_links = "false",
        max_filesize = "None",
        global_ignore_files = "None",
        custom_ignore_filenames = "None",
        overrides = "None",
        types = "None",
        hidden = "true",
        parents = "true",
        ignore = "true",
        git_global = "true",
        git_ignore = "true",
        git_exclude = "true",
        require_git = "true",
        ignore_case_insensitive = "false",
        sort = "None",
        same_file_system = "false",
        skip_stdout = "false",
        filter_entry = "None",
        onerror = "None"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        paths: &PyTuple,
        max_depth: Option<usize>,
        follow_links: bool,
        max_filesize: Option<u64>,
        global_ignore_files: Option<&PySequence>,
        custom_ignore_filenames: Option<&PySequence>,
        overrides: Option<PyObject>,
        types: Option<Py<Types>>,
        hidden: bool,
        parents: bool,
        ignore: bool,
        git_global: bool,
        git_ignore: bool,
        git_exclude: bool,
        require_git: bool,
        ignore_case_insensitive: bool,
        sort: Option<PyObject>,
        same_file_system: bool,
        skip_stdout: bool,
        filter_entry: Option<PyObject>,
        onerror: Option<PyObject>,
    ) -> PyResult<Self> {
        let paths = PyList::new(py, paths);
        let global_ignore_files = match global_ignore_files {
            Some(seq) => Some(seq.list()?.into_py(py)),
            None => Some(PyList::empty(py).into_py(py)),
        };
        let custom_ignore_filenames = match custom_ignore_filenames {
            Some(seq) => Some(seq.list()?.into_py(py)),
            None => Some(PyList::empty(py).into_py(py)),
        };
        let sort = match sort {
            Some(sort) => {
                if sort.is_true(py)? {
                    Some(sort)
                } else {
                    None
                }
            }
            None => None,
        };
        let mut instance = Self {
            state: State::Unopened,
            paths: Some(paths.into_py(py)),
            max_depth,
            follow_links,
            max_filesize,
            global_ignore_files,
            custom_ignore_filenames,
            overrides: None,
            types,
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
            filter_entry: filter_entry.map(|f| f.into_py(py)),
            onerror: onerror.map(|c| c.into_py(py)),
        };
        if let Some(overrides) = overrides {
            instance.set_overrides(py, Some(overrides.as_ref(py)))?;
        }
        Ok(instance)
    }

    /// Disable the `hidden`, `parents`, `ignore`, `git_ignore`, `git_global`, and `git_exclude`
    /// filters.
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

    /// Enable the `hidden`, `parents`, `ignore`, `git_ignore`, `git_global`, and `git_exclude`
    /// filters.
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
    fn paths(&self) -> Py<PyList> {
        self.paths.clone().unwrap()
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
    fn follow_links(&self) -> bool {
        self.follow_links
    }

    #[setter]
    fn set_follow_links(&mut self, value: bool) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.follow_links = value;
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
    fn global_ignore_files(&self) -> Py<PyList> {
        self.global_ignore_files.clone().unwrap()
    }

    #[getter]
    fn custom_ignore_filenames(&self) -> Py<PyList> {
        self.custom_ignore_filenames.clone().unwrap()
    }

    #[getter]
    fn overrides(&self, py: Python<'_>) -> PyObject {
        self.overrides.to_object(py)
    }

    #[setter]
    fn set_overrides(&mut self, py: Python<'_>, overrides: Option<&PyAny>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.overrides = overrides
            .map(|overrides| {
                let types_mod = TYPES_MODULE.get(py).unwrap().as_ref(py);
                let overrides_type = types_mod.getattr("Overrides").unwrap().extract()?;
                if !overrides.is_instance(overrides_type)? {
                    return Err(PyTypeError::new_err(
                        "overrides must be an Overrides instance",
                    ));
                }
                Ok(overrides.into_py(py))
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn types(&self, py: Python<'_>) -> PyObject {
        self.types.to_object(py)
    }

    #[setter]
    fn set_types(&mut self, types: Option<Py<Types>>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.types = types;
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
    fn sort(&self, py: Python<'_>) -> PyObject {
        self.sort.clone().unwrap_or_else(|| false.into_py(py))
    }

    #[setter]
    fn set_sort(&mut self, py: Python<'_>, value: PyObject) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.sort = if value.is_true(py)? {
            Some(value)
        } else {
            None
        };
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
    fn filter_entry(&self) -> Option<PyObject> {
        self.filter_entry.clone()
    }

    #[setter]
    fn set_filter_entry(&mut self, value: Option<PyObject>) -> PyResult<()> {
        self.check_not_started_setter()?;
        self.filter_entry = value;
        Ok(())
    }

    #[getter]
    fn onerror(&self) -> Option<PyObject> {
        self.onerror.clone()
    }

    #[setter]
    fn set_onerror(&mut self, value: Option<PyObject>) -> PyResult<()> {
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

    #[pyo3(text_signature = "()")]
    fn close(&mut self) {
        self.state = State::Closed;
    }

    fn __exit__(
        &mut self,
        _exc_typepe: Option<&PyType>,
        _exc_val: Option<&PyException>,
        _exc_tb: Option<&PyTraceback>,
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

        loop {
            let dent = match py.allow_threads(|| walk.next()) {
                Some(dent) => dent,
                None => break,
            };

            if let Some(err) = PyErr::take(py) {
                // Don't pass user-caused errors through onerror, raise directly
                return Err(err);
            }

            match dent {
                Ok(dent) => {
                    if let Some(err) = dent.error() {
                        self.convert_and_call_onerror(py, err.clone())?;
                    }
                    return Ok(Some(DirEntry::new(dent)));
                }
                Err(err) => {
                    if let Some(onerror) = self.onerror.clone() {
                        convert_and_call_onerror(py, onerror.as_ref(py), err)?;
                    }
                }
            }
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
        if let Some(sort) = &self.sort {
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
        self.sort = None;
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
        let paths = self.paths.as_ref().unwrap().as_ref(py);
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

        let mut builder = ignore::WalkBuilder::new(&paths[0]);

        for path in paths.iter().skip(1) {
            builder.add(path);
        }

        builder
            .max_depth(self.max_depth)
            .follow_links(self.follow_links)
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

        for path in fspath_list(self.global_ignore_files.as_ref().unwrap().as_ref(py))? {
            if let Some(err) = builder.add_ignore(path) {
                self.convert_and_call_onerror(py, err)?;
            }
        }

        for path in self
            .custom_ignore_filenames
            .as_ref()
            .unwrap()
            .as_ref(py)
            .iter()
        {
            builder.add_custom_ignore_filename(path.extract::<OsString>()?);
        }

        if let Some(filter_entry) = self.filter_entry.clone() {
            builder.filter_entry(move |dent| {
                let py_dent = DirEntry::new(dent.clone());
                Python::with_gil(|py| {
                    filter_entry
                        .call1(py, (py_dent,))
                        .and_then(|result| result.is_true(py))
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

        if let Some(sort) = self.sort.clone() {
            if sort.as_ref(py).is_callable() {
                builder.sort_by_file_path(move |a, b| {
                    fn inner(sort_key: &PyObject, a: &Path, b: &Path) -> PyResult<Ordering> {
                        Python::with_gil(|py| {
                            let a = sort_key.call1(py, (a.as_os_str(),))?;
                            let b = sort_key.call1(py, (b.as_os_str(),))?;

                            let ra = a.as_ref(py);
                            let rb = b.as_ref(py);

                            Ok(match ra.gt(rb)? as i8 - ra.lt(rb)? as i8 {
                                -1 => Ordering::Less,
                                0 => Ordering::Equal,
                                1 => Ordering::Greater,
                                _ => unreachable!(),
                            })
                        })
                    }

                    inner(&sort, a, b).unwrap_or_else(|err| {
                        Python::with_gil(|py| {
                            if !PyErr::occurred(py) {
                                err.restore(py);
                            }
                        });
                        a.cmp(b)
                    })
                });
            } else {
                builder.sort_by_file_path(|a, b| a.cmp(b));
            }
        }

        if let Some(overrides) = self.overrides.clone() {
            let overrides = overrides.as_ref(py);
            let path: OsString = fspath(overrides.getattr("path")?)?.extract()?;
            let mut overrides_builder = OverrideBuilder::new(path);
            for override_ in overrides.iter()? {
                let override_ = override_?;
                let glob = override_.get_item(0)?.extract()?;
                let case_insensitive = override_.get_item(1)?.extract()?;
                overrides_builder
                    .case_insensitive(case_insensitive)
                    .map_err(|err| err.into_py_err(py))?;
                overrides_builder
                    .add(glob)
                    .map_err(|err| err.into_py_err(py))?;
            }
            let overrides = overrides_builder
                .build()
                .map_err(|err| err.into_py_err(py))?;
            builder.overrides(overrides);
        }

        if let Some(types) = &self.types {
            let types = types.borrow(py);
            let mut types_builder = TypesBuilder::new();
            for name in types.__iter__(py)?.as_ref(py) {
                let name = name?;
                let globs = types.__getitem__(py, name)?;
                for glob in globs.extract::<Vec<&str>>(py)? {
                    types_builder
                        .add(name.extract()?, glob)
                        .map_err(|err| err.into_py_err(py))?;
                }
            }
            for selection in &types.selections {
                match selection {
                    Selection::Select(name) => {
                        types_builder.select(name);
                    }
                    Selection::Negate(name) => {
                        types_builder.negate(name);
                    }
                }
            }
            let types = types_builder.build().map_err(|err| err.into_py_err(py))?;
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
        if let Some(onerror) = self.onerror.clone() {
            convert_and_call_onerror(py, onerror.as_ref(py), err)?;
        }
        Ok(())
    }
}

impl Drop for Walk {
    fn drop(&mut self) {
        if matches!(self.state, State::Started(_)) {
            Python::with_gil(|py| {
                // SAFETY: We're borrowing PyExc_ResourceWarning so need to incref
                let resource_warning_type =
                    unsafe { py.from_borrowed_ptr(ffi::PyExc_ResourceWarning) };

                // SAFETY: We're borrowing PyExc_Warning so need to incref
                let warning_type: &PyAny = unsafe { py.from_borrowed_ptr(ffi::PyExc_Warning) };

                // Note: Until pyo3 lets us implement tp_finalize, we can't use
                // PyErr_ResourceWarning, include the repr in the message, or pass the instance to
                // PyErr_WriteUnraisable for additional context.
                if let Err(err) =
                    PyErr::warn(py, resource_warning_type, "Unclosed Walk iterator", 1)
                {
                    if err.matches(py, warning_type) {
                        err.restore(py);
                        // SAFETY: NULL is an acceptable pointer when there is no available context.
                        unsafe { ffi::PyErr_WriteUnraisable(ptr::null_mut()) };
                    }
                }
            });
        }
    }
}

fn convert_and_call_onerror(py: Python<'_>, onerror: &PyAny, err: ignore::Error) -> PyResult<()> {
    onerror.call1((err.into_py_err(py),)).map(|_| ())
}

#[pymodule]
fn _lib(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    let mutable_mapping_type = py
        .import("collections.abc")?
        .getattr("MutableMapping")?
        .downcast::<PyType>()?;

    m.add_class::<Walk>()?;
    mutable_mapping_type.call_method1("register", (Types::type_object(py),))?;
    m.add_class::<Types>()?;
    m.add_class::<DirEntry>()?;

    let name: Py<PyString> = "_types".into_py(py);
    let globals = m.dict().as_ptr();

    let exc_mod: &PyModule = unsafe {
        let mod_ptr = ffi::PyImport_ImportModuleLevelObject(
            name.as_ptr(),
            globals,
            ptr::null_mut(),
            ptr::null_mut(),
            1,
        );
        py.from_owned_ptr_or_err(mod_ptr)?
    };

    let _ = TYPES_MODULE.set(py, exc_mod.into_py(py));

    let collections_abc_mod = py.import("collections.abc")?;

    let keys_view_typepe = collections_abc_mod
        .getattr("KeysView")?
        .downcast::<PyType>()?;

    let items_view_typepe = collections_abc_mod
        .getattr("ItemsView")?
        .downcast::<PyType>()?;

    let values_view_typepe = collections_abc_mod
        .getattr("ValuesView")?
        .downcast::<PyType>()?;

    let _ = KEYS_VIEW_TYPE.set(py, keys_view_typepe.into_py(py));
    let _ = ITEMS_VIEW_TYPE.set(py, items_view_typepe.into_py(py));
    let _ = VALUES_VIEW_TYPE.set(py, values_view_typepe.into_py(py));

    Ok(())
}
