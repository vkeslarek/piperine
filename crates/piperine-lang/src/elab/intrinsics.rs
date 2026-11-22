//! The two functions of §22.1 that are generic over `Repr` and name no `Logic` (`same`, `cast`)
//! lowered by their normative contract at the concrete type the checker gave them
//! (`types::intrinsic`), until `generics` lets them compile from `std/values.msl` (spec Q1
//! (b)). Nothing outside this module and `types::intrinsic` knows they are special: when
//! `generics` lands, a differential test runs the stdlib source against this lowering and this
//! module is deleted. `logic` and `matches_bits` are `P0001` until `Logic` (T45).
//!
//! - `same(a, b)` is `Expr::Same`, a `bool` (§22.1.4).
//! - `cast::<A, B>(a)` is `Expr::LevelCast`: the levels of `a` under `B`; an enum target lists
//!   its tags (§22.1.3).

use piperine_ir::{Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::ExprId;
use crate::types::{Intrinsic, Ty};

impl Elaboration<'_, '_> {
    /// A call of `intrinsic`, whose call expression is `expr`, its arguments `args` and its value
    /// type `ty`.
    pub(super) fn lower_intrinsic(&mut self, scope: &mut Scope, expr: ExprId, intrinsic: Intrinsic, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        match (intrinsic, args) {
            (Intrinsic::Same, [a, b]) => {
                let (a, b) = (self.lower_expr(scope, *a)?, self.lower_expr(scope, *b)?);
                Some(self.emit(Expr::Same { a, b }, ValueType::Bool, origin))
            }
            (Intrinsic::Cast, [value]) => {
                let operand = self.lower_expr(scope, *value)?;
                // an enum target decodes the tags of its variants
                let tags = match self.types.ty(expr) {
                    Some(Ty::Enum(decl)) => self.types.enum_tag(*decl).map(|t| t.discriminants.clone()),
                    _ => None,
                };
                Some(self.emit(Expr::LevelCast { expr: operand, to: ty.clone(), tags }, ty, origin))
            }
            _ => {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "a function of §22.1 called with the wrong number of arguments");
                None
            }
        }
    }
}
