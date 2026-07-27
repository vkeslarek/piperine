//! [`Design`] — a loaded, elaborated design and the navigation root of the
//! object model (CLA-17), plus [`Selection`]/[`Node`], the typed result of
//! resolving a hierarchical selector against it.
//!
//! The design is held behind `Rc` so every [`Module`] view can re-look its
//! module up on each call instead of borrowing the POM for its own lifetime.
//! That is also why the model is single-threaded: the POM's interior (its
//! staging area) is not `Sync`.

use std::collections::HashSet;
use std::rc::Rc;

use piperine_lang::pom::node::Node as PomNode;
use piperine_lang::pom::{Kinded, Named};
use piperine_lang::{parse_and_elaborate, SourceMap, Value};

use crate::error::Error;
use crate::model::module::Module;

/// A loaded, elaborated design — the root a host navigates from.
///
/// **The hierarchy this exposes is the authored one** (MD-25):
/// [`Design::modules`] yields the modules as written, and descending through
/// [`Instance::module`](crate::model::Instance::module) →
/// [`Design::module`] walks `instance → submodule → sub-instances` exactly as
/// the source spells it. Monomorphized variants appear under their concrete
/// names (`urc__5`) with their sub-instance trees intact. The flattened form
/// codegen builds for itself is a separate side artifact and is deliberately
/// unreachable from here — there is no accessor for it, and adding one would
/// break MD-25.
#[derive(Clone)]
pub struct Design {
    design: Rc<piperine_lang::Design>,
}

impl std::fmt::Debug for Design {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Design").field("top", &self.design.top().map(|m| m.name().to_string())).finish()
    }
}

impl Design {
    /// Load + elaborate the PHDL at `path` as a self-contained design
    /// (`SourceMap::dummy()`). A parse/elaboration failure or an unreadable
    /// file surfaces as [`Error::Model`] carrying the diagnostic.
    ///
    /// Project-aware loading is [`Self::load_with`]: resolving a
    /// `Piperine.toml` root into a `SourceMap` lives in `piperine-project`, and
    /// `piperine-api`'s dependency set is deliberately {lang, codegen, solver}
    /// (MD-20 — `crates/piperine-api/tests/smoke.rs` enforces it), so the host
    /// that already depends on `piperine-project` (the CLI, the Python binding)
    /// builds the map and hands it in.
    pub fn load(path: &str) -> Result<Self, Error> {
        Self::load_with(path, SourceMap::dummy())
    }

    /// [`Self::load`] with a caller-supplied `SourceMap` — the project-aware
    /// path: pass `piperine_project::project_source_map(&root)` to resolve
    /// dependency namespaces and the prelude the way the CLI does.
    pub fn load_with(path: &str, source_map: SourceMap) -> Result<Self, Error> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| Error::Model(format!("failed to read `{path}`: {e}")))?;
        Self::from_source(&source, source_map)
    }

    /// Elaborate `src` directly — no filesystem read, no project discovery
    /// (the same `SourceMap::dummy()` a project-less [`Self::load`] falls back
    /// to). Parse/elaboration failures surface as [`Error::Model`].
    pub fn load_str(src: &str) -> Result<Self, Error> {
        Self::from_source(src, SourceMap::dummy())
    }

    /// Wrap an already-elaborated, shared design — the entry point for a caller
    /// that elaborated the POM itself (the plugin hook bridge hands hook
    /// contexts the design the host is working on).
    pub fn from_shared(design: Rc<piperine_lang::Design>) -> Self {
        Self { design }
    }

    /// The shared POM handle — what [`Session`](crate::Session) and the
    /// staging surface take.
    pub fn shared(&self) -> Rc<piperine_lang::Design> {
        Rc::clone(&self.design)
    }

    /// The underlying POM, borrowed.
    pub fn pom(&self) -> &piperine_lang::Design {
        &self.design
    }

    /// Shared elaborate + top-inference recipe behind [`Self::load`] and
    /// [`Self::load_str`].
    fn from_source(source: &str, source_map: SourceMap) -> Result<Self, Error> {
        let mut design =
            parse_and_elaborate(source, &source_map).map_err(|e| Error::Model(format!("{e}")))?;
        if let Some(top) = Self::infer_top(&design) {
            design.set_top(&top);
        }
        Ok(Self { design: Rc::new(design) })
    }

    /// Infer the design's top module: the unique module that no other module
    /// instantiates (the board, vs. its leaf primitives). `None` when there is
    /// no unambiguous root (zero or several candidates) — the caller then
    /// leaves the top unset rather than guessing.
    fn infer_top(design: &piperine_lang::Design) -> Option<String> {
        let instantiated: HashSet<String> = design
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

    /// The elaborated top module, if one is set (either declared or inferred by
    /// [`Self::load`]/[`Self::load_str`]).
    pub fn top(&self) -> Option<Module> {
        self.design.top().map(|m| Module::new(self.shared(), m.name().to_string()))
    }

    /// Look up a module by name. Fails loud with [`Error::Model`] when the
    /// design declares no such module.
    pub fn module(&self, name: &str) -> Result<Module, Error> {
        if self.design.module(name).is_some() {
            Ok(Module::new(self.shared(), name.to_string()))
        } else {
            Err(Error::Model(format!("module `{name}` not found")))
        }
    }

    /// Every elaborated module, in the authored hierarchy (MD-25) — never the
    /// flattened side artifact.
    pub fn modules(&self) -> Vec<Module> {
        self.design.modules().map(|m| Module::new(self.shared(), m.name().to_string())).collect()
    }

    /// A global constant by name; `None` for an unknown name.
    pub fn const_(&self, name: &str) -> Option<Value> {
        self.design.const_(name).cloned()
    }

    /// Resolve a hierarchical selector path against the design (spec §13 Part
    /// IV selector). Returns the typed [`Selection`] of matched nodes.
    ///
    /// Path grammar follows the POM selector: `/`-separated steps, each `name`
    /// (default `inst` axis) or `axis::name` (`net`/`port`/`param`/`behavior`/
    /// `attr`). A leading `/` makes the path absolute, rooted at the top
    /// module.
    ///
    /// Fails loud, never a silent empty success: a malformed path is
    /// [`Error::Model`], a path that resolves to zero nodes is
    /// [`Error::NotFound`].
    pub fn select(&self, path: &str) -> Result<Selection, Error> {
        let selection = self.design.select(path).map_err(|e| Error::Model(format!("{e}")))?;
        if selection.is_empty() {
            return Err(Error::NotFound(format!("selector `{path}` resolved to no nodes")));
        }
        Ok(Selection { nodes: selection.iter().map(Node::of).collect() })
    }
}

/// The typed result of [`Design::select`]: a snapshot of the matched nodes'
/// `(kind, name)`, taken at resolution time because the POM's own `Node<'a>` is
/// borrowed from the design.
#[derive(Debug, Clone)]
pub struct Selection {
    nodes: Vec<Node>,
}

impl Selection {
    /// Number of matched nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// `true` when no nodes matched. [`Design::select`] fails loud before
    /// returning an empty selection; this stays for honest reflection if a
    /// selection is obtained another way.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The matched nodes, kind + name each.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}

/// One matched POM node from a selector resolution: its kind (`"module"`,
/// `"instance"`, `"port"`, `"param"`, `"wire"`, `"behavior"`, `"attribute"`, …)
/// and its name. Behaviors and attributes carry no name and report the empty
/// string.
#[derive(Debug, Clone)]
pub struct Node {
    kind: String,
    name: String,
}

impl Node {
    fn of(node: &PomNode<'_>) -> Self {
        Self { kind: node.kind().to_string(), name: node.name().to_string() }
    }

    /// The node's discriminator.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The node's declared name (label for instances); the empty string for
    /// behaviors and attributes.
    pub fn name(&self) -> &str {
        &self.name
    }
}
