//! The read-only state every resolution stage shares.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use super::super::body::{PathNode, PathSegment, TypeKind};
use super::super::builtin::builtin_member;
use super::super::decl::{DeclKind, GenericParamData};
use super::super::design::Design;
use super::super::ids::{DeclId, PathId};
use super::super::resolution::{DeferReason, Resolution};
use super::imports::{ImportEntry, Prefer, Visit, WalkError, Walked};
use super::Output;
use crate::msl::Diagnostic;

/// The read-only state every stage shares.
pub(super) struct Ctx<'a> {
    pub(super) design: &'a Design,
    pub(super) scopes: super::scopes::Scopes,
    pub(super) imports: super::imports::Imports,
    /// The namespace `prelude` of the standard library's root: its `pub` items are implicit.
    pub(super) prelude: Option<DeclId>,
    /// The dependency names of each crate (by `CrateId`) and the root namespace each names:
    /// the declared dependencies, and `std`.
    pub(super) deps: Vec<BTreeMap<String, DeclId>>,
    /// The module each behavior block is bound to (stage 4).
    pub(super) links: BTreeMap<DeclId, DeclId>,
    /// The `impl` blocks of each declaration, by the declaration their self
    /// type names (stage 5 reads associated items through them).
    pub(super) impls: BTreeMap<DeclId, Vec<DeclId>>,
    /// The access names quantities declare (`v`, `i`, ..): a call of one that
    /// no declaration names is an access call, not an unresolved name.
    pub(super) access: BTreeMap<String, Vec<DeclId>>,
    /// Set by a lookup that found a name through two glob imports of
    /// different declarations: the walk reports it as `E0162`, not `E0161`.
    pub(super) ambiguous: Cell<bool>,
}

impl<'a> Ctx<'a> {
    /// The context over a lowered `design`: its scopes, imports, prelude and dependency names.
    pub(super) fn new(design: &'a Design) -> Self {
        Ctx {
            design,
            scopes: super::scopes::Scopes::build(design),
            imports: super::imports::Imports::build(design),
            prelude: Self::std_prelude(design),
            deps: Self::dependency_names(design),
            links: BTreeMap::new(),
            impls: BTreeMap::new(),
            access: super::classify::Classifier::new(design).access_table(),
            ambiguous: Cell::new(false),
        }
    }

    /// The `pub` namespace `prelude` declared at the root of the standard library (its first file).
    fn std_prelude(design: &Design) -> Option<DeclId> {
        let root = design.crate_data(design.std_crate).root;
        design.decl_data(root).children.iter().copied().find(|c| {
            let data = design.decl_data(*c);
            data.public && matches!(data.kind, super::super::decl::DeclKind::Namespace(_)) && data.name.as_ref().is_some_and(|n| n.text == "prelude")
        })
    }

    /// The dependency names of every crate, each mapped to the root namespace of the crate it names.
    fn dependency_names(design: &Design) -> Vec<BTreeMap<String, DeclId>> {
        let std_root = design.crate_data(design.std_crate).root;
        (0..design.crates.len())
            .map(|i| {
                let data = design.crate_data(super::super::ids::CrateId(i as u32));
                let mut names: BTreeMap<String, DeclId> = data.dependencies.iter().map(|d| (d.name.clone(), design.crate_data(d.krate).root)).collect();
                names.insert("std".to_string(), std_root);
                names
            })
            .collect()
    }
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

    /// Is the trait `tr` in scope for code owned by `from` (Rust's "trait in scope"): some name
    /// in scope at the call resolves to its declaration, whatever the name — an item of the
    /// enclosing namespace, an import (by name, under an `as` alias, or through a glob) or a
    /// prelude item. A model item that takes the trait's name hides the name, not the trait.
    pub(super) fn trait_in_scope(&self, from: DeclId, tr: DeclId) -> bool {
        let ns = if matches!(self.design.decl_data(from).kind, DeclKind::Namespace(_)) { Some(from) } else { self.parent_namespace(from) };
        let (Some(ns), Some(name)) = (ns, self.design.decl_data(tr).name.as_ref()) else { return false };
        let name = name.text.as_str();
        if self.parent_namespace(tr) == Some(ns) {
            return true;
        }
        if let Some(imports) = self.imports.by_namespace.get(&ns) {
            let named = imports.named.iter().enumerate().any(|(index, entry)| self.import_target(entry, false, ns, &mut vec![Visit { namespace: ns, glob: false, index }]) == Some(tr));
            let globbed = imports.globs.iter().enumerate().any(|(index, entry)| {
                let mut visiting = vec![Visit { namespace: ns, glob: true, index }];
                self.import_target(entry, true, ns, &mut visiting).is_some_and(|target| self.glob_member(target, name, Prefer::Types, ns, &mut visiting) == Some(tr))
            });
            if named || globbed {
                return true;
            }
        }
        self.prelude.is_some_and(|prelude| self.ns_item(prelude, name, Prefer::Types, None, &mut Vec::new()) == Some((tr, true)))
    }

    /// The crate that holds the namespace `ns`.
    fn crate_of(&self, ns: DeclId) -> usize {
        self.design.file_data(self.design.decl_data(ns).file).krate.index()
    }

    /// The root namespace of the crate that holds `ns`: what `crate` names there.
    fn crate_root(&self, ns: DeclId) -> DeclId {
        self.design.crate_data(self.design.file_data(self.design.decl_data(ns).file).krate).root
    }

    /// Does the first segment of a `use` path in `ns` name both a dependency of its crate and an
    /// item or import of the namespace (§3.5 rule 5)? `visiting` holds the import being read, so
    /// it does not clash with itself.
    pub(super) fn use_clash(&self, ns: DeclId, segments: &[PathSegment], visiting: &mut Vec<Visit>) -> bool {
        self.clashing_item(ns, segments, visiting).is_some()
    }

    /// The item or import of `ns` that the first segment of a `use` path names besides a dependency
    /// of its crate (§3.5 rule 5): the other half of an `E0163`, `None` when there is no clash.
    pub(super) fn clashing_item(&self, ns: DeclId, segments: &[PathSegment], visiting: &mut Vec<Visit>) -> Option<DeclId> {
        let name = segments.first()?.name.text.as_str();
        if matches!(name, "crate" | "self" | "super") || !self.deps[self.crate_of(ns)].contains_key(name) {
            return None;
        }
        self.ns_item(ns, name, Prefer::Head, Some(ns), visiting).map(|(item, _visible)| item)
    }

    /// The `E0163` of a `use` path in `ns` whose first segment clashes with `local`: when `local` is
    /// an item the namespace itself declares under that name, in the same file, a secondary label
    /// points at it (an import that renames another item is not the item: no label).
    pub(super) fn clash_diagnostic(&self, ns: DeclId, local: DeclId, node: &PathNode) -> Diagnostic {
        let diagnostic = WalkError::CrateClash { at: 0 }.diagnostic(self.design, &node.segments, node.span);
        let item = self.design.decl_data(local);
        let same_file = item.file == self.design.decl_data(ns).file;
        match &item.name {
            Some(name) if same_file && item.parent == Some(ns) && node.segments.first().is_some_and(|head| head.name.text == name.text) => diagnostic.with_label(item.span, format!("the local item `{}`", name.text)),
            _ => diagnostic,
        }
    }

    /// Is `name` bound in `ns` by an explicit `use` that reported `E0163`? Such a name is an
    /// errored binding: a later read of it reports nothing further (as rustc reports once).
    pub(super) fn errored_import(&self, ns: DeclId, name: &str) -> bool {
        let Some(table) = self.imports.by_namespace.get(&ns) else { return false };
        table.named.iter().enumerate().any(|(index, entry)| {
            entry.name == name && self.use_clash(ns, &self.design.path_node(entry.path).segments, &mut vec![Visit { namespace: ns, glob: false, index }])
        })
    }

    /// Is `name` a dependency name of the crate that holds `ns`?
    pub(super) fn is_dependency_name(&self, ns: DeclId, name: &str) -> bool {
        self.deps[self.crate_of(ns)].contains_key(name)
    }

    /// Does the namespace `ns` have an item or an import called `name`?
    pub(super) fn has_ns_item(&self, ns: DeclId, name: &str) -> bool {
        self.ns_item(ns, name, Prefer::Head, Some(ns), &mut Vec::new()).is_some()
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

    fn find_entry(table: &super::scopes::Named, name: &str) -> Option<DeclId> {
        table.get(name)
    }

    /// A name in namespace `ns`: its own items, then its `use` names, then its
    /// globs. The flag says whether it is visible from `from`.
    pub(super) fn ns_item(&self, ns: DeclId, name: &str, prefer: Prefer, from: Option<DeclId>, visiting: &mut Vec<Visit>) -> Option<(DeclId, bool)> {
        let inside = self.within(from, ns);
        if let Some(table) = self.scopes.namespaces.get(&ns) {
            let (first, second) = match prefer {
                Prefer::Types => (&table.types, Some(&table.values)),
                Prefer::Values => (&table.values, Some(&table.types)),
                Prefer::Head => (&table.types, None),
            };
            if let Some(found) = Self::find_entry(first, name).or_else(|| second.and_then(|t| Self::find_entry(t, name))) {
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
            let target = self.import_target(entry, false, ns, visiting);
            visiting.pop();
            if let Some(target) = target.filter(|t| prefer != Prefer::Head || self.design.decl_data(*t).kind.occupies().0) {
                return Some((target, inside || entry.public));
            }
        }
        let mut found: Option<(DeclId, bool)> = None;
        for (index, entry) in imports.globs.iter().enumerate() {
            let visit = Visit { namespace: ns, glob: true, index };
            if visiting.contains(&visit) {
                continue;
            }
            visiting.push(visit);
            let hit = self.glob_item(entry, ns, name, prefer, visiting);
            visiting.pop();
            match (hit, found) {
                (Some(hit), None) => found = Some((hit, inside || entry.public)),
                // Two globs, two declarations: the name is ambiguous (§3.4.9).
                (Some(hit), Some((first, _))) if hit != first => {
                    self.ambiguous.set(true);
                    return None;
                }
                _ => {}
            }
        }
        found
    }

    /// What a glob brings under `name`: a public item of the namespace it
    /// names, or a variant of the enum it names.
    fn glob_item(&self, entry: &ImportEntry, ns: DeclId, name: &str, prefer: Prefer, visiting: &mut Vec<Visit>) -> Option<DeclId> {
        let target = self.import_target(entry, true, ns, visiting)?;
        self.glob_member(target, name, prefer, ns, visiting)
    }

    /// What a glob of `target` brings under `name`, seen from namespace `from`.
    pub(super) fn glob_member(&self, target: DeclId, name: &str, prefer: Prefer, from: DeclId, visiting: &mut Vec<Visit>) -> Option<DeclId> {
        match &self.design.decl_data(target).kind {
            DeclKind::Namespace(_) => {
                let (found, visible) = self.ns_item(target, name, prefer, Some(from), visiting)?;
                visible.then_some(found)
            }
            DeclKind::Enum(_) => self.variant(target, name).filter(|v| prefer != Prefer::Head || self.design.decl_data(*v).kind.occupies().0),
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
    fn import_target(&self, entry: &ImportEntry, glob: bool, ns: DeclId, visiting: &mut Vec<Visit>) -> Option<DeclId> {
        let segments = &self.design.path_node(entry.path).segments;
        if self.use_clash(ns, segments, visiting) {
            return None;
        }
        let walked = self.walk(segments, ns, Prefer::of_import(glob), visiting, &mut |this, name, prefer, visiting| this.scope_lookup(ns, name, prefer, visiting));
        match walked.res.last() {
            Some(Resolution::Decl(d)) => Some(*d),
            _ => None,
        }
    }

    /// A single name seen from inside namespace `ns`: its items and imports,
    /// then the `pub` items of `std::prelude`, then the built-in table.
    pub(super) fn scope_lookup(&self, ns: DeclId, name: &str, prefer: Prefer, visiting: &mut Vec<Visit>) -> Option<Resolution> {
        if let Some((found, _)) = self.ns_item(ns, name, prefer, Some(ns), visiting) {
            return Some(Resolution::Decl(found));
        }
        if self.ambiguous.get() {
            return None;
        }
        if let Some(root) = self.deps[self.crate_of(ns)].get(name) {
            return Some(Resolution::Decl(*root));
        }
        if let Some(prelude) = self.prelude {
            if let Some((found, true)) = self.ns_item(prelude, name, prefer, None, visiting) {
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
        first: &mut dyn FnMut(&Self, &str, Prefer, &mut Vec<Visit>) -> Option<Resolution>,
    ) -> Walked {
        let mut walked = Walked { res: vec![Resolution::Unresolved; segments.len()], error: None };
        self.ambiguous.set(false);
        let Some(head) = segments.first() else { return walked };
        let head_name = head.name.text.as_str();
        let start = match head_name {
            "crate" => Some(Resolution::Decl(self.crate_root(ns))),
            "self" if segments.len() > 1 => Some(Resolution::Decl(ns)),
            "super" => self.parent_namespace(ns).map(Resolution::Decl),
            // A head that starts a longer path names a namespace, an enum or a type: only the
            // type namespace can (rustc looks `util::` up there, so a local `fn util` is no clash).
            _ => first(self, head_name, if segments.len() > 1 { Prefer::Head } else { last }, visiting),
        };
        let Some(mut current) = start else {
            walked.error = Some(self.missing(0));
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
                    walked.error = Some(self.missing(i));
                    return walked;
                }
            }
        }
        walked
    }

    /// Segment `at` found nothing: ambiguous when a glob lookup said so.
    fn missing(&self, at: usize) -> WalkError {
        if self.ambiguous.replace(false) { WalkError::Ambiguous { at } } else { WalkError::Missing { at } }
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
            // `Mode::WIDTH` reads a member of `Repr` through the enum (§2); a variant or an item of
            // the enum's own named alike wins, and a carrier of an enum without `Repr` is `E0112`.
            DeclKind::Enum(_) => self
                .variant(owner, name)
                .or_else(|| self.associated(owner, name))
                .map(|v| (Resolution::Decl(v), true))
                .or_else(|| builtin_member("Repr", name).map(|member| (Resolution::Builtin(member), true)))
                .or_else(|| Self::default_member(name)),
            // `Self::NAME` in a trait: its own items, then each supertrait in the order written.
            DeclKind::Trait(t) => match self.associated(owner, name) {
                Some(item) => Some((Resolution::Decl(item), true)),
                None => self.through_bounds(&t.supertraits, name, self.parent_namespace(owner).unwrap_or(from), visiting),
            },
            DeclKind::Struct(_) => self.associated(owner, name).map(|v| (Resolution::Decl(v), true)).or_else(|| builtin_member("Repr", name).map(|member| (Resolution::Builtin(member), true)))
                .or_else(|| Self::default_member(name)),
            DeclKind::Nature(_) | DeclKind::Module(_) => self.associated(owner, name).map(|v| (Resolution::Decl(v), true)),
            DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) => self.through_bounds(bounds, name, from, visiting),
            _ => None,
        }
    }

    /// `Point::default()` reads `Default::default` through the type (App. H.8): left for the checker,
    /// which refuses a type that does not implement `Default`.
    fn default_member(name: &str) -> Option<(Resolution, bool)> {
        (name == "default").then_some((Resolution::Deferred(DeferReason::BuiltinTraitMember), true))
    }

    /// `name` after a generic: an item of a user trait it is bounded by, a
    /// member row of `Repr`, or - through any other built-in bound - left for
    /// inference. A generic bounded by `Repr` alone has no other member.
    fn through_bounds(&self, bounds: &[PathId], name: &str, from: DeclId, visiting: &mut Vec<Visit>) -> Option<(Resolution, bool)> {
        let mut only_repr = !bounds.is_empty();
        let mut deferred = false;
        for bound in bounds {
            let node = self.design.path_node(*bound);
            let walked = self.walk(&node.segments, from, Prefer::Types, visiting, &mut |this, n, prefer, visiting| this.scope_lookup(from, n, prefer, visiting));
            match walked.res.last() {
                Some(Resolution::Decl(t)) if matches!(self.design.decl_data(*t).kind, DeclKind::Trait(_)) => {
                    if let Some(item) = self.associated(*t, name) {
                        return Some((Resolution::Decl(item), true));
                    }
                    only_repr = false;
                }
                Some(Resolution::Builtin(b)) if b.info().name == "Repr" => {
                    if let Some(member) = builtin_member("Repr", name) {
                        return Some((Resolution::Builtin(member), true));
                    }
                }
                Some(Resolution::Builtin(_)) => {
                    only_repr = false;
                    deferred = true;
                }
                _ => only_repr = false,
            }
        }
        (deferred && !only_repr).then_some((Resolution::Deferred(DeferReason::BuiltinTraitMember), true))
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
            let walked = self.walk(&node.segments, ns, Prefer::Types, &mut Vec::new(), &mut |this, name, prefer, visiting| this.scope_lookup(ns, name, prefer, visiting));
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
            let mut named_targets = Vec::new();
            let entries = table.named.iter().enumerate().map(|(i, e)| (e, Visit { namespace: *ns, glob: false, index: i }));
            let entries = entries.chain(table.globs.iter().enumerate().map(|(i, e)| (e, Visit { namespace: *ns, glob: true, index: i })));
            for (entry, own) in entries {
                let node = self.design.path_node(entry.path);
                if let Some(local) = self.clashing_item(*ns, &node.segments, &mut vec![own]) {
                    let file = self.design.decl_data(entry.use_decl).file;
                    out.diagnostics.push((file, self.clash_diagnostic(*ns, local, node)));
                    continue;
                }
                let walked = self.walk(&node.segments, *ns, Prefer::of_import(own.glob), &mut Vec::new(), &mut |this, name, prefer, visiting| this.scope_lookup(*ns, name, prefer, visiting));
                for (i, res) in walked.res.iter().enumerate() {
                    out.segments.push((entry.path, i, *res));
                }
                if !entry.name.is_empty() {
                    named_targets.push((entry, walked.res.last().copied().unwrap_or(Resolution::Unresolved)));
                }
                let file = self.design.decl_data(entry.use_decl).file;
                if let Some(error) = walked.error {
                    out.diagnostics.push((file, error.diagnostic(self.design, &node.segments, node.span)));
                }
            }
            self.import_collisions(*ns, &named_targets, out);
        }
    }

    /// `E0162` at the later of an explicit `use` and an item, or of two
    /// explicit `use`s, that bring one name into one table (§3.4.8). An import of
    /// the very item it meets (`use a::{self}` beside `mod a`) is no second name.
    fn import_collisions(&self, ns: DeclId, named: &[(&ImportEntry, Resolution)], out: &mut Output) {
        let design = self.design;
        let mut entries: Vec<Taken> = Vec::new();
        if let Some(items) = self.scopes.namespaces.get(&ns) {
            let in_types: BTreeSet<DeclId> = items.types.iter().map(|(_, d)| *d).collect();
            let in_values: BTreeSet<DeclId> = items.values.iter().map(|(_, d)| *d).collect();
            let mut spans = BTreeSet::new();
            for (name, decl) in items.types.iter().chain(&items.values) {
                let data = design.decl_data(*decl);
                let span = data.name.as_ref().map_or(data.span, |n| n.span);
                if spans.insert((span.start, span.end)) {
                    let (in_types, in_values) = (in_types.contains(decl), in_values.contains(decl));
                    entries.push(Taken { name: name.clone(), types: in_types, values: in_values, span, import: false, decl: *decl });
                }
            }
        }
        for (entry, target) in named {
            let node = design.path_node(entry.path);
            let Resolution::Decl(target) = *target else { continue };
            let (types, values) = design.decl_data(target).kind.occupies();
            entries.push(Taken { name: entry.name.clone(), types, values, span: node.span, import: true, decl: target });
        }
        entries.sort_by_key(|e| e.span.start);
        let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, later) in entries.iter().enumerate() {
            let earlier = by_name.entry(later.name.as_str()).or_default();
            let first = earlier.iter().map(|j| &entries[*j]).find(|e| e.name == later.name && ((e.types && later.types) || (e.values && later.values)) && (e.import || later.import) && !(e.decl == later.decl && e.import != later.import));
            earlier.push(i);
            if let Some(first) = first {
                let message = format!("`{}` is defined more than once in this namespace", later.name);
                let diagnostic = Diagnostic::new("E0162", message, later.span, "defined again here").with_label(first.span, "first defined here");
                out.diagnostics.push((design.decl_data(ns).file, diagnostic));
            }
        }
    }
}


/// A name an item or an explicit `use` brings into a namespace, and the tables it takes.
struct Taken {
    name: String,
    types: bool,
    values: bool,
    span: crate::msl::Span,
    import: bool,
    /// The declaration it is, or for an import the one it names.
    decl: DeclId,
}
