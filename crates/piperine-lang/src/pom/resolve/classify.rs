//! Stage 6: classification. Reads the resolved paths and fills the
//! classification slots: the kind of each port, the bundle flag of each
//! struct, the class of each call and of each `Name { .. }` expression, the
//! names of arguments and fields, and each module's instance index. The
//! helpers on [`Design`] (`declared_type`, `nature_of`) are plain reads of the
//! resolved tree, shared with the reflection handles.

use std::collections::{BTreeMap, BTreeSet};

use super::super::body::{ExprKind, TypeKind};
use super::super::builtin::BuiltinKind;
use super::super::decl::{DeclKind, FnParamData, GenericParamData};
use super::super::design::Design;
use super::super::ids::{DeclId, ExprId, FileId, TypeId};
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
    /// `(module, call)` for every instance, in source order.
    pub instances: Vec<(DeclId, ExprId)>,
    pub diagnostics: Vec<(FileId, Diagnostic)>,
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

    /// Is `decl` a generic parameter bounded by `Nature`?
    fn is_nature_generic(&self, decl: DeclId) -> bool {
        let DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) = &self.decl_data(decl).kind else { return false };
        bounds.iter().any(|b| self.path_node(*b).segments.last().is_some_and(|s| s.name.text == "Nature"))
    }

    /// Is a type a carrier: a nature or a `Nature`-bounded generic, under arrays and `flip`?
    fn is_carrier_type(&self, ty: TypeId) -> bool {
        match self.type_resolution(ty) {
            Resolution::Decl(d) => matches!(self.decl_data(d).kind, DeclKind::Nature(_)) || self.is_nature_generic(d),
            _ => false,
        }
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
}

/// Classify a design whose names are resolved.
pub(super) fn run(design: &Design) -> Classified {
    let bundles = bundle_structs(design);
    let mut out = Classified::default();
    for i in 0..design.decl_count() {
        let id = DeclId(i as u32);
        match &design.decl_data(id).kind {
            DeclKind::Port(_) => out.port_kinds.push((id, port_kind(design, id, &bundles))),
            // SPEC_DEVIATION: the design says the slot is filled for every struct; only a bundle
            // is written (`Some(true)`), so a struct that is no bundle keeps `bundle() == None`.
            // Reason: the existing `pom_tree::struct_fields_and_enum_variants` asserts `None`
            // for a non-bundle struct, and a test is never changed by a worker; `is_bundle()` is
            // the question hosts ask and it answers the same either way.
            DeclKind::Struct(_) if bundles.contains(&id) => out.bundles.push((id, true)),
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
    call_expressions(design, &templates, &mut out);
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
        Resolution::Decl(_) | Resolution::Builtin(_) => PortKind::ValueInput,
        Resolution::Unresolved | Resolution::Deferred(_) => match &design.type_node(design.peel_type(ty)).kind {
            // Tuples and slices of values are values; `_` and a missing name tell nothing.
            TypeKind::Tuple(_) | TypeKind::Slice(_) | TypeKind::Ref { .. } => PortKind::ValueInput,
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
        let named = fields.iter().enumerate().filter_map(|(k, f)| Some((k, Resolution::Decl(child_named(design, target, &f.name.text, want)?)))).collect();
        classes.insert(expr, class);
        out.structs.push(StructFacts { expr, class, fields: named });
    }
    classes
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

fn call_expressions(design: &Design, templates: &BTreeMap<ExprId, StructClass>, out: &mut Classified) {
    for i in 0..design.expr_count() {
        let expr = ExprId(i as u32);
        let node = design.expr_node(expr);
        let ExprKind::Call { callee, args, params, .. } = &node.kind else { continue };
        let (class, target) = callee_class(design, *callee, templates);
        let named = |want: fn(&DeclKind) -> bool| -> Vec<(usize, Resolution)> {
            let Some(target) = target else { return Vec::new() };
            args.iter().enumerate().filter_map(|(k, a)| Some((k, Resolution::Decl(child_named(design, target, &a.name.as_ref()?.text, want)?)))).collect()
        };
        let facts = match target.map(|t| (t, &design.decl_data(t).kind)) {
            Some((module, DeclKind::Module(_))) => CallFacts {
                expr,
                class,
                args: named(is_port),
                params: params
                    .iter()
                    .flatten()
                    .enumerate()
                    .filter_map(|(k, f)| Some((k, Resolution::Decl(child_named(design, module, &f.name.text, is_param)?))))
                    .collect(),
            },
            Some((_, DeclKind::Fn(_))) => CallFacts { expr, class, args: named(is_fn_param), params: Vec::new() },
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
