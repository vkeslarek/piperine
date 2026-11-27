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
use super::classify::Classifier;
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

/// Where a carrier method was applied that is no carrier (`E0211`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Misapplied {
    /// The method is applied to a carrier, or to nothing a carrier method is refused on.
    Nowhere,
    /// The method is applied to an array of carriers.
    OnArray,
    /// The method is applied to this whole bundle.
    OnBundle(DeclId),
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

/// Resolves the members every field access and method call of a design names.
pub(super) struct MemberResolution<'a> {
    design: &'a Design,
    /// The annotated type of each `let` binding of a block.
    local_types: BTreeMap<DeclId, TypeId>,
    /// Whether a trait is in scope for the code of a declaration (blanket methods need it).
    in_scope: InScope<'a>,
}

/// Is the trait (second) in scope for code owned by the declaration (first)?
pub(super) type InScope<'a> = &'a dyn Fn(DeclId, DeclId) -> bool;

impl<'a> MemberResolution<'a> {
    /// The resolution over `design`; `in_scope` tells whether a trait is in scope for a declaration.
    pub(super) fn new(design: &'a Design, in_scope: InScope<'a>) -> Self {
        MemberResolution { design, local_types: Self::local_types(design), in_scope }
    }

    /// What every field access and method call of the design names, and the errors found.
    pub(super) fn resolve(&self) -> Members {
        let design = self.design;
        let reader = self;
        let mut out = Members::default();
        let access = Classifier::new(design).access_table();
        let probe_operands = reader.probe_operands();
        for i in 0..design.expr_count() {
            let expr = ExprId(i as u32);
            if let Some(segment) = reader.reached_access(expr, &access) {
                out.segments.push(segment);
            }
            if let ExprKind::MethodCall { recv, method, .. } = &design.expr_node(expr).kind {
                let (res, misapplied) = reader.method(*recv, &method.text, design.expr_node(expr).owner);
                out.res.push((expr, res));
                let owner = design.expr_node(expr).owner;
                match misapplied {
                    Misapplied::Nowhere => {}
                    Misapplied::OnArray => {
                        let message = format!("the carrier method `{}` is applied to an array of carriers", method.text);
                        out.diagnostics.push((design.decl_data(owner).file, Diagnostic::new("E0211", message, method.span, "index the array first")));
                    }
                    // §7 rule 12: carrier methods apply to a field, not to the bundle
                    Misapplied::OnBundle(bundle) => {
                        let name = design.decl_data(bundle).name.as_ref().map_or("", |n| n.text.as_str());
                        let message = format!("the carrier method `{}` is applied to the bundle `{name}`: it applies to a field of it (§7 rule 12, §5.1)", method.text);
                        out.diagnostics.push((design.decl_data(owner).file, Diagnostic::new("E0211", message, method.span, "call it on a field")));
                    }
                }
            }
            if let ExprKind::Field { expr: recv, name, .. } = &design.expr_node(expr).kind {
                let (res, error) = reader.field(*recv, &name.text, design.expr_node(expr).owner);
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
        // an element `c.tap[k]` of a `pub` array of nodes is read through the item `tap`
        let mut first = self.unparenthesized(args.first()?.value);
        while let ExprKind::Index { expr: array, .. } = &self.design.expr_node(first).kind {
            first = self.unparenthesized(*array);
        }
        let ExprKind::Field { expr: recv, name, .. } = &self.design.expr_node(first).kind else { return None };
        // the item of a placed child, or the field of a bundle (`v(d.p, d.n)`, §7 rule 12)
        let item = match self.receiver(*recv) {
            Recv::Instance(module) => self.member_of(module, &name.text)?,
            Recv::Struct(bundle) => self.child_field(bundle, &name.text)?,
            _ => return None,
        };
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
    fn field(&self, recv: ExprId, name: &str, owner: DeclId) -> (Resolution, Option<(DeclId, bool)>) {
        let deferred = (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), None);
        match self.receiver(recv) {
            Recv::Instance(module) => match self.member_of(module, name) {
                Some(item) if self.visible(item) || self.written_in_impl_of(owner, module) => (Resolution::Decl(item), None),
                Some(item) => (Resolution::Decl(item), Some((module, false))),
                None => (Resolution::Unresolved, Some((module, true))),
            },
            Recv::Struct(s) => match self.child_field(s, name) {
                Some(f) => (Resolution::Decl(f), None),
                None => (Resolution::Unresolved, None),
            },
            // The fields of a carrier come from its value type, which needs inference.
            Recv::Carrier(_) | Recv::CarrierArray(..) | Recv::StructArray(_) | Recv::InstanceArray(_) | Recv::Unknown => deferred,
        }
    }

    /// Whether the code of `owner` is written in an `impl` on the module type `module` (§3.1.4): `self.vth0` reads a param the
    /// module declares without `pub`, which is the module's own business.
    fn written_in_impl_of(&self, owner: DeclId, module: DeclId) -> bool {
        let mut at = Some(owner);
        while let Some(decl) = at {
            if let DeclKind::Impl(data) = &self.design.decl_data(decl).kind {
                return self.design.type_resolution(data.self_ty) == Resolution::Decl(module);
            }
            at = self.design.decl_data(decl).parent;
        }
        false
    }

    /// The method `name` of the receiver: the carrier built-ins, then the
    /// `impl`s of its nature or bundle, then those of the value it reads. The
    /// second answer says whether a carrier method was applied to an array of carriers or to a whole bundle (`E0211`).
    fn method(&self, recv: ExprId, name: &str, owner: DeclId) -> (Resolution, Misapplied) {
        let design = self.design;
        let in_impls = |target: DeclId| design.impls_of(target).into_iter().find_map(|i| self.child_fn(i, name)).map(Resolution::Decl);
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
                    Some(_) => (Resolution::Unresolved, Misapplied::OnArray),
                    None => (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), Misapplied::Nowhere),
                };
            }
            Recv::Struct(s) => match in_impls(s) {
                Some(found) => Some(found),
                // a carrier method of the built-in catalogue on a whole bundle (§7 rule 12)
                None if matches!(&design.decl_data(s).kind, DeclKind::Struct(data) if data.bundle == Some(true)) && builtin_method(name).is_some() => {
                    return (Resolution::Unresolved, Misapplied::OnBundle(s));
                }
                None => None,
            },
            Recv::Instance(_) => None,
            Recv::StructArray(_) | Recv::InstanceArray(_) | Recv::Unknown => return (Resolution::Deferred(DeferReason::ReceiverTypeUnknown), Misapplied::Nowhere),
        };
        (found.unwrap_or(Resolution::Unresolved), Misapplied::Nowhere)
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
            self.child_fn(tr, name)
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
                Recv::Struct(s) => self.child_field(s, &name.text).map_or(Recv::Unknown, |d| self.of_decl(d)),
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
        let design = self.design;
        for c in &design.decl_data(module).children {
            let data = design.decl_data(*c);
            match &data.kind {
                DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) if data.name.as_ref().is_some_and(|n| n.text == name) => return Some(*c),
                DeclKind::Let(_) => {
                    if let Some(b) = data.children.iter().find(|b| design.decl_data(**b).name.as_ref().is_some_and(|n| n.text == name)) {
                        return Some(*b);
                    }
                }
                DeclKind::Generation(_) => {
                    if let Some(found) = self.member_of(*c, name) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
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

    /// The field `name` of a struct.
    fn child_field(&self, owner: DeclId, name: &str) -> Option<DeclId> {
        self.design.decl_data(owner).children.iter().copied().find(|c| {
            let d = self.design.decl_data(*c);
            matches!(d.kind, DeclKind::Field(_)) && d.name.as_ref().is_some_and(|n| n.text == name)
        })
    }

    /// The function `name` written in an `impl` block.
    fn child_fn(&self, imp: DeclId, name: &str) -> Option<DeclId> {
        self.design.decl_data(imp).children.iter().copied().find(|c| {
            let d = self.design.decl_data(*c);
            matches!(d.kind, DeclKind::Fn(_)) && d.name.as_ref().is_some_and(|n| n.text == name)
        })
    }
}
