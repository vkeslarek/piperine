//! Stage 7: members. `e.name` is linked when the declarations alone decide
//! the type of `e`: an instance of a known module (its items), a value of a
//! declared struct or bundle type (its fields). Anything else is `Deferred`
//! and reports nothing: a type checker fills it later.

use std::collections::BTreeMap;

use super::super::body::{ExprKind, PatKind, StmtKind};
use super::super::decl::DeclKind;
use super::super::design::Design;
use super::super::ids::{DeclId, ExprId, FileId, TypeId};
use super::super::resolution::{CallClass, DeferReason, Resolution};
use crate::msl::Diagnostic;

/// What stage 7 computed, not yet written into the design.
#[derive(Default)]
pub(super) struct Members {
    /// `(field access or method call, what its member names)`.
    pub res: Vec<(ExprId, Resolution)>,
    pub diagnostics: Vec<(FileId, Diagnostic)>,
}

/// What the type of a receiver is, as far as declarations say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recv {
    /// An instance of this module: its items are the members.
    Instance(DeclId),
    /// A value of this struct (or bundle) type: its fields are the members.
    Struct(DeclId),
    /// Nothing declared tells.
    Unknown,
}

struct Reader<'a> {
    design: &'a Design,
    /// The annotated type of each `let` binding of a block.
    local_types: BTreeMap<DeclId, TypeId>,
}

pub(super) fn run(design: &Design) -> Members {
    let reader = Reader { design, local_types: local_types(design) };
    let mut out = Members::default();
    for i in 0..design.expr_count() {
        let expr = ExprId(i as u32);
        if let ExprKind::Field { expr: recv, name, .. } = &design.expr_node(expr).kind {
            let (res, error) = reader.field(*recv, &name.text);
            out.res.push((expr, res));
            if let Some(private) = error {
                let owner = design.expr_node(expr).owner;
                let message = format!("`{}` is not `pub` in module `{}`", name.text, design.decl_data(private).name.as_ref().map_or("", |n| n.text.as_str()));
                out.diagnostics.push((design.decl_data(owner).file, Diagnostic::new("E0343", message, name.span, "not `pub`")));
            }
        }
    }
    out
}

/// `let x: T = ..;` in a block: the binding `x` and its type.
fn local_types(design: &Design) -> BTreeMap<DeclId, TypeId> {
    let mut types = BTreeMap::new();
    for i in 0..design.stmt_count() {
        let StmtKind::Let { pat, ty: Some(ty), .. } = &design.stmt_node(super::super::ids::StmtId(i as u32)).kind else { continue };
        if let PatKind::Ident { binding, sub: None, .. } = &design.pat_node(*pat).kind {
            types.insert(*binding, *ty);
        }
    }
    types
}

impl Reader<'_> {
    /// The member `name` of the receiver expression: what it names, and the
    /// module whose private item it is when it is not `pub`.
    fn field(&self, recv: ExprId, name: &str) -> (Resolution, Option<DeclId>) {
        let deferred = (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), None);
        match self.receiver(recv) {
            Recv::Instance(module) => match self.member_of(module, name) {
                Some(item) if self.visible(item) => (Resolution::Decl(item), None),
                Some(item) => (Resolution::Decl(item), Some(module)),
                None => (Resolution::Unresolved, None),
            },
            Recv::Struct(s) => match child_field(self.design, s, name) {
                Some(f) => (Resolution::Decl(f), None),
                None => (Resolution::Unresolved, None),
            },
            Recv::Unknown => deferred,
        }
    }

    /// The type of a receiver chain: paths, fields, indexes and parentheses
    /// down to a root with a declared type.
    fn receiver(&self, expr: ExprId) -> Recv {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => self.receiver(*inner),
            ExprKind::Path(p) => match self.design.path_node(*p).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(d)) => self.of_decl(d),
                _ => Recv::Unknown,
            },
            ExprKind::Field { expr: inner, name, .. } => match self.receiver(*inner) {
                Recv::Instance(m) => self.member_of(m, &name.text).map_or(Recv::Unknown, |d| self.of_decl(d)),
                Recv::Struct(s) => child_field(self.design, s, &name.text).map_or(Recv::Unknown, |d| self.of_decl(d)),
                Recv::Unknown => Recv::Unknown,
            },
            _ => Recv::Unknown,
        }
    }

    /// What a declaration is a value of.
    fn of_decl(&self, d: DeclId) -> Recv {
        let design = self.design;
        // A body `let` that places a module of a known declaration is an instance of it.
        if let DeclKind::Binding(_) = &design.decl_data(d).kind {
            let placed = design.decl_data(d).parent.and_then(|p| match &design.decl_data(p).kind {
                DeclKind::Let(l) => match &design.expr_node(l.value).kind {
                    ExprKind::Call { class: CallClass::Instance { module: Some(m), .. }, .. } => Some(*m),
                    _ => None,
                },
                _ => None,
            });
            if let Some(m) = placed {
                return Recv::Instance(m);
            }
        }
        match self.local_types.get(&d).copied().or_else(|| design.declared_type(d)) {
            Some(ty) => self.of_type(ty),
            None => Recv::Unknown,
        }
    }

    fn of_type(&self, ty: TypeId) -> Recv {
        match self.design.type_resolution(ty) {
            Resolution::Decl(t) => match &self.design.decl_data(t).kind {
                DeclKind::Struct(_) => Recv::Struct(t),
                DeclKind::Module(_) => Recv::Instance(t),
                _ => Recv::Unknown,
            },
            _ => Recv::Unknown,
        }
    }

    /// The port, param, signal or `let` name `name` of a module, entering generation.
    fn member_of(&self, module: DeclId, name: &str) -> Option<DeclId> {
        fn find(design: &Design, parent: DeclId, name: &str) -> Option<DeclId> {
            for c in &design.decl_data(parent).children {
                let data = design.decl_data(*c);
                match &data.kind {
                    DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) if data.name.as_ref().is_some_and(|n| n.text == name) => return Some(*c),
                    DeclKind::Let(_) => {
                        if let Some(b) = data.children.iter().find(|b| design.decl_data(**b).name.as_ref().is_some_and(|n| n.text == name)) {
                            return Some(*b);
                        }
                    }
                    DeclKind::Generation(_) => {
                        if let Some(found) = find(design, *c, name) {
                            return Some(found);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        find(self.design, module, name)
    }

    /// Is an item of a module readable by its parent (§8.2.8)? A port is the
    /// module's interface and always is; a `let` name is as `pub` as its `let`.
    fn visible(&self, item: DeclId) -> bool {
        let data = self.design.decl_data(item);
        match &data.kind {
            DeclKind::Port(_) => true,
            DeclKind::Binding(_) => data.parent.is_some_and(|p| self.design.decl_data(p).public),
            _ => data.public,
        }
    }
}

/// The field `name` of a struct.
fn child_field(design: &Design, owner: DeclId, name: &str) -> Option<DeclId> {
    design.decl_data(owner).children.iter().copied().find(|c| {
        let d = design.decl_data(*c);
        matches!(d.kind, DeclKind::Field(_)) && d.name.as_ref().is_some_and(|n| n.text == name)
    })
}
