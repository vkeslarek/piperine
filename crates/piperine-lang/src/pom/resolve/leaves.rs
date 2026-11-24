//! Stage 6b: the leaves of bundle ports and signals (spec part II §7 rules 3 to 7 and 14). Each port or
//! signal of a module whose type is a bundle gets one declaration per field, named by its dotted path
//! (`tx.valid`), typed as the field is, with the direction the module's view gives it. They are recorded in
//! [`BundleLeaves`](super::super::bundles::BundleLeaves), never in a module's `children` (the POM mirrors
//! the source). A bundle that is generic (`Stream<u8>`) has the leaves of its instance: each field's type is written
//! again with the struct's parameters replaced by the arguments of the port (`TypeSubstitution`), so a leaf is typed like
//! any port of its module and no later pass substitutes.

use std::collections::BTreeMap;

use super::super::body::{ExprKind, GenericArgNode, GenericArgsNode, PathNode, TypeKind, TypeNode};
use super::super::bundles::{BundleLeaf, BundleView};
use super::super::decl::{DeclData, DeclKind, GenericParamData, Name, PortData, SignalData};
use super::super::design::Design;
use super::super::ids::{DeclId, TypeId};
use super::super::meta::Meta;
use super::super::resolution::{PortKind, Resolution};

/// What each generic parameter of a bundle `struct` is given where the bundle is written (`Stream<u8>`: `T` is `u8`).
type ParameterArguments = BTreeMap<DeclId, GenericArgNode>;

/// A leaf the stage will add: its data and where it sits in its owner's bundle.
struct PlannedLeaf {
    data: DeclData,
    path: String,
    nested: bool,
}

/// The leaves to add to every bundle port and signal of a design.
pub(super) struct LeafPlan<'design> {
    design: &'design mut Design,
}

impl<'design> LeafPlan<'design> {
    /// The plan over a classified design.
    pub(super) fn new(design: &'design mut Design) -> Self {
        LeafPlan { design }
    }

    /// The leaves of every owner, in the order of the arena.
    fn plan(&mut self) -> Vec<(DeclId, Vec<PlannedLeaf>)> {
        let mut out = Vec::new();
        for index in 0..self.design.decl_count() {
            let owner = DeclId(index as u32);
            let data = self.design.decl_data(owner);
            let (ty, view) = match &data.kind {
                DeclKind::Port(port) => (port.ty, BundleView::of_port(self.design, port.dir, port.ty)),
                DeclKind::Signal(signal) => (signal.ty, BundleView::Producer),
                _ => continue,
            };
            let Some(bundle) = self.bundle_of(ty) else { continue };
            // a port of the module, not of a function or a generation arm already unrolled elsewhere
            if !data.parent.is_some_and(|p| matches!(self.design.decl_data(p).kind, DeclKind::Module(_))) {
                continue;
            }
            let arguments = self.arguments_of(ty, bundle);
            let mut leaves = Vec::new();
            self.expand(owner, bundle, view, "", &arguments, &mut leaves, 0);
            out.push((owner, leaves));
        }
        out
    }

    /// The bundle struct `ty` names, through `flip` only (an array of bundles is `composite-values` C's).
    fn bundle_of(&self, ty: TypeId) -> Option<DeclId> {
        self.design.bundle_struct_of_type(ty)
    }

    /// The arguments the bundle type `ty` of the struct `bundle` is written with, one per generic parameter of the struct: the
    /// ones written, then the defaults of the parameters it leaves out.
    fn arguments_of(&self, ty: TypeId, bundle: DeclId) -> ParameterArguments {
        let mut at = ty;
        while let TypeKind::Flip(inner) = &self.design.type_node(at).kind {
            at = *inner;
        }
        let written = match &self.design.type_node(at).kind {
            TypeKind::Path(path) => match self.design.path_node(*path).segments.last().and_then(|segment| segment.args.clone()) {
                Some(GenericArgsNode::Angle { args, .. }) => args,
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        let mut arguments = ParameterArguments::new();
        let parameters = self.design.decl_data(bundle).children.iter().copied().filter(|child| matches!(self.design.decl_data(*child).kind, DeclKind::GenericParam(_)));
        for (index, parameter) in parameters.enumerate() {
            let given = written.get(index).cloned().or_else(|| match &self.design.decl_data(parameter).kind {
                DeclKind::GenericParam(GenericParamData::Type { default: Some(default), .. }) => Some(GenericArgNode::Type(*default)),
                DeclKind::GenericParam(GenericParamData::Const { default: Some(default), .. }) => Some(GenericArgNode::Const(*default)),
                _ => None,
            });
            if let Some(given) = given {
                arguments.insert(parameter, given);
            }
        }
        arguments
    }

    /// Appends the leaves of `bundle` seen with `view` under the prefix `prefix` of `owner`'s name, its generic parameters given
    /// by `arguments`; a nested bundle is a leaf of its own followed by its fields (a bundle that contains itself stops at a
    /// fixed depth: the checker reports it).
    #[allow(clippy::too_many_arguments)]
    fn expand(&mut self, owner: DeclId, bundle: DeclId, view: BundleView, prefix: &str, arguments: &ParameterArguments, out: &mut Vec<PlannedLeaf>, depth: usize) {
        if depth > 8 {
            return;
        }
        let owner_data = self.design.decl_data(owner).clone();
        let owner_name = owner_data.name.as_ref().map_or("", |name| name.text.as_str());
        for field in self.design.decl_data(bundle).children.clone() {
            let field_data = self.design.decl_data(field).clone();
            let DeclKind::Field(written) = &field_data.kind else { continue };
            let Some(name) = field_data.name.as_ref() else { continue };
            let path = if prefix.is_empty() { name.text.clone() } else { format!("{prefix}.{}", name.text) };
            let ty = TypeSubstitution::new(self.design, arguments).of_type(written.ty);
            let nested = self.bundle_of(ty);
            let direction = if nested.is_some() { None } else { view.field_direction(written.dir) };
            let kind = match (&owner_data.kind, nested) {
                (DeclKind::Port(_), nested) => DeclKind::Port(PortData { dir: direction, ty, default: None, kind: if nested.is_some() { PortKind::Bundle } else { PortKind::Carrier } }),
                _ => DeclKind::Signal(SignalData { ty, init: None }),
            };
            let data = DeclData {
                kind,
                name: Some(Name { text: format!("{owner_name}.{path}"), raw: false, span: name.span }),
                parent: owner_data.parent,
                children: Vec::new(),
                file: owner_data.file,
                // at the port or signal that has the field, where a diagnostic about a leaf and the host's declaration order point
                span: owner_data.span,
                generics_span: None,
                public: owner_data.public,
                meta: Meta::default(),
            };
            out.push(PlannedLeaf { data, path: path.clone(), nested: nested.is_some() });
            if let Some(inner) = nested {
                let inner_view = if matches!(self.design.type_node(written.ty).kind, TypeKind::Flip(_)) { view.flipped() } else { view };
                let inner_arguments = self.arguments_of(ty, inner);
                self.expand(owner, inner, inner_view, &path, &inner_arguments, out, depth + 1);
            }
        }
    }
}

/// A type written again with the generic parameters of a bundle `struct` replaced by the arguments it is instantiated with.
/// Only the nodes that change are written: a type that names no parameter is the field's own.
struct TypeSubstitution<'design> {
    design: &'design mut Design,
    arguments: &'design ParameterArguments,
}

impl<'design> TypeSubstitution<'design> {
    /// The substitution of `arguments` over `design`.
    fn new(design: &'design mut Design, arguments: &'design ParameterArguments) -> Self {
        TypeSubstitution { design, arguments }
    }

    /// The type `ty` with the parameters replaced; `ty` itself when it names none.
    fn of_type(&mut self, ty: TypeId) -> TypeId {
        if self.arguments.is_empty() {
            return ty;
        }
        let node = self.design.type_node(ty).clone();
        let kind = match &node.kind {
            TypeKind::Path(path) => {
                if let Some(GenericArgNode::Type(given)) = self.parameter_named_by_type(ty) {
                    return given;
                }
                match self.of_path(*path) {
                    Some(replaced) => TypeKind::Path(replaced),
                    None => return ty,
                }
            }
            TypeKind::Array { elem, len } => {
                let replaced = self.of_type(*elem);
                if replaced == *elem {
                    return ty;
                }
                TypeKind::Array { elem: replaced, len: *len }
            }
            TypeKind::Tuple(items) => {
                let replaced: Vec<TypeId> = items.iter().map(|item| self.of_type(*item)).collect();
                if replaced == *items {
                    return ty;
                }
                TypeKind::Tuple(replaced)
            }
            TypeKind::Flip(inner) => {
                let replaced = self.of_type(*inner);
                if replaced == *inner {
                    return ty;
                }
                TypeKind::Flip(replaced)
            }
            TypeKind::FnPtr { .. } | TypeKind::Infer => return ty,
        };
        self.design.push_type(TypeNode { kind, span: node.span, owner: node.owner })
    }

    /// The argument of the generic parameter a lone name `ty` is, if it is one.
    fn parameter_named_by_type(&self, ty: TypeId) -> Option<GenericArgNode> {
        let TypeKind::Path(path) = &self.design.type_node(ty).kind else { return None };
        self.parameter_named_by_path(self.design.path_node(*path))
    }

    /// The argument of the generic parameter a one-segment path is the name of, if it is one.
    fn parameter_named_by_path(&self, path: &PathNode) -> Option<GenericArgNode> {
        let [segment] = path.segments.as_slice() else { return None };
        let Resolution::Decl(parameter) = segment.res else { return None };
        self.arguments.get(&parameter).cloned()
    }

    /// The path `path` with the arguments of its last segment replaced; `None` when none of them names a parameter.
    fn of_path(&mut self, path: crate::pom::PathId) -> Option<crate::pom::PathId> {
        let node = self.design.path_node(path).clone();
        let last = node.segments.last()?;
        let Some(GenericArgsNode::Angle { args, turbofish, span }) = &last.args else { return None };
        let replaced: Vec<GenericArgNode> = args.iter().map(|argument| self.of_argument(argument)).collect();
        if replaced == *args {
            return None;
        }
        let mut segments = node.segments.clone();
        let index = segments.len() - 1;
        segments[index].args = Some(GenericArgsNode::Angle { args: replaced, turbofish: *turbofish, span: *span });
        Some(self.design.push_path(PathNode { segments, span: node.span, owner: node.owner }))
    }

    /// One generic argument with the parameters replaced: a name of a parameter (spelled as a type or as a constant) is
    /// what the parameter is given, any other type is replaced inside.
    fn of_argument(&mut self, argument: &GenericArgNode) -> GenericArgNode {
        match argument {
            GenericArgNode::Type(ty) => self.parameter_named_by_type(*ty).unwrap_or_else(|| GenericArgNode::Type(self.of_type(*ty))),
            GenericArgNode::Const(expr) => match &self.design.expr_node(*expr).kind {
                ExprKind::Path(path) => self.parameter_named_by_path(self.design.path_node(*path)).unwrap_or_else(|| argument.clone()),
                _ => argument.clone(),
            },
        }
    }
}

/// Adds the leaves of every bundle port and signal to the design.
pub(super) fn run(design: &mut Design) {
    let plan = LeafPlan::new(design).plan();
    for (owner, planned) in plan {
        let mut leaves = Vec::new();
        for leaf in planned {
            let decl = design.push_decl(leaf.data);
            leaves.push(BundleLeaf { decl, path: leaf.path, nested: leaf.nested });
        }
        design.add_bundle_leaves(owner, leaves);
    }
}
