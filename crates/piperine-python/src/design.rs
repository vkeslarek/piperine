//! `_Design` — the loaded, elaborated design root exposed to Python (PY-01
//! load, PY-02 reflection). A thin wrapper over
//! [`piperine_api::model::Design`] (CLA-18): every method forwards to the api
//! model and maps its error onto the documented Python exception; the only
//! logic left here is project-aware `SourceMap` resolution (the api crate
//! deliberately does not depend on `piperine-project`, MD-20).

use std::path::Path;
use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;

use piperine_api::model::Selection as ApiSelection;
use piperine_lang::{Design, SourceMap};

use crate::module::_Module;
use crate::value_bridge::PyValue;

/// `_Design` — a loaded, elaborated design. Wraps the api model's
/// [`Design`](piperine_api::model::Design), which owns the shared
/// (refcounted) POM so child `_Module` views re-look it up on each call
/// without FFI lifetime fights. The Python facade re-exports this as
/// `Design`.
///
/// `unsendable`: the POM carries `Rc<RefCell<…>>` internally (the staging
/// area), so it is not `Sync`; the binding is single-interpreter, so the
/// `unsendable` pyclass (usable only from the interpreter's thread) is the
/// honest fit.
#[pyclass(module = "piperine", unsendable)]
pub struct _Design {
    inner: piperine_api::model::Design,
}

impl _Design {
    /// Wrap an already-elaborated, shared design (the plugin hook bridge
    /// hands hook contexts the design the host is working on).
    pub(crate) fn from_shared(design: Rc<Design>) -> Self {
        Self { inner: piperine_api::model::Design::from_shared(design) }
    }

    /// Load + elaborate the PHDL at `path` into a `_Design` (PY-01).
    ///
    /// The `SourceMap` is project-aware: when a `Piperine.toml` root is found
    /// above `path`, dependency namespaces + the prelude resolve as the CLI
    /// resolves them; otherwise a dummy map is used (self-contained designs
    /// still elaborate). Parse/elaboration failures surface as `ValueError`
    /// carrying the diagnostic; a missing/unreadable file surfaces the same way.
    pub(crate) fn load(path: &str) -> PyResult<Self> {
        let source_map = match Path::new(path)
            .parent()
            .and_then(piperine_project::find_project_root)
        {
            Some(root) => piperine_project::project_source_map(&root),
            None => SourceMap::dummy(),
        };
        piperine_api::model::Design::load_with(path, source_map)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    /// Elaborate `src` directly (HOST-24, `pip.load_str`) — no filesystem
    /// read, no project discovery (a standalone/self-contained design, the
    /// same `SourceMap::dummy()` a project-less `load` falls back to).
    /// Parse/elaboration failures surface as `ValueError`, same as `load`.
    pub(crate) fn load_str(src: &str) -> PyResult<Self> {
        piperine_api::model::Design::load_str(src)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }
}

#[pymethods]
impl _Design {
    /// The elaborated top module, if one is set (PY-02).
    fn top(&self) -> Option<_Module> {
        self.inner.top().map(_Module::new)
    }

    /// Look up a module by name; raises `ValueError` if absent (PY-02).
    fn module(&self, name: &str) -> PyResult<_Module> {
        self.inner.module(name).map(_Module::new).map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    /// Every elaborated module (PY-02).
    fn modules(&self) -> Vec<_Module> {
        self.inner.modules().into_iter().map(_Module::new).collect()
    }

    /// A global constant by name — scalars map to native Python values, other
    /// value kinds fall back to their string form, and an unknown name yields
    /// `None`. Read-only reflection starter (PY-02).
    fn const_(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        match self.inner.const_(name) {
            Some(value) => PyValue(&value).to_object(py),
            None => Ok(py.None()),
        }
    }

    /// Resolve a hierarchical selector path against the design (PY-14 / spec
    /// §13 Part IV selector). Returns a typed [`_Selection`] of the matched
    /// nodes; an unresolved path (zero matches) raises `KeyError` and a
    /// malformed path raises `ValueError` — fail loud, never a silent empty
    /// success (spec edge cases).
    ///
    /// Path grammar follows the POM selector (`piperine-lang/pom/selector`):
    /// `/`-separated steps, each `name` (default `inst` axis) or
    /// `axis::name` (`net`/`port`/`param`/`behavior`/`attr`). A leading `/`
    /// makes the path absolute — rooted at the inferred top module.
    fn select(&self, path: &str) -> PyResult<_Selection> {
        self.inner
            .select(path)
            .map(|selection| _Selection::of(&selection))
            .map_err(|e| match e {
                piperine_api::Error::NotFound(msg) => PyKeyError::new_err(msg),
                other => PyValueError::new_err(format!("{other}")),
            })
    }
}

// ── selector result ──────────────────────────────────────────────────────────

/// `_Selection` — the typed result of [`_Design::select`] (PY-14): the
/// matched nodes' `(kind, name)`, snapshotted by the api model at resolution
/// time and mirrored here one for one.
#[pyclass(module = "piperine")]
pub struct _Selection {
    nodes: Vec<_Node>,
}

impl _Selection {
    /// Mirror an api [`Selection`](piperine_api::model::Selection) into
    /// owned `_Node`s.
    fn of(selection: &ApiSelection) -> Self {
        Self { nodes: selection.nodes().iter().map(_Node::of).collect() }
    }
}

#[pymethods]
impl _Selection {
    /// Number of matched nodes.
    fn len(&self) -> usize {
        self.nodes.len()
    }

    /// `true` when no nodes matched. (`_Design::select` raises `KeyError`
    /// before returning an empty selection; this is kept for honest
    /// reflection if a selection is obtained another way later.)
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The matched nodes as a list of typed `_Node` objects (kind + name).
    fn nodes(&self) -> Vec<_Node> {
        self.nodes.iter().map(_Node::clone_snapshot).collect()
    }
}

/// `_Node` — one matched POM node from a selector resolution: its kind
/// (`"module"`, `"instance"`, `"port"`, ...) and its name. Behaviors and
/// attributes carry no name and surface the empty string.
#[pyclass(module = "piperine")]
pub struct _Node {
    kind: String,
    name: String,
}

impl _Node {
    fn of(node: &piperine_api::model::Node) -> Self {
        Self { kind: node.kind().to_string(), name: node.name().to_string() }
    }

    /// Clone a snapshot — pyclasses are not `Clone` by default; we hand-copy
    /// the two owned strings so `nodes()` can return fresh wrappers.
    fn clone_snapshot(other: &_Node) -> Self {
        Self { kind: other.kind.clone(), name: other.name.clone() }
    }
}

#[pymethods]
impl _Node {
    /// The node's discriminator: `"module"`, `"instance"`, `"port"`,
    /// `"param"`, `"wire"`, `"behavior"`, `"attribute"`, ... (PY-14).
    #[getter]
    fn kind(&self) -> String {
        self.kind.clone()
    }

    /// The node's declared name (label for instances); the empty string for
    /// behaviors and attributes, which carry no name.
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
}
