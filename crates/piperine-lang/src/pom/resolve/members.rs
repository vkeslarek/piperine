//! Stage 7: members. `e.name` and `e.m()` are linked when the declarations
//! alone decide the type of `e`: an instance of a known module (its items), a
//! value of a declared struct or bundle type (its fields), a carrier (its
//! methods, in the order of §3.1.7). Anything else is `Deferred` and reports
//! nothing: a type checker fills it later.

use std::collections::BTreeMap;

use super::super::body::{ExprKind, PatKind, StmtKind, TypeKind};
use super::super::builtin::builtin_method;
use super::super::decl::{DeclKind, FnParamData, GenericParamData, NatureFieldNode};
use super::super::design::Design;
use super::super::ids::{DeclId, ExprId, FileId, PathId, TypeId};
use super::classify::access_table;
use super::super::resolution::{CallClass, DeferReason, Resolution};
use crate::msl::Diagnostic;

/// What stage 7 computed, not yet written into the design.
#[derive(Default)]
pub(super) struct Members {
    /// `(field access or method call, what its member names)`.
    pub res: Vec<(ExprId, Resolution)>,
    /// `(path, segment, what it names)`: the callee of an access call whose first node is a reached
    /// item (`v(c.mid)`), which classification could not give a nature (stage 6 leaves it ambiguous).
    pub segments: Vec<(PathId, usize, Resolution)>,
    pub diagnostics: Vec<(FileId, Diagnostic)>,
}

/// What the type of a receiver is, as far as declarations say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recv {
    /// An instance of this module: its items are the members.
    Instance(DeclId),
    /// A value of this struct (or bundle) type: its fields are the members.
    Struct(DeclId),
    /// A carrier of this nature; none for a generic bounded by `Nature`.
    Carrier(Option<DeclId>),
    /// An array of carriers, still `depth` indexes away from one carrier.
    CarrierArray(Option<DeclId>, usize),
    /// An array of values of this struct: its items are the struct's only after an index.
    StructArray(DeclId),
    /// An array of instances of this module: its items are the module's only after an index.
    InstanceArray(DeclId),
    /// Nothing declared tells.
    Unknown,
}

struct Reader<'a> {
    design: &'a Design,
    /// The annotated type of each `let` binding of a block.
    local_types: BTreeMap<DeclId, TypeId>,
    /// Whether a trait is in scope for the code of a declaration (blanket methods need it).
    in_scope: InScope<'a>,
}

/// Is the trait (second) in scope for code owned by the declaration (first)?
pub(super) type InScope<'a> = &'a dyn Fn(DeclId, DeclId) -> bool;

pub(super) fn run<'a>(design: &Design, in_scope: InScope<'a>) -> Members {
    let reader = Reader { design, local_types: local_types(design), in_scope };
    let mut out = Members::default();
    let access = access_table(design);
    let probe_operands = reader.probe_operands();
    for i in 0..design.expr_count() {
        let expr = ExprId(i as u32);
        if let Some(segment) = reader.reached_access(expr, &access) {
            out.segments.push(segment);
        }
        if let ExprKind::MethodCall { recv, method, .. } = &design.expr_node(expr).kind {
            let (res, on_array) = reader.method(*recv, &method.text, design.expr_node(expr).owner);
            out.res.push((expr, res));
            if on_array {
                let message = format!("the carrier method `{}` is applied to an array of carriers", method.text);
                let owner = design.expr_node(expr).owner;
                out.diagnostics.push((design.decl_data(owner).file, Diagnostic::new("E0211", message, method.span, "index the array first")));
            }
        }
        if let ExprKind::Field { expr: recv, name, .. } = &design.expr_node(expr).kind {
            let (res, error) = reader.field(*recv, &name.text);
            out.res.push((expr, res));
            if let Some((module, missing)) = error {
                let owner = design.expr_node(expr).owner;
                let module_name = design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str());
                let (code, message, label) = if missing {
                    ("E0343", format!("no item `{}` in module `{module_name}`", name.text), "no such item")
                } else if probe_operands.contains(&expr) {
                    // §6 rule 7: a probe or a contribution on a node the module cannot reach.
                    ("E0244", format!("the node `{}` of module `{module_name}` is not `pub`: the module cannot reach it (§6 rule 7)", name.text), "node out of reach")
                } else {
                    ("E0343", format!("`{}` is not `pub` in module `{module_name}`", name.text), "not `pub`")
                };
                out.diagnostics.push((design.decl_data(owner).file, Diagnostic::new(code, message, name.span, label)));
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
    /// The callee of the access call `expr` (`v(c.mid)`) when classification left it ambiguous and
    /// its first node is a `pub` port or signal of a placed child: the quantity of that item's
    /// nature, which the declarations alone decide (§8.2.8). The segment to write and what it names.
    fn reached_access(&self, expr: ExprId, access: &BTreeMap<String, Vec<DeclId>>) -> Option<(PathId, usize, Resolution)> {
        let ExprKind::Call { callee, args, .. } = &self.design.expr_node(expr).kind else { return None };
        let ExprKind::Path(path) = &self.design.expr_node(*callee).kind else { return None };
        let segment = self.design.path_node(*path).segments.last()?;
        if segment.res != Resolution::Deferred(DeferReason::AmbiguousAccess) {
            return None;
        }
        let ExprKind::Field { expr: recv, name, .. } = &self.design.expr_node(self.unparenthesized(args.first()?.value)).kind else { return None };
        let Recv::Instance(module) = self.receiver(*recv) else { return None };
        let item = self.member_of(module, &name.text)?;
        let nature = self.design.nature_of(self.design.declared_type(item)?)?;
        let quantity = access.get(&segment.name.text)?.iter().copied().find(|q| self.design.decl_data(*q).parent == Some(nature))?;
        Some((*path, self.design.path_node(*path).segments.len() - 1, Resolution::Decl(quantity)))
    }

    /// The nodes named by the arguments of every access call (`v(c.x)`, `i(a, c.x)`, `potential(..)`)
    /// as the expression of the node: what a probe or a contribution reads or drives (§6 rule 7).
    fn probe_operands(&self) -> std::collections::BTreeSet<ExprId> {
        let mut operands = std::collections::BTreeSet::new();
        for i in 0..self.design.expr_count() {
            let ExprKind::Call { callee, args, .. } = &self.design.expr_node(ExprId(i as u32)).kind else { continue };
            let ExprKind::Path(path) = &self.design.expr_node(*callee).kind else { continue };
            let is_access = match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Deferred(DeferReason::AmbiguousAccess)) => true,
                Some(Resolution::Decl(decl)) => matches!(self.design.decl_data(decl).kind, DeclKind::Quantity(_)),
                Some(Resolution::Builtin(id)) => matches!(id.info().name, "potential" | "flow"),
                _ => false,
            };
            if is_access {
                operands.extend(args.iter().map(|arg| self.unparenthesized(arg.value)));
            }
        }
        operands
    }

    /// `expr` with its parentheses peeled.
    fn unparenthesized(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    /// The member `name` of the receiver expression: what it names, and the
    /// module whose item it is when the item is not `pub` or does not exist
    /// (`true`): both are `E0343` (§8.4.8).
    fn field(&self, recv: ExprId, name: &str) -> (Resolution, Option<(DeclId, bool)>) {
        let deferred = (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), None);
        match self.receiver(recv) {
            Recv::Instance(module) => match self.member_of(module, name) {
                Some(item) if self.visible(item) => (Resolution::Decl(item), None),
                Some(item) => (Resolution::Decl(item), Some((module, false))),
                None => (Resolution::Unresolved, Some((module, true))),
            },
            Recv::Struct(s) => match child_field(self.design, s, name) {
                Some(f) => (Resolution::Decl(f), None),
                None => (Resolution::Unresolved, None),
            },
            // The fields of a carrier come from its value type, which needs inference.
            Recv::Carrier(_) | Recv::CarrierArray(..) | Recv::StructArray(_) | Recv::InstanceArray(_) | Recv::Unknown => deferred,
        }
    }

    /// The method `name` of the receiver: the carrier built-ins, then the
    /// `impl`s of its nature or bundle, then those of the value it reads. The
    /// flag is set for a carrier method applied to an array of carriers (`E0211`).
    fn method(&self, recv: ExprId, name: &str, owner: DeclId) -> (Resolution, bool) {
        let design = self.design;
        let in_impls = |target: DeclId| design.impls_of(target).into_iter().find_map(|i| child_fn(design, i, name)).map(Resolution::Decl);
        let carrier_method = |nature: Option<DeclId>| -> Option<Resolution> {
            builtin_method(name).map(Resolution::Builtin).or_else(|| {
                let nature = nature?;
                in_impls(nature).or_else(|| {
                    let DeclKind::Nature(n) = &design.decl_data(nature).kind else { return None };
                    let ty = n.fields.iter().find_map(|f| match f {
                        NatureFieldNode::Value { ty, .. } => Some(*ty),
                        _ => None,
                    })?;
                    match design.type_resolution(ty) {
                        Resolution::Decl(value) => in_impls(value),
                        _ => None,
                    }
                })
            })
            // Last: a blanket `impl<T: Repr> Tr for T`, which every carrier satisfies (§4).
            .or_else(|| self.blanket_method(name, owner).map(Resolution::Decl))
        };
        let found = match self.receiver(recv) {
            Recv::Carrier(nature) => carrier_method(nature),
            Recv::CarrierArray(nature, _) => {
                return match carrier_method(nature) {
                    Some(_) => (Resolution::Unresolved, true),
                    None => (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), false),
                };
            }
            Recv::Struct(s) => in_impls(s),
            Recv::Instance(_) => None,
            Recv::StructArray(_) | Recv::InstanceArray(_) | Recv::Unknown => return (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), false),
        };
        (found.unwrap_or(Resolution::Unresolved), false)
    }

    /// The method `name` of a trait that a blanket `impl<T: B> Tr for T` gives every carrier, `B`
    /// absent or the core `Repr` (the value type of a nature is `Repr`). The impls are read in
    /// declaration order (two visible ones: the first wins, G.67); only a trait in scope for the
    /// code of `owner` counts; no trait is solved, so any other bound is left to a type checker.
    fn blanket_method(&self, name: &str, owner: DeclId) -> Option<DeclId> {
        let design = self.design;
        (0..design.decl_count()).map(|i| DeclId(i as u32)).find_map(|imp| {
            let DeclKind::Impl(i) = &design.decl_data(imp).kind else { return None };
            let TypeKind::Path(self_path) = &design.type_node(i.self_ty).kind else { return None };
            let Some(Resolution::Decl(generic)) = design.path_node(*self_path).segments.last().map(|s| s.res) else { return None };
            let DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) = &design.decl_data(generic).kind else { return None };
            let only_repr = bounds.iter().all(|b| matches!(design.path_node(*b).segments.last().map(|s| s.res), Some(Resolution::Builtin(r)) if r.info().name == "Repr"));
            if !only_repr || design.decl_data(generic).parent != Some(imp) {
                return None;
            }
            let Some(Resolution::Decl(tr)) = i.trait_.and_then(|t| design.path_node(t).segments.last().map(|s| s.res)) else { return None };
            if !matches!(design.decl_data(tr).kind, DeclKind::Trait(_)) {
                return None;
            }
            if !(self.in_scope)(owner, tr) {
                return None;
            }
            child_fn(design, tr, name)
        })
    }

    /// The type of a receiver chain: paths, fields, indexes and parentheses
    /// down to a root with a declared type.
    fn receiver(&self, expr: ExprId) -> Recv {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.receiver(*inner),
            // An index peels one array level; any other receiver is read as it is.
            ExprKind::Index { expr: inner, .. } => match self.receiver(*inner) {
                Recv::CarrierArray(nature, 1) => Recv::Carrier(nature),
                Recv::CarrierArray(nature, depth) => Recv::CarrierArray(nature, depth - 1),
                Recv::StructArray(s) => Recv::Struct(s),
                Recv::InstanceArray(m) => Recv::Instance(m),
                other => other,
            },
            ExprKind::Path(p) => match self.design.path_node(*p).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(d)) => self.of_decl(d),
                _ => Recv::Unknown,
            },
            ExprKind::Field { expr: inner, name, .. } => match self.receiver(*inner) {
                Recv::Instance(m) => self.member_of(m, &name.text).map_or(Recv::Unknown, |d| self.of_decl(d)),
                Recv::Struct(s) => child_field(self.design, s, &name.text).map_or(Recv::Unknown, |d| self.of_decl(d)),
                Recv::Carrier(_) | Recv::CarrierArray(..) | Recv::StructArray(_) | Recv::InstanceArray(_) | Recv::Unknown => Recv::Unknown,
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
        // `self` is a value of the type its `impl` is written for.
        if let DeclKind::FnParam(FnParamData::SelfValue) = &design.decl_data(d).kind {
            let imp = design.decl_data(d).parent.and_then(|f| design.decl_data(f).parent);
            return match imp.map(|i| &design.decl_data(i).kind) {
                Some(DeclKind::Impl(i)) => self.of_type(i.self_ty),
                _ => Recv::Unknown,
            };
        }
        match self.local_types.get(&d).copied().or_else(|| design.declared_type(d)) {
            Some(ty) => self.of_type(ty),
            None => Recv::Unknown,
        }
    }

    /// What a declared type is a value of. An array of carriers is its own
    /// receiver: it reads as a carrier only after an index.
    fn of_type(&self, ty: TypeId) -> Recv {
        match &self.design.type_node(ty).kind {
            TypeKind::Flip(inner) => self.of_type(*inner),
            TypeKind::Array { elem, .. } => match self.of_type(*elem) {
                Recv::Carrier(nature) => Recv::CarrierArray(nature, 1),
                Recv::CarrierArray(nature, depth) => Recv::CarrierArray(nature, depth + 1),
                Recv::Struct(s) | Recv::StructArray(s) => Recv::StructArray(s),
                Recv::Instance(m) | Recv::InstanceArray(m) => Recv::InstanceArray(m),
                Recv::Unknown => Recv::Unknown,
            },
            _ => match self.design.type_resolution(ty) {
                Resolution::Decl(t) => match &self.design.decl_data(t).kind {
                    DeclKind::Struct(_) => Recv::Struct(t),
                    DeclKind::Module(_) => Recv::Instance(t),
                    DeclKind::Nature(_) => Recv::Carrier(Some(t)),
                    DeclKind::GenericParam(_) if self.design.is_nature_generic(t) => Recv::Carrier(None),
                    _ => Recv::Unknown,
                },
                _ => Recv::Unknown,
            },
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

/// The function `name` written in an `impl` block.
fn child_fn(design: &Design, imp: DeclId, name: &str) -> Option<DeclId> {
    design.decl_data(imp).children.iter().copied().find(|c| {
        let d = design.decl_data(*c);
        matches!(d.kind, DeclKind::Fn(_)) && d.name.as_ref().is_some_and(|n| n.text == name)
    })
}
