//! Stage 2: per-namespace type and value tables and per-module member tables.
//! A table keeps every entry in source order; a lookup of a repeated name
//! returns the first declaration.

use std::collections::BTreeMap;

use super::super::builtin::is_core_primitive;
use super::super::decl::{DeclKind, FieldsForm};
use super::super::design::Design;
use super::super::ids::{DeclId, FileId};
use crate::msl::Diagnostic;

/// The named items of one namespace, split by what they are used as. A
/// hardware module is in both (§3.4.5).
#[derive(Debug, Default)]
pub(super) struct NsTable {
    pub types: Vec<(String, DeclId)>,
    pub values: Vec<(String, DeclId)>,
}

pub(super) struct Scopes {
    /// By namespace declaration.
    pub namespaces: BTreeMap<DeclId, NsTable>,
    /// Ports, params, signals and body `let` names of each module, in source
    /// order, through generation.
    pub members: BTreeMap<DeclId, Vec<(String, DeclId)>>,
}

impl Scopes {
    pub fn build(design: &Design) -> Scopes {
        let mut scopes = Scopes { namespaces: BTreeMap::new(), members: BTreeMap::new() };
        for i in 0..design.decl_count() {
            let id = DeclId(i as u32);
            match &design.decl_data(id).kind {
                DeclKind::Namespace(_) => {
                    scopes.namespaces.insert(id, namespace_table(design, id));
                }
                DeclKind::Module(_) => {
                    let mut entries = Vec::new();
                    collect_members(design, id, &mut entries);
                    scopes.members.insert(id, entries);
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
            let mut seen: Vec<DeclId> = Vec::new();
            for (name, decl) in table.types.iter().chain(&table.values) {
                if is_core_primitive(name) && !seen.contains(decl) {
                    seen.push(*decl);
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
            out.extend(duplicates(design, *ns, table));
        }
        for (module, entries) in &self.members {
            let module_name = design.decl_data(*module).name.as_ref().map_or("", |n| n.text.as_str());
            for (i, (name, decl)) in entries.iter().enumerate() {
                let Some((_, first)) = entries[..i].iter().find(|(n, _)| n == name) else { continue };
                let span_of = |d: DeclId| {
                    let data = design.decl_data(d);
                    data.name.as_ref().map_or(data.span, |n| n.span)
                };
                let message = format!("`{name}` is defined more than once in module `{module_name}`");
                let diagnostic = Diagnostic::new("E0330", message, span_of(*decl), "defined again here")
                    .with_label(span_of(*first), "first defined here");
                out.push((design.decl_data(*module).file, diagnostic));
            }
        }
        out
    }
}

/// `E0162` for each declaration that repeats a name of its table, at its name,
/// with a label at the first. A declaration in both tables is reported once.
/// Two functions are an overload: the parser reports them as `E0143`.
fn duplicates(design: &Design, ns: DeclId, table: &NsTable) -> Vec<(FileId, Diagnostic)> {
    let mut out = Vec::new();
    let mut reported: Vec<DeclId> = Vec::new();
    for entries in [&table.types, &table.values] {
        for (i, (name, decl)) in entries.iter().enumerate() {
            let Some((_, first)) = entries[..i].iter().find(|(n, d)| n == name && d != decl) else { continue };
            let is_fn = |d: DeclId| matches!(design.decl_data(d).kind, DeclKind::Fn(_));
            if (is_fn(*decl) && is_fn(*first)) || reported.contains(decl) {
                continue;
            }
            reported.push(*decl);
            let span_of = |d: DeclId| {
                let data = design.decl_data(d);
                data.name.as_ref().map_or(data.span, |n| n.span)
            };
            let message = format!("`{name}` is defined more than once in this namespace");
            let diagnostic = Diagnostic::new("E0162", message, span_of(*decl), "defined again here").with_label(span_of(*first), "first defined here");
            out.push((design.decl_data(ns).file, diagnostic));
        }
    }
    out
}

/// The named children of a namespace.
fn namespace_table(design: &Design, ns: DeclId) -> NsTable {
    let mut table = NsTable::default();
    for child in &design.decl_data(ns).children {
        let data = design.decl_data(*child);
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
/// generations (their loop bindings are not members).
fn collect_members(design: &Design, parent: DeclId, out: &mut Vec<(String, DeclId)>) {
    for child in &design.decl_data(parent).children {
        let data = design.decl_data(*child);
        match &data.kind {
            DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) => {
                if let Some(name) = &data.name {
                    out.push((name.text.clone(), *child));
                }
            }
            DeclKind::Let(_) => {
                for binding in &data.children {
                    let b = design.decl_data(*binding);
                    if let (DeclKind::Binding(_), Some(name)) = (&b.kind, &b.name) {
                        out.push((name.text.clone(), *binding));
                    }
                }
            }
            DeclKind::Generation(_) => collect_members(design, *child, out),
            _ => {}
        }
    }
}
