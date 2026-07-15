//! `_Design` — the loaded, elaborated POM root exposed to Python
//! (PY-01 load, PY-02 reflection).

use std::path::Path;
use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;

use piperine_lang::pom::node::Node;
use piperine_lang::pom::{Kinded, Named};
use piperine_lang::{parse_and_elaborate, Design, SourceMap};

use crate::module::_Module;
use crate::value_bridge::PyValue;

/// `_Design` — a loaded, elaborated POM design. Owns a shared (refcounted)
/// `Design` so child `_Module` views can re-look it up on each call without
/// FFI lifetime fights (design `python-bindings/design.md` — POM borrow-
/// lifetime risk). The Python facade re-exports this as `Design`.
///
/// `unsendable`: `Design` carries `Rc<RefCell<…>>` internally (the staging
/// area), so it is not `Sync`; the binding is single-interpreter, so the
/// `unsendable` pyclass (usable only from the interpreter's thread) is the
/// honest fit.
#[pyclass(module = "piperine", unsendable)]
pub struct _Design {
    design: Rc<Design>,
}

impl _Design {
    /// Load + elaborate the PHDL at `path` into a `_Design` (PY-01).
    ///
    /// The `SourceMap` is project-aware: when a `Piperine.toml` root is found
    /// above `path`, dependency namespaces + the prelude resolve as the CLI
    /// resolves them; otherwise a dummy map is used (self-contained designs
    /// still elaborate). Parse/elaboration failures surface as `ValueError`
    /// carrying the diagnostic; a missing/unreadable file surfaces the same way.
    ///
    /// The top module is inferred when elaboration left it unset (the unique
    /// module no other module instantiates) so `top()` (AC2) and `select()`
    /// (PY-14) have a navigation root. Ambiguous roots leave it unset.
    pub(crate) fn load(path: &str) -> PyResult<Self> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| PyValueError::new_err(format!("failed to read `{path}`: {e}")))?;
        let source_map = match Path::new(path)
            .parent()
            .and_then(piperine_project::find_project_root)
        {
            Some(root) => piperine_project::project_source_map(&root),
            None => SourceMap::dummy(),
        };
        let mut design = parse_and_elaborate(&source, &source_map)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        if let Some(top) = Self::infer_top(&design) {
            design.set_top(&top);
        }
        Ok(Self {
            design: Rc::new(design),
        })
    }

    /// Infer the design's top module: the unique module that no other module
    /// instantiates (the board, vs. its leaf primitives). `None` when there is
    /// no unambiguous root (zero or several candidates) — the caller then
    /// leaves the top unset rather than guessing.
    fn infer_top(design: &Design) -> Option<String> {
        let instantiated: std::collections::HashSet<String> = design
            .modules()
            .flat_map(|m| m.instances().iter().map(|i| i.module_name().to_string()))
            .collect();
        let roots: Vec<String> = design
            .modules()
            .map(|m| m.name().to_string())
            .filter(|name| !instantiated.contains(name))
            .collect();
        match roots.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        }
    }

    /// A shared handle to the underlying POM — `_Module` borrows it per call.
    pub(crate) fn shared(&self) -> Rc<Design> {
        Rc::clone(&self.design)
    }
}

#[pymethods]
impl _Design {
    /// The elaborated top module, if one is set (PY-02).
    fn top(&self) -> Option<_Module> {
        self.design
            .top()
            .map(|m| _Module::new(self.shared(), m.name().to_string()))
    }

    /// Look up a module by name; raises `ValueError` if absent (PY-02).
    fn module(&self, name: &str) -> PyResult<_Module> {
        if self.design.module(name).is_some() {
            Ok(_Module::new(self.shared(), name.to_string()))
        } else {
            Err(PyValueError::new_err(format!("module `{name}` not found")))
        }
    }

    /// Every elaborated module (PY-02).
    fn modules(&self) -> Vec<_Module> {
        self.design
            .modules()
            .map(|m| _Module::new(self.shared(), m.name().to_string()))
            .collect()
    }

    /// A global constant by name — scalars map to native Python values, other
    /// value kinds fall back to their string form, and an unknown name yields
    /// `None`. Read-only reflection starter (PY-02).
    fn const_(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        match self.design.const_(name) {
            Some(value) => PyValue(value).to_object(py),
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
        let selection = self
            .design
            .select(path)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        if selection.is_empty() {
            return Err(PyKeyError::new_err(format!(
                "selector `{path}` resolved to no nodes"
            )));
        }
        Ok(_Selection::from_nodes(selection.iter()))
    }
}

// ── selector result ──────────────────────────────────────────────────────────

/// `_Selection` — the typed result of [`_Design::select`] (PY-14). A snapshot
/// of the matched nodes' `(kind, name)` taken at resolution time: the POM
/// `Node<'a>` is borrowed and cannot cross the FFI boundary, so — like
/// `_Port`/`_Net`/`_Instance` — each match is reflected into an owned
/// [`_Node`] at construction.
#[pyclass(module = "piperine")]
pub struct _Selection {
    nodes: Vec<_Node>,
}

impl _Selection {
    /// Snapshot a borrowed `NodeSelection` iterator into owned `_Node`s.
    fn from_nodes<'a, I>(nodes: I) -> Self
    where
        I: IntoIterator<Item = &'a Node<'a>>,
    {
        Self {
            nodes: nodes.into_iter().map(_Node::of).collect(),
        }
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
        self.nodes.iter().map(|n| _Node::clone_snapshot(n)).collect()
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
    fn of(node: &Node<'_>) -> Self {
        Self {
            kind: node.kind().to_string(),
            name: node.name().to_string(),
        }
    }

    /// Clone a snapshot — pyclasses are not `Clone` by default; we hand-copy
    /// the two owned strings so `nodes()` can return fresh wrappers.
    fn clone_snapshot(other: &_Node) -> Self {
        Self {
            kind: other.kind.clone(),
            name: other.name.clone(),
        }
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
