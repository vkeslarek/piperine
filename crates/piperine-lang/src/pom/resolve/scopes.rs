//! Stage 2: per-namespace type and value tables and per-module member tables.
//! A table keeps every entry in source order; a lookup of a repeated name
//! returns the first declaration.

use std::collections::{BTreeMap, BTreeSet};

use super::super::builtin::is_core_primitive;
use super::super::decl::{DeclKind, FieldsForm};
use super::super::design::Design;
use super::super::ids::{DeclId, FileId};
use crate::msl::Diagnostic;

/// Named entries in source order, with a sorted map from each name to its
/// first entry: the vector keeps what diagnostics need, the map answers a
/// lookup without a scan.
#[derive(Debug, Default)]
pub(super) struct Named {
    pub entries: Vec<(String, DeclId)>,
    first: BTreeMap<String, DeclId>,
}

impl Named {
    pub fn push(&mut self, entry: (String, DeclId)) {
        self.first.entry(entry.0.clone()).or_insert(entry.1);
        self.entries.push(entry);
    }

    /// The first declaration named `name`.
    pub fn get(&self, name: &str) -> Option<DeclId> {
        self.first.get(name).copied()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (String, DeclId)> {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a Named {
    type Item = &'a (String, DeclId);
    type IntoIter = std::slice::Iter<'a, (String, DeclId)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// The named items of one namespace, split by what they are used as. A
/// hardware module is in both (§3.4.5).
#[derive(Debug, Default)]
pub(super) struct NsTable {
    pub types: Named,
    pub values: Named,
}

pub(super) struct Scopes {
    /// By namespace declaration.
    pub namespaces: BTreeMap<DeclId, NsTable>,
    /// Ports, params, signals and body `let` names of each module, in source
    /// order, through generation.
    pub members: BTreeMap<DeclId, Named>,
    /// The generic parameters of each declaration that has any, by name.
    pub generics: BTreeMap<DeclId, Named>,
}

impl Scopes {
    pub fn build(design: &Design) -> Scopes {
        let mut scopes = Scopes { namespaces: BTreeMap::new(), members: BTreeMap::new(), generics: BTreeMap::new() };
        for i in 0..design.decl_count() {
            let id = DeclId(i as u32);
            match &design.decl_data(id).kind {
                DeclKind::Namespace(_) => {
                    scopes.namespaces.insert(id, ScopeCollector::new(design).namespace_table(id));
                }
                DeclKind::Module(_) => {
                    let mut entries = Named::default();
                    ScopeCollector::new(design).collect_members(id, &mut entries);
                    scopes.members.insert(id, entries);
                }
                DeclKind::GenericParam(_) => {
                    let data = design.decl_data(id);
                    if let (Some(owner), Some(name)) = (data.parent, &data.name) {
                        scopes.generics.entry(owner).or_default().push((name.text.clone(), id));
                    }
                }
                _ => {}
            }
        }
        scopes
    }

    /// `E0457` for a namespace item named like a core primitive, `E0330` for a
    /// repeated member name.
    pub fn diagnostics(&self, design: &Design) -> Vec<(FileId, Diagnostic)> {
        let mut out = Vec::new();
        for (ns, table) in &self.namespaces {
            let mut seen: BTreeSet<DeclId> = BTreeSet::new();
            for (name, decl) in table.types.iter().chain(&table.values) {
                if is_core_primitive(name) && seen.insert(*decl) {
                    let data = design.decl_data(*decl);
                    let span = data.name.as_ref().map_or(data.span, |n| n.span);
                    let message = format!("`{name}` is a core primitive and cannot be redefined (§14 rule 1)");
                    out.push((design.decl_data(*ns).file, Diagnostic::new("E0457", message, span, "core name")));
                }
            }
        }
        // Source order across the namespaces' tables.
        out.sort_by_key(|(_, d)| d.span().start);
        for (ns, table) in &self.namespaces {
            out.extend(ScopeCollector::new(design).duplicates(*ns, table));
        }
        for (module, entries) in &self.members {
            let module_name = design.decl_data(*module).name.as_ref().map_or("", |n| n.text.as_str());
            let mut firsts: BTreeMap<&str, DeclId> = BTreeMap::new();
            for (name, decl) in entries {
                let Some(first) = ScopeCollector::first_of(&mut firsts, name, *decl) else { continue };
                let span_of = |d: DeclId| {
                    let data = design.decl_data(d);
                    data.name.as_ref().map_or(data.span, |n| n.span)
                };
                let message = format!("`{name}` is defined more than once in module `{module_name}`");
                let diagnostic = Diagnostic::new("E0330", message, span_of(*decl), "defined again here")
                    .with_label(span_of(first), "first defined here");
                out.push((design.decl_data(*module).file, diagnostic));
            }
        }
        out
    }
}

/// Collects the names each namespace and module of one design declares.
struct ScopeCollector<'design> {
    design: &'design Design,
}

impl<'design> ScopeCollector<'design> {
    /// A collector over `design`.
    fn new(design: &'design Design) -> Self {
        Self { design }
    }

    /// The declaration that first took `name`, or none when `decl` is it (and now recorded).
    fn first_of<'a>(firsts: &mut BTreeMap<&'a str, DeclId>, name: &'a str, decl: DeclId) -> Option<DeclId> {
        match firsts.get(name) {
            Some(first) => Some(*first),
            None => {
                firsts.insert(name, decl);
                None
            }
        }
    }


    /// `E0162` for each declaration that repeats a name of its table, at its name,
    /// with a label at the first. A declaration in both tables is reported once.
    /// Two functions are an overload: the parser reports them as `E0143`.
    fn duplicates(&self, ns: DeclId, table: &NsTable) -> Vec<(FileId, Diagnostic)> {
        let mut out = Vec::new();
        let mut reported: BTreeSet<DeclId> = BTreeSet::new();
        for entries in [&table.types, &table.values] {
            let mut firsts: BTreeMap<&str, DeclId> = BTreeMap::new();
            for (name, decl) in entries {
                let Some(first) = Self::first_of(&mut firsts, name, *decl) else { continue };
                if first == *decl {
                    continue;
                }
                let is_fn = |d: DeclId| matches!(self.design.decl_data(d).kind, DeclKind::Fn(_));
                if (is_fn(*decl) && is_fn(first)) || !reported.insert(*decl) {
                    continue;
                }
                let span_of = |d: DeclId| {
                    let data = self.design.decl_data(d);
                    data.name.as_ref().map_or(data.span, |n| n.span)
                };
                let message = format!("`{name}` is defined more than once in this namespace");
                let diagnostic = Diagnostic::new("E0162", message, span_of(*decl), "defined again here").with_label(span_of(first), "first defined here");
                out.push((self.design.decl_data(ns).file, diagnostic));
            }
        }
        out
    }


    /// The named children of a namespace.
    fn namespace_table(&self, ns: DeclId) -> NsTable {
        let mut table = NsTable::default();
        for child in &self.design.decl_data(ns).children {
            let data = self.design.decl_data(*child);
            let Some(name) = &data.name else { continue };
            let entry = (name.text.clone(), *child);
            match &data.kind {
                // A unit struct is a value too, as a module is (§3.4.7).
                DeclKind::Struct(s) if s.form == FieldsForm::Unit => {
                    table.types.push(entry.clone());
                    table.values.push(entry);
                }
                DeclKind::Nature(_) | DeclKind::Struct(_) | DeclKind::Enum(_) | DeclKind::Trait(_) | DeclKind::Namespace(_) => {
                    table.types.push(entry)
                }
                DeclKind::Fn(_) | DeclKind::Const(_) => table.values.push(entry),
                DeclKind::Module(_) => {
                    table.types.push(entry.clone());
                    table.values.push(entry);
                }
                _ => {}
            }
        }
        table
    }


    /// The ports, params, signals and `let` names under `parent`, entering
    /// generations (their loop bindings are not members). A `let` contributes
    /// the names of its pattern, not those bound inside its value.
    fn collect_members(&self, parent: DeclId, out: &mut Named) {
        for child in &self.design.decl_data(parent).children {
            let data = self.design.decl_data(*child);
            match &data.kind {
                DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) => {
                    if let Some(name) = &data.name {
                        out.push((name.text.clone(), *child));
                    }
                }
                DeclKind::Let(l) => {
                    // Only the names the `let`'s own pattern binds: a `match` arm or an `if let` in
                    // its value binds names of that arm alone, which are no members.
                    let pat = self.design.pat_node(l.pat).span;
                    for binding in &data.children {
                        let b = self.design.decl_data(*binding);
                        if let (DeclKind::Binding(_), Some(name)) = (&b.kind, &b.name)
                            && b.span.start >= pat.start
                            && b.span.end <= pat.end
                        {
                            out.push((name.text.clone(), *binding));
                        }
                    }
                }
                DeclKind::Generation(_) => self.collect_members(*child, out),
                _ => {}
            }
        }
    }
}
