//! Stage 6: classification. Reads the resolved paths and fills the
//! classification slots: the kind of each port, the bundle flag of each
//! struct, the class of each call and of each `Name { .. }` expression, the
//! names of arguments and fields, and each module's instance index. The
//! helpers on [`Design`] (`declared_type`, `nature_of`) are plain reads of the
//! resolved tree, shared with the reflection handles.

use std::collections::{BTreeMap, BTreeSet};

use super::super::body::{Arg, ExprKind, FieldInit, PatKind, QuantityRole, StmtKind, TypeKind};
use super::super::builtin::BuiltinKind;
use super::super::decl::{DeclKind, FnParamData, GenericParamData, Name};
use super::super::design::Design;
use super::super::ids::{DeclId, ExprId, FileId, PatId, PathId, TypeId};
use super::super::resolution::{CallClass, PortKind, Resolution, StructClass};
use crate::msl::Diagnostic;

/// One call, classified: the class, and what its named arguments and param
/// bindings name, by position.
pub(super) struct CallFacts {
    pub expr: ExprId,
    pub class: CallClass,
    pub args: Vec<(usize, Resolution)>,
    pub params: Vec<(usize, Resolution)>,
}

/// One `Name { .. }` expression, classified.
pub(super) struct StructFacts {
    pub expr: ExprId,
    pub class: StructClass,
    pub fields: Vec<(usize, Resolution)>,
}

/// What classification computed, not yet written into the design.
#[derive(Default)]
pub(super) struct Classified {
    pub port_kinds: Vec<(DeclId, PortKind)>,
    pub bundles: Vec<(DeclId, bool)>,
    pub calls: Vec<CallFacts>,
    pub structs: Vec<StructFacts>,
    /// `(pattern, field index, the field it names)` for each field of a struct pattern.
    pub pattern_fields: Vec<(PatId, usize, Resolution)>,
    /// `(module, call)` for every instance, in source order.
    pub instances: Vec<(DeclId, ExprId)>,
    pub diagnostics: Vec<(FileId, Diagnostic)>,
    /// `(path, segment, what it names)`: the callee of an access call.
    pub segments: Vec<(PathId, usize, Resolution)>,
}

/// The quantities that declare each access name (`v`, `i`, ..), in source order.
pub(super) fn access_table(design: &Design) -> BTreeMap<String, Vec<DeclId>> {
    let mut table: BTreeMap<String, Vec<DeclId>> = BTreeMap::new();
    for i in 0..design.decl_count() {
        let id = DeclId(i as u32);
        let DeclKind::Quantity(q) = &design.decl_data(id).kind else { continue };
        let Some(access) = q.attrs.iter().find(|a| a.name.text == "access") else { continue };
        let ExprKind::Path(p) = &design.expr_node(access.value).kind else { continue };
        if let [one] = design.path_node(*p).segments.as_slice() {
            table.entry(one.name.text.clone()).or_default().push(id);
        }
    }
    table
}

impl Design {
    /// The declared type of a port, param, signal, field, constant or
    /// function parameter; none for any other declaration.
    pub(crate) fn declared_type(&self, id: DeclId) -> Option<TypeId> {
        match &self.decl_data(id).kind {
            DeclKind::Port(p) => Some(p.ty),
            DeclKind::Param(p) => Some(p.ty),
            DeclKind::Signal(s) => Some(s.ty),
            DeclKind::Field(f) => Some(f.ty),
            DeclKind::Const(c) => Some(c.ty),
            DeclKind::FnParam(FnParamData::Named { ty }) => Some(*ty),
            // `self` of a function in an `impl` has the type the `impl` is written for (§3.1.5)
            DeclKind::FnParam(FnParamData::SelfValue) => {
                let function = self.decl_data(id).parent?;
                match &self.decl_data(self.decl_data(function).parent?).kind {
                    DeclKind::Impl(owner) => Some(owner.self_ty),
                    _ => None,
                }
            }
            DeclKind::GenericParam(GenericParamData::Const { ty, .. }) => Some(*ty),
            _ => None,
        }
    }

    /// The type under arrays and `flip`: the element a carrier type is made of.
    pub(crate) fn peel_type(&self, mut ty: TypeId) -> TypeId {
        loop {
            match &self.type_node(ty).kind {
                TypeKind::Array { elem, .. } | TypeKind::Flip(elem) => ty = *elem,
                _ => return ty,
            }
        }
    }

    /// What the path of a type names, under arrays and `flip`.
    pub(crate) fn type_resolution(&self, ty: TypeId) -> Resolution {
        match &self.type_node(self.peel_type(ty)).kind {
            TypeKind::Path(p) => self.path_node(*p).segments.last().map_or(Resolution::Unresolved, |s| s.res),
            _ => Resolution::Unresolved,
        }
    }

    /// The nature a type is made of, through arrays and `flip`.
    pub(crate) fn nature_of(&self, ty: TypeId) -> Option<DeclId> {
        match self.type_resolution(ty) {
            Resolution::Decl(d) if matches!(self.decl_data(d).kind, DeclKind::Nature(_)) => Some(d),
            _ => None,
        }
    }

    /// The resolution of the last segment of a bound path.
    fn bound_resolution(&self, bound: PathId) -> Resolution {
        self.path_node(bound).segments.last().map_or(Resolution::Unresolved, |s| s.res)
    }

    /// Is `decl` a generic parameter bounded by the core `Nature` (§3.4.2)? The
    /// bound counts by what it resolved to, not by how it is spelled.
    pub(crate) fn is_nature_generic(&self, decl: DeclId) -> bool {
        let DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) = &self.decl_data(decl).kind else { return false };
        bounds.iter().any(|b| matches!(self.bound_resolution(*b), Resolution::Builtin(t) if t.info().name == "Nature"))
    }

    /// Is `decl` a generic parameter with a bound that resolved to nothing?
    /// What it carries is not known then.
    pub(super) fn has_unresolved_bound(&self, decl: DeclId) -> bool {
        let DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) = &self.decl_data(decl).kind else { return false };
        bounds.iter().any(|b| self.bound_resolution(*b) == Resolution::Unresolved)
    }

    /// Is a type a carrier: a nature or a `Nature`-bounded generic, under arrays and `flip`?
    pub(super) fn is_carrier_type(&self, ty: TypeId) -> bool {
        match self.type_resolution(ty) {
            Resolution::Decl(d) => matches!(self.decl_data(d).kind, DeclKind::Nature(_)) || self.is_nature_generic(d),
            _ => false,
        }
    }

    /// Every `impl` block, in source order.
    pub(crate) fn impls_in_order(&self) -> Vec<DeclId> {
        (0..self.decl_count()).map(|i| DeclId(i as u32)).filter(|id| matches!(self.decl_data(*id).kind, DeclKind::Impl(_))).collect()
    }

    /// The `impl` blocks whose self type names `target`, in source order.
    pub(crate) fn impls_of(&self, target: DeclId) -> Vec<DeclId> {
        (0..self.decl_count())
            .map(|i| DeclId(i as u32))
            .filter(|id| match &self.decl_data(*id).kind {
                DeclKind::Impl(i) => self.type_resolution(i.self_ty) == Resolution::Decl(target),
                _ => false,
            })
            .collect()
    }

    /// Whether `nature` is resolved: some `impl Resolve<X, V> for nature` exists (§4 rule 8), the
    /// one fact that tells a one-driver discrete nature from a resolved one.
    pub(crate) fn is_resolved_nature(&self, nature: DeclId) -> bool {
        self.impls_of(nature).into_iter().any(|decl| match &self.decl_data(decl).kind {
            DeclKind::Impl(data) => data.trait_.and_then(|path| self.path_node(path).segments.last().map(|segment| segment.res)).is_some_and(|res| matches!(res, Resolution::Builtin(id) if id.info().name == "Resolve")),
            _ => false,
        })
    }
}

/// Classify a design whose names are resolved.
pub(super) fn run(design: &Design) -> Classified {
    let bundles = bundle_structs(design);
    let mut out = Classified::default();
    for i in 0..design.decl_count() {
        let id = DeclId(i as u32);
        match &design.decl_data(id).kind {
            DeclKind::Port(_) => out.port_kinds.push((id, port_kind(design, id, &bundles))),
            DeclKind::Struct(_) => out.bundles.push((id, bundles.contains(&id))),
            DeclKind::Impl(i) => {
                if let Resolution::Decl(d) = design.type_resolution(i.self_ty) {
                    if matches!(design.decl_data(d).kind, DeclKind::Module(_)) {
                        let message = "an `impl` block cannot be written for a module (§3.1.4)".to_string();
                        let span = design.type_node(i.self_ty).span;
                        out.diagnostics.push((design.decl_data(id).file, Diagnostic::new("E0130", message, span, "a module")));
                    }
                }
            }
            _ => {}
        }
    }
    let templates = struct_expressions(design, &bundles, &mut out);
    struct_patterns(design, &mut out);
    call_expressions(design, &templates, &access_table(design), &mut out);
    out
}

/// The structs with a carrier field, directly or through a bundle field: the
/// least fixpoint, so a cycle of structs ends and a carrier anywhere on it
/// makes every member a bundle.
fn bundle_structs(design: &Design) -> BTreeSet<DeclId> {
    let structs: Vec<DeclId> = (0..design.decl_count()).map(|i| DeclId(i as u32)).filter(|d| matches!(design.decl_data(*d).kind, DeclKind::Struct(_))).collect();
    let mut bundles = BTreeSet::new();
    loop {
        let before = bundles.len();
        for s in &structs {
            if bundles.contains(s) {
                continue;
            }
            let has_carrier = design.decl_data(*s).children.iter().filter_map(|c| design.declared_type(*c)).any(|ty| {
                design.is_carrier_type(ty) || matches!(design.type_resolution(ty), Resolution::Decl(d) if bundles.contains(&d))
            });
            if has_carrier {
                bundles.insert(*s);
            }
        }
        if bundles.len() == before {
            return bundles;
        }
    }
}

fn port_kind(design: &Design, port: DeclId, bundles: &BTreeSet<DeclId>) -> PortKind {
    let Some(ty) = design.declared_type(port) else { return PortKind::Unknown };
    if design.is_carrier_type(ty) {
        return PortKind::Carrier;
    }
    match design.type_resolution(ty) {
        Resolution::Decl(d) if bundles.contains(&d) => PortKind::Bundle,
        Resolution::Decl(d) if design.has_unresolved_bound(d) => PortKind::Unknown,
        Resolution::Decl(_) | Resolution::Builtin(_) => PortKind::ValueInput,
        Resolution::Unresolved | Resolution::Deferred(_) => match &design.type_node(design.peel_type(ty)).kind {
            // Tuples of values are values; `_` and a missing name tell nothing.
            TypeKind::Tuple(_) => PortKind::ValueInput,
            _ => PortKind::Unknown,
        },
    }
}

// ---- `Name { .. }` expressions ----------------------------------------------------------------

/// What the path of an expression names, last segment.
fn path_res(design: &Design, expr: ExprId) -> Resolution {
    match &design.expr_node(expr).kind {
        ExprKind::Path(p) => design.path_node(*p).segments.last().map_or(Resolution::Unresolved, |s| s.res),
        ExprKind::Paren(inner) => path_res(design, *inner),
        _ => Resolution::Unresolved,
    }
}

/// The first declaration of kind `want` named `name` among the children of
/// `owner`, entering generation.
fn child_named(design: &Design, owner: DeclId, name: &str, want: fn(&DeclKind) -> bool) -> Option<DeclId> {
    for c in &design.decl_data(owner).children {
        let d = design.decl_data(*c);
        if want(&d.kind) && d.name.as_ref().is_some_and(|n| n.text == name) {
            return Some(*c);
        }
        if matches!(d.kind, DeclKind::Generation(_)) {
            if let Some(found) = child_named(design, *c, name, want) {
                return Some(found);
            }
        }
    }
    None
}

/// The diagnostic `code` for a name written for something `owner` does not
/// have: a port, a param, a field or a function parameter.
fn missing_binding(design: &Design, code: &'static str, owner: DeclId, what: &str, name: &Name, from: DeclId) -> (FileId, Diagnostic) {
    let owner_name = design.decl_data(owner).name.as_ref().map_or("", |n| n.text.as_str());
    let message = format!("no {what} `{}` in `{owner_name}`", name.text);
    (design.decl_data(from).file, Diagnostic::new(code, message, name.span, "not found"))
}

/// The diagnostic for a name that an earlier entry of the same list already bound.
fn repeated_binding(design: &Design, code: &'static str, what: &str, name: &Name, from: DeclId) -> (FileId, Diagnostic) {
    let message = format!("{what} `{}` is given twice", name.text);
    (design.decl_data(from).file, Diagnostic::new(code, message, name.span, "repeated"))
}

/// Is a declaration's type written `Option<..>`? Syntactic, as §3.2.7 reads it.
fn is_written_option(design: &Design, decl: DeclId) -> bool {
    let Some(ty) = design.declared_type(decl) else { return false };
    match &design.type_node(ty).kind {
        TypeKind::Path(p) => design.path_node(*p).segments.last().is_some_and(|s| s.name.text == "Option"),
        _ => false,
    }
}

/// The declarations whose position a call counts by: the ports of a module,
/// the parameters of a function, in order.
fn positional_slots(design: &Design, owner: DeclId) -> Vec<DeclId> {
    let want: fn(&DeclKind) -> bool = if matches!(design.decl_data(owner).kind, DeclKind::Module(_)) { is_port } else { is_fn_param };
    design.decl_data(owner).children.iter().copied().filter(|c| want(&design.decl_data(*c).kind)).collect()
}

fn is_param(k: &DeclKind) -> bool {
    matches!(k, DeclKind::Param(_))
}

fn is_port(k: &DeclKind) -> bool {
    matches!(k, DeclKind::Port(_))
}

fn is_field(k: &DeclKind) -> bool {
    matches!(k, DeclKind::Field(_))
}

fn is_fn_param(k: &DeclKind) -> bool {
    matches!(k, DeclKind::FnParam(_))
}

/// Classify every struct expression; the answers by expression, for the call pass.
fn struct_expressions(design: &Design, bundles: &BTreeSet<DeclId>, out: &mut Classified) -> BTreeMap<ExprId, StructClass> {
    let mut classes = BTreeMap::new();
    for i in 0..design.expr_count() {
        let expr = ExprId(i as u32);
        let ExprKind::Struct { path, fields, .. } = &design.expr_node(expr).kind else { continue };
        let res = design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(target) = res else {
            out.structs.push(StructFacts { expr, class: StructClass::Unclassified, fields: Vec::new() });
            continue;
        };
        let (class, want): (StructClass, fn(&DeclKind) -> bool) = match &design.decl_data(target).kind {
            DeclKind::Module(_) => (StructClass::Template(target), is_param),
            DeclKind::Struct(_) if bundles.contains(&target) => (StructClass::BundleLiteral, is_field),
            DeclKind::Struct(_) | DeclKind::Variant(_) => (StructClass::StructLiteral, is_field),
            _ => (StructClass::Unclassified, is_field),
        };
        let from = design.expr_node(expr).owner;
        let mut named: Vec<(usize, Resolution)> = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for (k, f) in fields.iter().enumerate() {
            let found = child_named(design, target, &f.name.text, want);
            let repeated = seen.contains(&f.name.text.as_str());
            seen.push(&f.name.text);
            match (found, class) {
                (Some(_), StructClass::Template(_)) if repeated => out.diagnostics.push(repeated_binding(design, "E0320", "param", &f.name, from)),
                (Some(_), StructClass::BundleLiteral) if repeated => out.diagnostics.push(repeated_binding(design, "E0263", "field", &f.name, from)),
                (Some(d), _) => named.push((k, Resolution::Decl(d))),
                (None, StructClass::Template(_)) => out.diagnostics.push(missing_binding(design, "E0320", target, "param", &f.name, from)),
                (None, StructClass::BundleLiteral) => out.diagnostics.push(missing_binding(design, "E0263", target, "field", &f.name, from)),
                (None, StructClass::Unclassified) => {}
                // A field a struct literal names that the type lacks (§1.3.3).
                (None, _) => out.diagnostics.push(missing_binding(design, "E0186", target, "field", &f.name, from)),
            }
        }
        classes.insert(expr, class);
        out.structs.push(StructFacts { expr, class, fields: named });
    }
    classes
}

/// The fields a struct pattern names: each links to the field of the struct or
/// variant its path names; one that does not exist is `E0186`, as in a literal (§1.3.3).
fn struct_patterns(design: &Design, out: &mut Classified) {
    for i in 0..design.pat_count() {
        let id = PatId(i as u32);
        let node = design.pat_node(id);
        let PatKind::Struct { path, fields, .. } = &node.kind else { continue };
        let Resolution::Decl(target) = design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) else { continue };
        if !matches!(design.decl_data(target).kind, DeclKind::Struct(_) | DeclKind::Variant(_)) {
            continue;
        }
        for (k, f) in fields.iter().enumerate() {
            match child_named(design, target, &f.name.text, is_field) {
                Some(d) => out.pattern_fields.push((id, k, Resolution::Decl(d))),
                None => out.diagnostics.push(missing_binding(design, "E0186", target, "field", &f.name, node.owner)),
            }
        }
    }
}

// ---- calls and instances ----------------------------------------------------------------------

/// The nearest module around a declaration; a behavior block belongs to the
/// module it is bound to.
fn enclosing_module(design: &Design, mut at: DeclId) -> Option<DeclId> {
    loop {
        let data = design.decl_data(at);
        match &data.kind {
            DeclKind::Module(_) => return Some(at),
            DeclKind::Behavior(b) => return b.module,
            _ => at = data.parent?,
        }
    }
}

/// Is `generic` a generic parameter with an `Fn(..)` bound?
fn is_fn_bounded(design: &Design, generic: DeclId) -> bool {
    let DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) = &design.decl_data(generic).kind else { return false };
    bounds.iter().any(|b| design.path_node(*b).segments.last().is_some_and(|s| s.name.text == "Fn"))
}

/// A call written through the value `value` (a param, value input, constant
/// or template binding): an instance when the value is a template.
fn through_value(design: &Design, value: DeclId, templates: &BTreeMap<ExprId, StructClass>) -> CallClass {
    if let DeclKind::Binding(_) = &design.decl_data(value).kind {
        // A body `let` whose value is a template written in place.
        let template = design.decl_data(value).parent.and_then(|p| match &design.decl_data(p).kind {
            DeclKind::Let(l) => match templates.get(&l.value) {
                Some(StructClass::Template(m)) => Some(*m),
                _ => None,
            },
            _ => None,
        });
        return match template {
            Some(m) => CallClass::Instance { module: Some(m), via: Some(value) },
            None => CallClass::Function,
        };
    }
    let Some(ty) = design.declared_type(value) else { return CallClass::Function };
    match design.type_node(ty).kind {
        TypeKind::Path(p) => match design.path_node(p).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(t)) if matches!(design.decl_data(t).kind, DeclKind::Module(_)) => CallClass::Instance { module: Some(t), via: Some(value) },
            Some(Resolution::Decl(t)) if is_fn_bounded(design, t) => CallClass::Instance { module: None, via: Some(value) },
            _ => CallClass::Function,
        },
        _ => CallClass::Function,
    }
}

/// The class of a call by what its callee is, and the declaration whose
/// parameters its named arguments name.
fn callee_class(design: &Design, callee: ExprId, templates: &BTreeMap<ExprId, StructClass>) -> (CallClass, Option<DeclId>) {
    match &design.expr_node(callee).kind {
        ExprKind::Paren(inner) => callee_class(design, *inner, templates),
        ExprKind::Path(_) => match path_res(design, callee) {
            Resolution::Decl(d) => match &design.decl_data(d).kind {
                DeclKind::Module(_) => (CallClass::Instance { module: Some(d), via: None }, Some(d)),
                DeclKind::Fn(_) => (CallClass::Function, Some(d)),
                DeclKind::Param(_) | DeclKind::Port(_) | DeclKind::Const(_) | DeclKind::FnParam(_) | DeclKind::Binding(_) => {
                    let class = through_value(design, d, templates);
                    let module = match class {
                        CallClass::Instance { module, .. } => module,
                        _ => None,
                    };
                    (class, module)
                }
                _ => (CallClass::Function, None),
            },
            Resolution::Builtin(b) if b.info().kind == BuiltinKind::Module => (CallClass::Instance { module: None, via: None }, None),
            _ => (CallClass::Function, None),
        },
        // `Name { .. }(connections)`: the template written in place.
        ExprKind::Struct { .. } => match templates.get(&callee) {
            Some(StructClass::Template(m)) => (CallClass::Instance { module: Some(*m), via: Some(*m) }, Some(*m)),
            _ => (CallClass::Function, None),
        },
        // `pick(..)(connections)`: a function that returns a module type.
        ExprKind::Call { callee: inner, .. } => match path_res(design, *inner) {
            Resolution::Decl(f) => match &design.decl_data(f).kind {
                DeclKind::Fn(data) => {
                    let module = data.ret.and_then(|r| match design.type_resolution(r) {
                        Resolution::Decl(m) if matches!(design.decl_data(m).kind, DeclKind::Module(_)) => Some(m),
                        _ => None,
                    });
                    match module {
                        Some(m) => (CallClass::Instance { module: Some(m), via: Some(f) }, Some(m)),
                        None => (CallClass::Function, None),
                    }
                }
                _ => (CallClass::Function, None),
            },
            _ => (CallClass::Function, None),
        },
        _ => (CallClass::Function, None),
    }
}

/// The nature of an expression read from a declaration: a path to something
/// with a declared type, through indexes and parentheses.
fn nature_of_expr(design: &Design, mut expr: ExprId) -> Option<DeclId> {
    loop {
        match &design.expr_node(expr).kind {
            ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => expr = *inner,
            ExprKind::Path(_) => {
                let Resolution::Decl(d) = path_res(design, expr) else { return None };
                return design.nature_of(design.declared_type(d)?);
            }
            _ => return None,
        }
    }
}

/// What an access call's callee decided.
enum Probed {
    /// A probe: the callee names this.
    Probe(PathId, Resolution),
    /// The first argument's declared nature declares no such access: the
    /// callee stays unresolved and the call is no probe (`E0206`).
    WrongNature(PathId),
}

/// A call whose callee is the access name of a quantity, or the built-in
/// `potential` / `flow`: what this call decides about the callee. A
/// declaration in scope with that name already resolved the path and wins. An
/// access never falls back to a quantity of another nature.
fn probe_callee(design: &Design, callee: ExprId, first: Option<ExprId>, access: &BTreeMap<String, Vec<DeclId>>) -> Option<Probed> {
    let ExprKind::Path(path) = &design.expr_node(callee).kind else { return None };
    let node = design.path_node(*path);
    let [segment] = node.segments.as_slice() else { return None };
    match segment.res {
        Resolution::Builtin(b) if matches!(b.info().name, "potential" | "flow") => Some(Probed::Probe(*path, segment.res)),
        Resolution::Unresolved if segment.args.is_none() => {
            let candidates = access.get(&segment.name.text)?;
            let nature = first.and_then(|e| nature_of_expr(design, e));
            let by_nature = nature.and_then(|n| candidates.iter().copied().find(|q| design.decl_data(*q).parent == Some(n)));
            let res = match (by_nature, nature, candidates.as_slice()) {
                (Some(q), _, _) => Resolution::Decl(q),
                (None, Some(_), _) => return Some(Probed::WrongNature(*path)),
                (None, None, &[q]) => Resolution::Decl(q),
                _ => Resolution::Deferred(super::super::resolution::DeferReason::AmbiguousAccess),
            };
            Some(Probed::Probe(*path, res))
        }
        _ => None,
    }
}

/// What the named arguments of a call name: an optional parameter (a port or a
/// function parameter written `Option<..>`) that no earlier argument bound.
/// Any other name is `E0140` and its slot stays unresolved (§3.2.9).
fn named_arguments(design: &Design, owner: DeclId, args: &[Arg], from: DeclId, out: &mut Classified) -> Vec<(usize, Resolution)> {
    let is_module = matches!(design.decl_data(owner).kind, DeclKind::Module(_));
    let (what, want): (&str, fn(&DeclKind) -> bool) = if is_module { ("port", is_port) } else { ("parameter", is_fn_param) };
    let positional = args.iter().filter(|a| a.name.is_none()).count();
    let mut bound: Vec<DeclId> = positional_slots(design, owner).into_iter().take(positional).collect();
    let mut links = Vec::new();
    for (k, a) in args.iter().enumerate() {
        let Some(name) = &a.name else { continue };
        let owner_name = design.decl_data(owner).name.as_ref().map_or("", |n| n.text.as_str());
        match child_named(design, owner, &name.text, want).filter(|d| is_written_option(design, *d)) {
            Some(d) if !bound.contains(&d) => {
                bound.push(d);
                links.push((k, Resolution::Decl(d)));
            }
            Some(_) => {
                let message = format!("the {what} `{}` of `{owner_name}` is already bound", name.text);
                out.diagnostics.push((design.decl_data(from).file, Diagnostic::new("E0140", message, name.span, "already bound")));
            }
            None => {
                let message = format!("no optional {what} `{}` in `{owner_name}`", name.text);
                out.diagnostics.push((design.decl_data(from).file, Diagnostic::new("E0140", message, name.span, "not an optional parameter")));
            }
        }
    }
    links
}

/// What the param block of an instance names: params of the module, each once.
fn param_block(design: &Design, module: DeclId, params: &[FieldInit], from: DeclId, out: &mut Classified) -> Vec<(usize, Resolution)> {
    let mut links = Vec::new();
    for (k, f) in params.iter().enumerate() {
        match child_named(design, module, &f.name.text, is_param) {
            Some(_) if params[..k].iter().any(|e| e.name.text == f.name.text) => out.diagnostics.push(repeated_binding(design, "E0320", "param", &f.name, from)),
            Some(d) => links.push((k, Resolution::Decl(d))),
            None => out.diagnostics.push(missing_binding(design, "E0320", module, "param", &f.name, from)),
        }
    }
    links
}

/// The nature of the first argument of the call `node`, when it names a declared node.
fn args_first_nature(design: &Design, node: &super::super::body::ExprNode) -> Option<DeclId> {
    let ExprKind::Call { args, .. } = &node.kind else { return None };
    nature_of_expr(design, args.first()?.value)
}

/// The left-hand sides of the contributions of the design: the calls a `+>` writes to.
fn contribution_targets(design: &Design) -> BTreeSet<ExprId> {
    (0..design.stmt_count())
        .filter_map(|i| match &design.stmt_node(crate::pom::StmtId(i as u32)).kind {
            StmtKind::Contribution { lhs, .. } => Some(*lhs),
            _ => None,
        })
        .collect()
}

/// Whether the nature `nature` declares a `flow` quantity.
fn declares_flow(design: &Design, nature: DeclId) -> bool {
    design.decl_data(nature).children.iter().any(|c| matches!(&design.decl_data(*c).kind, DeclKind::Quantity(q) if q.role == QuantityRole::Flow))
}

/// Whether the access `name` is that of a flow quantity of some nature.
fn is_flow_access(design: &Design, access: &BTreeMap<String, Vec<DeclId>>, name: &str) -> bool {
    access.get(name).is_some_and(|quantities| quantities.iter().any(|q| matches!(&design.decl_data(*q).kind, DeclKind::Quantity(data) if data.role == QuantityRole::Flow)))
}

fn call_expressions(design: &Design, templates: &BTreeMap<ExprId, StructClass>, access: &BTreeMap<String, Vec<DeclId>>, out: &mut Classified) {
    let targets = contribution_targets(design);
    for i in 0..design.expr_count() {
        let expr = ExprId(i as u32);
        let node = design.expr_node(expr);
        let ExprKind::Call { callee, args, params, .. } = &node.kind else { continue };
        match probe_callee(design, *callee, args.first().map(|a| a.value), access) {
            Some(Probed::Probe(path, res)) => {
                out.segments.push((path, 0, res));
                out.calls.push(CallFacts { expr, class: CallClass::Probe, args: Vec::new(), params: Vec::new() });
                continue;
            }
            Some(Probed::WrongNature(path)) => {
                let segment = &design.path_node(path).segments[0];
                // §11 rule 11, RN-08 AC 4: a flow access written as the target of a contribution to a
                // node of a nature without `flow` is a flow contribution (`E0413`); read, or any other
                // access the nature lacks, it is a probe of something it does not declare (`E0206`).
                let first_nature = args_first_nature(design, node);
                let flow_contribution = targets.contains(&expr) && is_flow_access(design, access, &segment.name.text) && first_nature.is_some_and(|n| !declares_flow(design, n));
                let diagnostic = if flow_contribution {
                    let message = format!("a flow contribution needs a nature with a `flow`: the nature of the first argument declares none, so `{}` cannot be contributed to it (§11 rule 11)", segment.name.text);
                    Diagnostic::new("E0413", message, segment.name.span, "no flow on this nature")
                } else {
                    let message = format!("the nature of the first argument declares no access `{}`", segment.name.text);
                    Diagnostic::new("E0206", message, segment.name.span, "not an access of that nature")
                };
                out.diagnostics.push((design.decl_data(node.owner).file, diagnostic));
                out.calls.push(CallFacts { expr, class: CallClass::Function, args: Vec::new(), params: Vec::new() });
                continue;
            }
            None => {}
        }
        let (class, target) = callee_class(design, *callee, templates);
        let from = node.owner;
        let facts = match target.map(|t| (t, &design.decl_data(t).kind)) {
            Some((module, DeclKind::Module(_))) => CallFacts {
                expr,
                class,
                args: named_arguments(design, module, args, from, out),
                params: param_block(design, module, params.as_deref().unwrap_or(&[]), from, out),
            },
            Some((func, DeclKind::Fn(_))) => CallFacts { expr, class, args: named_arguments(design, func, args, from, out), params: Vec::new() },
            _ => CallFacts { expr, class, args: Vec::new(), params: Vec::new() },
        };
        if matches!(class, CallClass::Instance { .. }) {
            if let Some(module) = enclosing_module(design, node.owner) {
                out.instances.push((module, expr));
            }
        }
        out.calls.push(facts);
    }
}
