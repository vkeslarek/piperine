//! A folded composite as an IR constant (CV-36; part I §1.5.25): the value `ConstEval` holds as a
//! tree (`ConstTree`) becomes a `Constant::Aggregate` of the builder's arena, its parts typed by the
//! IR type the checker gave the expression.
//!
//! A unit variant of an enum with no field is held as its tag, so where the
//! type is that enum it becomes the variant of the arena entry, as `types.rs` makes one.

use piperine_ir::{AggregateData, Constant, Expr, ExprId as LoweredExpr, Origin, Real, ValueType};

use super::super::elaborator::Elaboration;
use super::super::lower::Scope;
use super::{Bindings, CompositeKind, ConstFault, ConstTree};
use crate::pom::ExprId;

impl Elaboration<'_, '_> {
    /// The constant a composite-typed const expression `expr` folds to over `bindings`, added to
    /// the IR's composite constants.
    pub(in crate::elab) fn composite_constant(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let tree = self.consts.tree(expr, bindings)?;
        let Some(ty) = self.typed(expr).and_then(|t| self.value_type(&t)) else {
            return Err(ConstFault::Unsupported { expr, what: "a composite value of a type that has no value type".to_string() });
        };
        self.import_tree(expr, &tree, &ty)
    }

    /// `value`, a param's constant, bound to `param` in `bindings`; a composite is also bound taken
    /// apart, for the evaluator that reads it (a `where`, a value over the param).
    pub(in crate::elab) fn bind_param(&self, bindings: &mut Bindings, param: crate::pom::DeclId, value: Constant) {
        match value {
            Constant::Aggregate(_) => bindings.bind_composite(param, value, self.imported_tree(value)),
            other => bindings.bind(param, other),
        }
    }

    /// The constant `value` of the IR's arena as the tree the evaluator reads: the reverse of
    /// `import_tree`. A unit variant of an enum with no field is its tag.
    fn imported_tree(&self, value: Constant) -> ConstTree {
        let Constant::Aggregate(id) = value else { return ConstTree::Scalar(value) };
        let data = self.ir.aggregate_at(id);
        let parts: Vec<ConstTree> = data.parts.iter().map(|part| self.imported_tree(*part)).collect();
        let kind = match (&data.ty, data.variant) {
            (ValueType::Tuple(_), _) => CompositeKind::Tuple,
            (ValueType::Array(..), _) => CompositeKind::Array,
            (ValueType::Option(_), _) => CompositeKind::Option,
            (ValueType::Enum(type_id), Some(position)) => {
                if self.type_arena.is_unit_only(*type_id) {
                    return ConstTree::Scalar(Constant::Int(i128::from(position)));
                }
                CompositeKind::Variant(position)
            }
            _ => CompositeKind::Struct,
        };
        ConstTree::Composite { kind, parts }
    }

    /// A read of the composite `const` `decl` in value code: its tree written out as the expression
    /// that builds the value (a tuple, an array, a struct or a variant of constants). `None` after
    /// a diagnostic.
    pub(in crate::elab) fn lower_composite_constant(&mut self, scope: &Scope, decl: crate::pom::DeclId, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let tree = match self.consts.constant_tree(decl) {
            Ok(tree) => tree,
            Err(fault) => {
                self.const_fault(&fault, "a `const`");
                return None;
            }
        };
        let lowered = self.tree_expr(&tree, &ty, &origin);
        if lowered.is_none() {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a `const` composite of a type with no constant form");
        }
        lowered
    }

    /// `tree` as the expression of type `ty` that builds it. `None` for a range or a type with no
    /// constant form.
    fn tree_expr(&mut self, tree: &ConstTree, ty: &ValueType, origin: &Origin) -> Option<LoweredExpr> {
        match (tree, ty) {
            (ConstTree::Scalar(Constant::Int(position)), ValueType::Enum(id)) => Some(self.emit(Expr::VariantOf { ty: *id, variant: *position as u32, fields: Vec::new() }, ty.clone(), origin.clone())),
            (ConstTree::Scalar(Constant::Real(value)), _) => Some(self.emit(Expr::Real(Real::new(value.get())), ValueType::F64, origin.clone())),
            (ConstTree::Scalar(Constant::Bool(value)), _) => Some(self.emit(Expr::Bool(*value), ValueType::Bool, origin.clone())),
            (ConstTree::Scalar(Constant::Int(value)), _) => Some(self.emit(Expr::Int(*value), ty.clone(), origin.clone())),
            (ConstTree::Scalar(Constant::Aggregate(_)), _) => None,
            (ConstTree::Composite { kind, parts }, _) => {
                let wanted = match (kind, ty) {
                    (CompositeKind::Tuple, ValueType::Tuple(types)) => types.clone(),
                    (CompositeKind::Array, ValueType::Array(element, count)) => vec![(**element).clone(); *count as usize],
                    (CompositeKind::Struct, ValueType::Struct(id)) => self.type_arena.field_types(*id).to_vec(),
                    (CompositeKind::Variant(position), ValueType::Enum(id)) => self.type_arena.variant_field_types(*id, *position).to_vec(),
                    _ => return None,
                };
                let mut lowered = Vec::with_capacity(parts.len());
                for (part, part_ty) in parts.iter().zip(&wanted) {
                    lowered.push(self.tree_expr(part, part_ty, origin)?);
                }
                let built = match (kind, ty) {
                    (CompositeKind::Tuple, _) => Expr::Tuple(lowered),
                    (CompositeKind::Array, _) => Expr::Array(lowered),
                    (CompositeKind::Struct, ValueType::Struct(id)) => Expr::StructOf { ty: *id, fields: lowered },
                    (CompositeKind::Variant(position), ValueType::Enum(id)) => Expr::VariantOf { ty: *id, variant: *position, fields: lowered },
                    _ => return None,
                };
                Some(self.emit(built, ty.clone(), origin.clone()))
            }
        }
    }

    /// `tree` as a constant of type `ty`.
    fn import_tree(&mut self, expr: ExprId, tree: &ConstTree, ty: &ValueType) -> Result<Constant, ConstFault> {
        match (tree, ty) {
            // a unit variant of an enum with no field: its tag is the position of the variant
            (ConstTree::Scalar(Constant::Int(position)), ValueType::Enum(_)) => Ok(Constant::Aggregate(self.ir.aggregate(AggregateData { ty: ty.clone(), variant: Some(*position as u32), parts: Vec::new() }))),
            (ConstTree::Scalar(value), _) => Ok(*value),
            (ConstTree::Composite { kind, parts }, _) => {
                let (variant, wanted) = match (kind, ty) {
                    (CompositeKind::Tuple, ValueType::Tuple(types)) => (None, types.clone()),
                    (CompositeKind::Array, ValueType::Array(element, count)) => (None, vec![(**element).clone(); *count as usize]),
                    (CompositeKind::Struct, ValueType::Struct(id)) => (None, self.type_arena.field_types(*id).to_vec()),
                    (CompositeKind::Variant(position), ValueType::Enum(id)) => (Some(*position), self.type_arena.variant_field_types(*id, *position).to_vec()),
                    (CompositeKind::Option, ValueType::Option(inner)) => (None, vec![(**inner).clone(); parts.len()]),
                    _ => return Err(ConstFault::Unsupported { expr, what: "a composite value that does not have the type its position needs".to_string() }),
                };
                if wanted.len() != parts.len() {
                    return Err(ConstFault::Unsupported { expr, what: "a composite value with a different number of parts than its type".to_string() });
                }
                let mut constants = Vec::with_capacity(parts.len());
                for (part, part_ty) in parts.iter().zip(&wanted) {
                    constants.push(self.import_tree(expr, part, part_ty)?);
                }
                Ok(Constant::Aggregate(self.ir.aggregate(AggregateData { ty: ty.clone(), variant, parts: constants })))
            }
        }
    }
}
