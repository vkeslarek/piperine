//! A folded composite as an IR constant (CV-36; part I §1.5.25): the value `ConstEval` holds as a
//! tree (`ConstTree`) becomes a `Constant::Aggregate` of the builder's arena, its parts typed by the
//! IR type the checker gave the expression.
//!
//! A unit variant of an enum with no field is held as its tag, so where the
//! type is that enum it becomes the variant of the arena entry, as `types.rs` makes one.

use piperine_ir::{AggregateData, Constant, Expr, ExprId as LoweredExpr, Origin, Real, ValueType};

use super::ValueLowering;
use crate::elab::consts::{Bindings, CompositeKind, ConstFault, ConstTree};
use crate::elab::elaborator::Scope;
use crate::refusal::forms::{ConstantForm, StructureForm};
use crate::pom::ExprId;

impl ValueLowering<'_, '_, '_> {
    /// The constant a composite-typed const expression `expr` folds to over `bindings`, added to
    /// the IR's composite constants.
    pub(in crate::elab) fn composite_constant(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let tree = self.state.consts.tree(expr, bindings)?;
        let Some(ty) = self.typed(expr).and_then(|t| self.values().stored_type(&t)) else {
            return Err(ConstFault::refused(expr, StructureForm::CompositeValueOfTypeThatHasNo));
        };
        self.import_tree(expr, &tree, &ty)
    }

    /// A read of the composite `const` `decl` in value code: its tree written out as the expression
    /// that builds the value (a tuple, an array, a struct or a variant of constants). `None` after
    /// a diagnostic.
    pub(in crate::elab) fn lower_composite_constant(&mut self, scope: &Scope, decl: crate::pom::DeclId, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let tree = match self.state.consts.constant_tree(decl) {
            Ok(tree) => tree,
            Err(fault) => {
                self.const_fault(&fault, "a `const`");
                return None;
            }
        };
        let lowered = self.tree_expr(&tree, &ty, &origin);
        if lowered.is_none() {
            self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, ConstantForm::ConstCompositeOfTypeWithNoConstant);
        }
        lowered
    }

    /// `tree` as the expression of type `ty` that builds it. `None` for a range or a type with no
    /// constant form.
    pub(in crate::elab) fn tree_expr(&mut self, tree: &ConstTree, ty: &ValueType, origin: &Origin) -> Option<LoweredExpr> {
        match (tree, ty) {
            (ConstTree::Scalar(Constant::Int(position)), ValueType::Enum(id)) => Some(self.emit(Expr::VariantOf { ty: *id, variant: *position as u32, fields: Vec::new() }, ty.clone(), origin.clone())),
            (ConstTree::Scalar(Constant::Real(value)), _) => Some(self.emit(Expr::Real(Real::new(value.get())), ValueType::F64, origin.clone())),
            (ConstTree::Scalar(Constant::Bool(value)), _) => Some(self.emit(Expr::Bool(*value), ValueType::Bool, origin.clone())),
            (ConstTree::Scalar(Constant::Int(value)), _) => Some(self.emit(Expr::Int(*value), ty.clone(), origin.clone())),
            (ConstTree::Scalar(Constant::FloatBits(bits)), _) => Some(self.emit(Expr::FloatBits(*bits), ty.clone(), origin.clone())),
            (ConstTree::Scalar(Constant::Wide(id)), _) => Some(self.emit(Expr::Wide(*id), ty.clone(), origin.clone())),
            (ConstTree::Scalar(Constant::Aggregate(_)), _) => None,
            // an `Option` is its two halves packed: `Some(p)` is `(true, p)`, `None` is `(false, zero)` (`compound.rs`)
            (ConstTree::Composite { kind: CompositeKind::Option, parts }, ValueType::Option(payload)) => {
                let (is, value) = match parts.first() {
                    Some(held) => (self.emit(Expr::Bool(true), ValueType::Bool, origin.clone()), self.tree_expr(held, payload, origin)?),
                    None => (self.emit(Expr::Bool(false), ValueType::Bool, origin.clone()), self.values().zero(payload, origin.clone())?),
                };
                Some(self.emit(Expr::OptionPack { is, value }, ty.clone(), origin.clone()))
            }
            (ConstTree::Composite { kind, parts }, _) => {
                let wanted = match (kind, ty) {
                    (CompositeKind::Tuple, ValueType::Tuple(types)) => types.clone(),
                    (CompositeKind::Array, ValueType::Array(element, count)) => vec![(**element).clone(); *count as usize],
                    (CompositeKind::Struct, ValueType::Struct(id)) => self.state.values.type_arena.field_types(*id).to_vec(),
                    (CompositeKind::Variant(position), ValueType::Enum(id)) => self.state.values.type_arena.variant_field_types(*id, *position).to_vec(),
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

    /// The constant of `expr`, a value of type `T` bound to a param of type `Option<T>`: `Some` of it (§3.2.3, PI-03 AC 2).
    pub(in crate::elab) fn implicit_some_constant(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let payload = self.state.consts.tree(expr, bindings)?;
        let Some(held) = self.typed(expr).and_then(|ty| self.values().stored_type(&ty)) else {
            return Err(ConstFault::refused(expr, ConstantForm::ImplicitSomeOfValueWithNoValue));
        };
        self.import_tree(expr, &ConstTree::Composite { kind: CompositeKind::Option, parts: vec![payload] }, &ValueType::Option(Box::new(held)))
    }

    /// `tree` as a constant of type `ty`.
    pub(in crate::elab) fn import_tree(&mut self, expr: ExprId, tree: &ConstTree, ty: &ValueType) -> Result<Constant, ConstFault> {
        match (tree, ty) {
            // a unit variant of an enum with no field: its tag is the position of the variant
            (ConstTree::Scalar(Constant::Int(position)), ValueType::Enum(_)) => Ok(Constant::Aggregate(self.state.builder.aggregate(AggregateData { ty: ty.clone(), variant: Some(*position as u32), parts: Vec::new() }))),
            (ConstTree::Scalar(value), _) => Ok(*value),
            (ConstTree::Composite { kind, parts }, _) => {
                let (variant, wanted) = match (kind, ty) {
                    (CompositeKind::Tuple, ValueType::Tuple(types)) => (None, types.clone()),
                    (CompositeKind::Array, ValueType::Array(element, count)) => (None, vec![(**element).clone(); *count as usize]),
                    (CompositeKind::Struct, ValueType::Struct(id)) => (None, self.state.values.type_arena.field_types(*id).to_vec()),
                    (CompositeKind::Variant(position), ValueType::Enum(id)) => (Some(*position), self.state.values.type_arena.variant_field_types(*id, *position).to_vec()),
                    (CompositeKind::Option, ValueType::Option(inner)) => (None, vec![(**inner).clone(); parts.len()]),
                    _ => return Err(ConstFault::refused(expr, ConstantForm::CompositeValueThatDoesNotHaveType)),
                };
                if wanted.len() != parts.len() {
                    return Err(ConstFault::refused(expr, ConstantForm::CompositeValueWithDifferentNumberOfParts));
                }
                let mut constants = Vec::with_capacity(parts.len());
                for (part, part_ty) in parts.iter().zip(&wanted) {
                    constants.push(self.import_tree(expr, part, part_ty)?);
                }
                Ok(Constant::Aggregate(self.state.builder.aggregate(AggregateData { ty: ty.clone(), variant, parts: constants })))
            }
        }
    }
}
