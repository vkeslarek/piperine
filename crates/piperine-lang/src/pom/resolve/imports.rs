//! Stage 3: `use` items, `self`/`super`/`crate`, visibility and prelude
//! sources. Imports are resolved on demand, with a guard against cycles, so
//! chains of aliases and globs that feed globs need no iteration order.

use std::collections::BTreeMap;

use super::super::body::PathSegment;
use super::super::decl::DeclKind;
use super::super::design::Design;
use super::super::ids::{DeclId, PathId};
use super::super::resolution::Resolution;
use crate::msl::Diagnostic;

/// One leaf of a `use` tree, as the namespace that holds it sees it.
#[derive(Debug)]
pub(super) struct ImportEntry {
    /// The name it introduces; empty for a glob.
    pub name: String,
    pub path: PathId,
    pub use_decl: DeclId,
    /// `pub use`: the name is visible outside the namespace.
    pub public: bool,
}

#[derive(Debug, Default)]
pub(super) struct NsImports {
    pub named: Vec<ImportEntry>,
    pub globs: Vec<ImportEntry>,
}

#[derive(Debug, Default)]
pub(super) struct Imports {
    pub by_namespace: BTreeMap<DeclId, NsImports>,
}

impl Imports {
    pub fn build(design: &Design) -> Imports {
        let mut imports = Imports::default();
        for i in 0..design.decl_count() {
            let id = DeclId(i as u32);
            let data = design.decl_data(id);
            let DeclKind::Use(u) = &data.kind else { continue };
            let Some(parent) = data.parent else { continue };
            if !matches!(design.decl_data(parent).kind, DeclKind::Namespace(_)) {
                continue;
            }
            let table = imports.by_namespace.entry(parent).or_default();
            for import in &u.imports {
                let last = design.path_node(import.path).segments.last().map(|s| s.name.text.clone()).unwrap_or_default();
                let name = import.alias.as_ref().map_or(last, |a| a.text.clone());
                let entry = ImportEntry { name, path: import.path, use_decl: id, public: data.public };
                if import.glob {
                    table.globs.push(ImportEntry { name: String::new(), ..entry });
                } else {
                    table.named.push(entry);
                }
            }
        }
        imports
    }
}

/// Which table a lookup tries first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Prefer {
    Types,
    Values,
    /// The first segment of a path of several: the type namespace only.
    Head,
}

impl Prefer {
    /// What the last segment of a `use` path is looked up as: a glob names a namespace or an enum.
    pub(super) fn of_import(glob: bool) -> Prefer {
        if glob { Prefer::Head } else { Prefer::Types }
    }
}

/// An import being resolved: the namespace and the entry's place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Visit {
    pub(super) namespace: DeclId,
    pub(super) glob: bool,
    pub(super) index: usize,
}

/// Why a path stopped resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WalkError {
    /// Segment `at` names nothing.
    Missing { at: usize },
    /// Segment `at` is an item that is not visible from here.
    Private { at: usize },
    /// Segment `at` is provided by two glob imports for different declarations.
    Ambiguous { at: usize },
    /// Segment `at` of a `use` path names both a dependency and an item or import of the
    /// enclosing namespace (`E0163`).
    CrateClash { at: usize },
}

/// The outcome of walking a path: a resolution per segment (those after a
/// failure stay unresolved) and the first problem found.
#[derive(Debug)]
pub(super) struct Walked {
    pub res: Vec<Resolution>,
    pub error: Option<WalkError>,
}

impl WalkError {
    /// The `E0161` for a path that stopped at this error.
    pub(super) fn diagnostic(self, design: &Design, segments: &[PathSegment], span: crate::msl::Span) -> Diagnostic {
    match self {
            WalkError::Missing { at } => {
                let name = &segments[at].name.text;
                let diagnostic = Diagnostic::new("E0161", format!("cannot find `{name}` in this scope"), span, "not found");
                // A first segment that is a crate of the design but no dependency of this one.
                if at == 0 && design.crates.iter().any(|c| c.name == *name) {
                    return diagnostic.with_help("name it under [dependencies] in Piperine.toml");
                }
                // The names value semantics took out of the language (§1.4.6): nothing grants a copy.
                if matches!(name.as_str(), "Clone" | "Copy" | "clone") {
                    return diagnostic.with_help(crate::msl::diag::COPY_HELP);
                }
                // The name the explicit model of unknowns took out (§3.3): a value is always known.
                if name == "Unknown" {
                    return diagnostic.with_help(crate::msl::diag::UNKNOWN_HELP);
                }
                diagnostic
            }
            WalkError::Private { at } => {
                let name = &segments[at].name.text;
                Diagnostic::new("E0161", format!("`{name}` is private"), span, "private item")
            }
            WalkError::CrateClash { at } => {
                let name = &segments[at].name.text;
                Diagnostic::new("E0163", format!("`{name}` names both a dependency and an item of this namespace"), span, "ambiguous name").with_help(format!(
                    "rename the dependency in Piperine.toml (other = {{ package = \"{name}\", .. }}) or write crate::{name} for the local one"
                ))
            }
            WalkError::Ambiguous { at } => {
                let name = &segments[at].name.text;
                Diagnostic::new("E0162", format!("`{name}` is provided by two glob imports"), span, "ambiguous name")
                    .with_help("import the one you mean by name: `use path::name;`")
            }
        }
    }
}
