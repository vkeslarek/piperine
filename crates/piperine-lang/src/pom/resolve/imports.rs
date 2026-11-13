//! Stage 3: `use` items, `self`/`super`/`crate`, visibility and prelude
//! sources. Imports are resolved on demand, with a guard against cycles, so
//! chains of aliases and globs that feed globs need no iteration order.

use std::collections::BTreeMap;

use super::super::body::{PathSegment, TypeKind};
use super::super::decl::DeclKind;
use super::super::design::Design;
use super::super::ids::{DeclId, PathId};
use super::super::resolution::{DeferReason, Resolution};
use super::{Ctx, Output};
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
}

/// An import being resolved: the namespace and the entry's place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Visit {
    namespace: DeclId,
    glob: bool,
    index: usize,
}

/// Why a path stopped resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WalkError {
    /// Segment `at` names nothing.
    Missing { at: usize },
    /// Segment `at` is an item that is not visible from here.
    Private { at: usize },
}

/// The outcome of walking a path: a resolution per segment (those after a
/// failure stay unresolved) and the first problem found.
#[derive(Debug)]
pub(super) struct Walked {
    pub res: Vec<Resolution>,
    pub error: Option<WalkError>,
}

impl Ctx<'_> {
    /// The nearest namespace strictly above `decl`.
    pub(super) fn parent_namespace(&self, decl: DeclId) -> Option<DeclId> {
        let mut at = self.design.decl_data(decl).parent;
        while let Some(d) = at {
            if matches!(self.design.decl_data(d).kind, DeclKind::Namespace(_)) {
                return Some(d);
            }
            at = self.design.decl_data(d).parent;
        }
        None
    }

    /// Is `from` the namespace `ns` or one nested inside it?
    fn within(&self, from: Option<DeclId>, ns: DeclId) -> bool {
        let mut at = from;
        while let Some(d) = at {
            if d == ns {
                return true;
            }
            at = self.design.decl_data(d).parent;
        }
        false
    }

    fn find_entry(table: &[(String, DeclId)], name: &str) -> Option<DeclId> {
        table.iter().find(|(n, _)| n == name).map(|(_, d)| *d)
    }

    /// A name in namespace `ns`: its own items, then its `use` names, then its
    /// globs. The flag says whether it is visible from `from`.
    pub(super) fn ns_item(&self, ns: DeclId, name: &str, prefer: Prefer, from: Option<DeclId>, visiting: &mut Vec<Visit>) -> Option<(DeclId, bool)> {
        let inside = self.within(from, ns);
        if let Some(table) = self.scopes.namespaces.get(&ns) {
            let (first, second) = match prefer {
                Prefer::Types => (&table.types, &table.values),
                Prefer::Values => (&table.values, &table.types),
            };
            if let Some(found) = Self::find_entry(first, name).or_else(|| Self::find_entry(second, name)) {
                return Some((found, inside || self.design.decl_data(found).public));
            }
        }
        let imports = self.imports.by_namespace.get(&ns)?;
        for (index, entry) in imports.named.iter().enumerate() {
            let visit = Visit { namespace: ns, glob: false, index };
            if entry.name != name || visiting.contains(&visit) {
                continue;
            }
            visiting.push(visit);
            let target = self.import_target(entry, ns, visiting);
            visiting.pop();
            if let Some(target) = target {
                return Some((target, inside || entry.public));
            }
        }
        for (index, entry) in imports.globs.iter().enumerate() {
            let visit = Visit { namespace: ns, glob: true, index };
            if visiting.contains(&visit) {
                continue;
            }
            visiting.push(visit);
            let found = self.glob_item(entry, ns, name, prefer, visiting);
            visiting.pop();
            if let Some(found) = found {
                return Some((found, inside || entry.public));
            }
        }
        None
    }

    /// What a glob brings under `name`: a public item of the namespace it
    /// names, or a variant of the enum it names.
    fn glob_item(&self, entry: &ImportEntry, ns: DeclId, name: &str, prefer: Prefer, visiting: &mut Vec<Visit>) -> Option<DeclId> {
        let target = self.import_target(entry, ns, visiting)?;
        match &self.design.decl_data(target).kind {
            DeclKind::Namespace(_) => {
                let (found, visible) = self.ns_item(target, name, prefer, Some(ns), visiting)?;
                visible.then_some(found)
            }
            DeclKind::Enum(_) => self.variant(target, name),
            _ => None,
        }
    }

    /// The variant `name` of enum `e`.
    pub(super) fn variant(&self, e: DeclId, name: &str) -> Option<DeclId> {
        self.design
            .decl_data(e)
            .children
            .iter()
            .copied()
            .find(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Variant(_)) && self.design.decl_data(*c).name.as_ref().is_some_and(|n| n.text == name))
    }

    /// The declaration an import names, wherever it is visible from.
    fn import_target(&self, entry: &ImportEntry, ns: DeclId, visiting: &mut Vec<Visit>) -> Option<DeclId> {
        let segments = &self.design.path_node(entry.path).segments;
        let walked = self.walk(segments, ns, Prefer::Types, visiting, &mut |this, name, visiting| this.scope_lookup(ns, name, Prefer::Types, visiting));
        match walked.res.last() {
            Some(Resolution::Decl(d)) => Some(*d),
            _ => None,
        }
    }

    /// A single name seen from inside namespace `ns`: its items and imports,
    /// then the prelude sources, then the built-in table.
    pub(super) fn scope_lookup(&self, ns: DeclId, name: &str, prefer: Prefer, visiting: &mut Vec<Visit>) -> Option<Resolution> {
        if let Some((found, _)) = self.ns_item(ns, name, prefer, Some(ns), visiting) {
            return Some(Resolution::Decl(found));
        }
        for root in &self.prelude {
            if let Some((found, true)) = self.ns_item(*root, name, prefer, None, visiting) {
                return Some(Resolution::Decl(found));
            }
        }
        super::super::builtin::builtin(name).map(Resolution::Builtin)
    }

    /// Resolve `segments` one by one. The first segment is `crate`, `self`,
    /// `super` or whatever `first` makes of its name; each later one is looked
    /// up in what the previous one named.
    pub(super) fn walk(
        &self,
        segments: &[PathSegment],
        ns: DeclId,
        last: Prefer,
        visiting: &mut Vec<Visit>,
        first: &mut dyn FnMut(&Self, &str, &mut Vec<Visit>) -> Option<Resolution>,
    ) -> Walked {
        let mut walked = Walked { res: vec![Resolution::Unresolved; segments.len()], error: None };
        let Some(head) = segments.first() else { return walked };
        let head_name = head.name.text.as_str();
        let start = match head_name {
            "crate" => Some(Resolution::Decl(self.root)),
            "self" if segments.len() > 1 => Some(Resolution::Decl(ns)),
            "super" => self.parent_namespace(ns).map(Resolution::Decl),
            _ => first(self, head_name, visiting),
        };
        let Some(mut current) = start else {
            walked.error = Some(WalkError::Missing { at: 0 });
            return walked;
        };
        walked.res[0] = current;
        for (i, segment) in segments.iter().enumerate().skip(1) {
            let name = segment.name.text.as_str();
            let next = match current {
                Resolution::Decl(d) => self.member(d, name, if i + 1 == segments.len() { last } else { Prefer::Types }, ns, visiting),
                // An associated item of a built-in type needs its type: left for inference.
                Resolution::Builtin(_) | Resolution::Deferred(_) => {
                    for later in &mut walked.res[i..] {
                        *later = Resolution::Deferred(DeferReason::ReceiverTypeUnknown);
                    }
                    return walked;
                }
                Resolution::Unresolved => None,
            };
            match next {
                Some((found, visible)) => {
                    if !visible && walked.error.is_none() {
                        walked.error = Some(WalkError::Private { at: i });
                    }
                    current = found;
                    walked.res[i] = current;
                }
                None => {
                    walked.error = Some(WalkError::Missing { at: i });
                    return walked;
                }
            }
        }
        walked
    }

    /// `name` after a segment that named `owner`: an item of a namespace, a
    /// variant of an enum, or `super` again.
    fn member(&self, owner: DeclId, name: &str, prefer: Prefer, from: DeclId, visiting: &mut Vec<Visit>) -> Option<(Resolution, bool)> {
        match &self.design.decl_data(owner).kind {
            DeclKind::Namespace(_) if name == "super" => self.parent_namespace(owner).map(|p| (Resolution::Decl(p), true)),
            DeclKind::Namespace(_) => {
                let (found, visible) = self.ns_item(owner, name, prefer, Some(from), visiting)?;
                Some((Resolution::Decl(found), visible))
            }
            DeclKind::Enum(_) => self.variant(owner, name).or_else(|| self.associated(owner, name)).map(|v| (Resolution::Decl(v), true)),
            DeclKind::Struct(_) | DeclKind::Nature(_) | DeclKind::Trait(_) | DeclKind::Module(_) => {
                self.associated(owner, name).map(|v| (Resolution::Decl(v), true))
            }
            _ => None,
        }
    }

    /// The associated constant or function `name` of `owner`: an item of one
    /// of its `impl` blocks, or of the trait itself.
    fn associated(&self, owner: DeclId, name: &str) -> Option<DeclId> {
        let design = self.design;
        let named = |item: &DeclId| {
            let d = design.decl_data(*item);
            matches!(d.kind, DeclKind::Fn(_) | DeclKind::Const(_)) && d.name.as_ref().is_some_and(|n| n.text == name)
        };
        let own = matches!(design.decl_data(owner).kind, DeclKind::Trait(_)).then_some(owner);
        own.iter().chain(self.impls.get(&owner).into_iter().flatten()).find_map(|holder| design.decl_data(*holder).children.iter().copied().find(named))
    }

    /// Every `impl` by the declaration its self type names.
    pub(super) fn collect_impls(&self) -> BTreeMap<DeclId, Vec<DeclId>> {
        let design = self.design;
        let mut impls: BTreeMap<DeclId, Vec<DeclId>> = BTreeMap::new();
        for i in 0..design.decl_count() {
            let id = DeclId(i as u32);
            let DeclKind::Impl(imp) = &design.decl_data(id).kind else { continue };
            let TypeKind::Path(path) = &design.type_node(imp.self_ty).kind else { continue };
            let Some(ns) = self.parent_namespace(id) else { continue };
            let node = design.path_node(*path);
            let walked = self.walk(&node.segments, ns, Prefer::Types, &mut Vec::new(), &mut |this, name, visiting| this.scope_lookup(ns, name, Prefer::Types, visiting));
            if let (None, Some(Resolution::Decl(target))) = (walked.error, walked.res.last()) {
                impls.entry(*target).or_default().push(id);
            }
        }
        impls
    }

    /// Write the resolution of every import path and report the ones that do
    /// not resolve or name a private item.
    pub(super) fn resolve_imports(&self, out: &mut Output) {
        for (ns, table) in &self.imports.by_namespace {
            for entry in table.named.iter().chain(&table.globs) {
                let node = self.design.path_node(entry.path);
                let walked = self.walk(&node.segments, *ns, Prefer::Types, &mut Vec::new(), &mut |this, name, visiting| this.scope_lookup(*ns, name, Prefer::Types, visiting));
                for (i, res) in walked.res.iter().enumerate() {
                    out.segments.push((entry.path, i, *res));
                }
                let file = self.design.decl_data(entry.use_decl).file;
                if let Some(error) = walked.error {
                    out.diagnostics.push((file, path_diagnostic(&node.segments, node.span, error)));
                }
            }
        }
    }
}

/// The `E0161` for a path that stopped at `error`.
pub(super) fn path_diagnostic(segments: &[PathSegment], span: crate::msl::Span, error: WalkError) -> Diagnostic {
    match error {
        WalkError::Missing { at } => {
            let name = &segments[at].name.text;
            Diagnostic::new("E0161", format!("cannot find `{name}` in this scope"), span, "not found")
        }
        WalkError::Private { at } => {
            let name = &segments[at].name.text;
            Diagnostic::new("E0161", format!("`{name}` is private"), span, "private item")
        }
    }
}
