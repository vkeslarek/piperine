//! Stage 6b: the leaves of bundle ports and signals (spec part II §7 rules 3 to 7 and 14). Each port or
//! signal of a module whose type is a bundle gets one declaration per field, named by its dotted path
//! (`tx.valid`), typed as the field is, with the direction the module's view gives it. They are recorded in
//! [`BundleLeaves`](super::super::bundles::BundleLeaves), never in a module's `children` (the POM mirrors
//! the source). A bundle that is generic (`Stream<u8>`) has the leaves of its instance: each field's type is written
//! again with the struct's parameters replaced by the arguments of the port (`TypeSubstitution`), so a leaf is typed like
//! any port of its module and no later pass substitutes. An array of bundles (`tx: [Lane; N]`) is a struct of arrays: each
//! field is one leaf typed as the array of the field's type with the owner's lengths (`tx.a: [Bit; N]`), so an element
//! `tx[k].a` is the element `k` of the leaf `tx.a` and every rule of arrays of carriers (generic lengths folded per instance,
//! elements judged one by one, connections by window) applies to it unchanged.

use std::collections::BTreeMap;

use super::super::body::{ExprKind, ExprNode, GenericArgNode, GenericArgsNode, PathNode, TypeKind, TypeNode};
use super::super::bundles::{BundleLeaf, BundleView};
use super::super::decl::{DeclData, DeclKind, GenericParamData, Name, PortData, SignalData};
use super::super::design::Design;
use super::super::ids::{DeclId, ExprId, TypeId};
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

/// The owner whose leaves are being planned, and the lengths of its element levels (none for a single bundle).
struct LeafSite<'lengths> {
    owner: DeclId,
    lengths: &'lengths [ExprId],
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

    /// The leaves of every owner, in the order of the arena, each with the number of element levels of its owner.
    fn plan(&mut self) -> Vec<(DeclId, Vec<PlannedLeaf>, usize)> {
        let mut out = Vec::new();
        for index in 0..self.design.decl_count() {
            let owner = DeclId(index as u32);
            let data = self.design.decl_data(owner);
            let (ty, view) = match &data.kind {
                DeclKind::Port(port) => (port.ty, BundleView::of_port(self.design, port.dir, port.ty)),
                DeclKind::Signal(signal) => (signal.ty, BundleView::Producer),
                _ => continue,
            };
            // a bundle, or an array of them (`[flip Stream<u8>; N]`): each field is one leaf, an array of the field's type when the
            // owner is an array (struct of arrays, design D12), whatever its length is written as
            let (element, lengths) = self.element_levels(ty);
            let Some(bundle) = self.bundle_of(element) else { continue };
            // a port of the module, not of a function or a generation arm already unrolled elsewhere
            if !data.parent.is_some_and(|p| matches!(self.design.decl_data(p).kind, DeclKind::Module(_))) {
                continue;
            }
            let element_view = if matches!(self.design.type_node(element).kind, TypeKind::Flip(_)) && !lengths.is_empty() { view.flipped() } else { view };
            let arguments = self.arguments_of(element, bundle);
            let mut leaves = Vec::new();
            let site = LeafSite { owner, lengths: &lengths };
            self.expand(&site, bundle, element_view, "", &arguments, &mut leaves, 0);
            out.push((owner, leaves, lengths.len()));
        }
        out
    }

    /// The element type of `ty` and the lengths of the arrays around it, outermost first, when `ty` is an array of bundles
    /// (`[Lane; N]`, `[[flip Stream<u8>; 2]; 3]`); `ty` itself and no length for any other type.
    fn element_levels(&self, ty: TypeId) -> (TypeId, Vec<ExprId>) {
        let mut lengths = Vec::new();
        let mut at = ty;
        while let TypeKind::Array { elem, len } = &self.design.type_node(at).kind {
            lengths.push(*len);
            at = *elem;
        }
        if self.bundle_of(at).is_some() { (at, lengths) } else { (ty, Vec::new()) }
    }

    /// The type of a leaf whose field is typed `field` under an owner of `lengths` element levels: the field's type, or the array
    /// of it with those lengths, outermost first (`tx.a: [Bit; N]` of `tx: [Lane; N]`).
    fn leaf_type(&mut self, field: TypeId, lengths: &[ExprId]) -> TypeId {
        let mut ty = field;
        for length in lengths.iter().rev() {
            let span = self.design.type_node(field).span;
            let owner = self.design.type_node(field).owner;
            ty = self.design.push_type(TypeNode { kind: TypeKind::Array { elem: ty, len: *length }, span, owner });
        }
        ty
    }

    /// The name of the leaf at `path` under the owner `owner_name`: `tx.valid`, `tx.inner.x`.
    fn leaf_name(owner_name: &str, path: &str) -> String {
        format!("{owner_name}.{path}")
    }

    /// The bundle struct `ty` names, through `flip` only (an array of bundles is taken apart by `element_levels`).
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

    /// Appends the leaves of `bundle` seen with `view` under the prefix `prefix` of the owner's name, its generic parameters given
    /// by `arguments`; a nested bundle is a leaf of its own followed by its fields (a bundle that contains itself stops at a
    /// fixed depth: the checker reports it).
    fn expand(&mut self, site: &LeafSite, bundle: DeclId, view: BundleView, prefix: &str, arguments: &ParameterArguments, out: &mut Vec<PlannedLeaf>, depth: usize) {
        if depth > 8 {
            return;
        }
        let owner_data = self.design.decl_data(site.owner).clone();
        let owner_name = owner_data.name.as_ref().map_or("", |name| name.text.as_str());
        for field in self.design.decl_data(bundle).children.clone() {
            let field_data = self.design.decl_data(field).clone();
            let DeclKind::Field(written) = &field_data.kind else { continue };
            let Some(name) = field_data.name.as_ref() else { continue };
            let path = if prefix.is_empty() { name.text.clone() } else { format!("{prefix}.{}", name.text) };
            let field_ty = TypeSubstitution::new(self.design, arguments).of_type(written.ty);
            let nested = self.bundle_of(field_ty);
            let ty = self.leaf_type(field_ty, site.lengths);
            let direction = if nested.is_some() { None } else { view.field_direction(written.dir) };
            let kind = match (&owner_data.kind, nested) {
                (DeclKind::Port(_), nested) => DeclKind::Port(PortData { dir: direction, ty, default: None, kind: if nested.is_some() { PortKind::Bundle } else { PortKind::Carrier } }),
                _ => DeclKind::Signal(SignalData { ty, init: None }),
            };
            let data = DeclData {
                kind,
                name: Some(Name { text: Self::leaf_name(owner_name, &path), raw: false, span: name.span }),
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
                let inner_arguments = self.arguments_of(field_ty, inner);
                self.expand(site, inner, inner_view, &path, &inner_arguments, out, depth + 1);
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
                let length = self.of_length(*len);
                if replaced == *elem && length == *len {
                    return ty;
                }
                TypeKind::Array { elem: replaced, len: length }
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

    /// The length expression `expr` of an array type written again with the constant parameters replaced: a name of a parameter is
    /// the constant it is given, and the operators over such names (`W + 1`, `(W)`, `-W`) are written again over what they are
    /// given; any other expression is its own.
    fn of_length(&mut self, expr: ExprId) -> ExprId {
        let node = self.design.expr_node(expr).clone();
        let kind = match &node.kind {
            ExprKind::Path(path) => return self.length_named_by_path(*path, &node).unwrap_or(expr),
            ExprKind::Paren(inner) => match self.of_length(*inner) {
                replaced if replaced == *inner => return expr,
                replaced => ExprKind::Paren(replaced),
            },
            ExprKind::Unary { op, expr: operand } => match self.of_length(*operand) {
                replaced if replaced == *operand => return expr,
                replaced => ExprKind::Unary { op: *op, expr: replaced },
            },
            ExprKind::Binary { op, lhs, rhs } => {
                let (left, right) = (self.of_length(*lhs), self.of_length(*rhs));
                if (left, right) == (*lhs, *rhs) {
                    return expr;
                }
                ExprKind::Binary { op: *op, lhs: left, rhs: right }
            }
            _ => return expr,
        };
        self.design.push_expr(ExprNode { kind, span: node.span, owner: node.owner })
    }

    /// What the parameter a one-segment path `path` names is given, as an expression: the constant written, or the name written
    /// as a type (`Word<W>` where `W` is a parameter of the module) read as the name of a constant. `None` for any other path.
    fn length_named_by_path(&mut self, path: crate::pom::PathId, written: &ExprNode) -> Option<ExprId> {
        match self.parameter_named_by_path(self.design.path_node(path))? {
            GenericArgNode::Const(given) => Some(given),
            GenericArgNode::Type(given) => match self.design.type_node(given).kind.clone() {
                TypeKind::Path(named) => Some(self.design.push_expr(ExprNode { kind: ExprKind::Path(named), span: written.span, owner: written.owner })),
                _ => None,
            },
        }
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
    for (owner, planned, levels) in plan {
        let mut leaves = Vec::new();
        for leaf in planned {
            let decl = design.push_decl(leaf.data);
            leaves.push(BundleLeaf { decl, path: leaf.path, nested: leaf.nested });
        }
        design.add_bundle_leaves(owner, leaves, levels);
    }
}
