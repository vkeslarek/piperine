//! The five functions of §22.1 that are generic over `Repr` (`merge`, `same`, `cast`, `logic`,
//! `matches_bits`) lowered by their normative contract at the concrete type the checker gave
//! them (`types::intrinsic`), until `generics` lets them compile from `std/values.msl` (spec Q1
//! (b)). Nothing outside this module and `types::intrinsic` knows they are special: when
//! `generics` lands, a differential test runs the stdlib source against this lowering and this
//! module is deleted.
//!
//! - `merge(a, b)` is `Expr::Merge`, always flagged (§22.1.2): §3.3.3 used as a function.
//! - `same(a, b)` is `Expr::Same`, a `bool` that is always known (§22.1.4).
//! - `cast::<A, B>(a)` is `Expr::LevelCast`: the levels of `a` under `B`, with its flag; an enum
//!   target lists its tags, a pattern outside them being `Unknown` (§22.1.3).
//! - `logic` and `matches_bits` follow in T29.

use piperine_ir::{Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::compound::EnumLayout;
use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::ExprId;
use crate::types::{Intrinsic, Ty};

impl Elaboration<'_, '_> {
    /// A call of `intrinsic`, whose call expression is `expr`, its arguments `args` and its value
    /// type `ty`.
    pub(super) fn lower_intrinsic(&mut self, scope: &mut Scope, expr: ExprId, intrinsic: Intrinsic, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        match (intrinsic, args) {
            (Intrinsic::Merge, [a, b]) => {
                let (a, b) = (self.lower_expr(scope, *a)?, self.lower_expr(scope, *b)?);
                Some(self.emit(Expr::Merge { a, b }, Flag::Flagged.wrap(ty), origin))
            }
            (Intrinsic::Same, [a, b]) => {
                let (a, b) = (self.lower_expr(scope, *a)?, self.lower_expr(scope, *b)?);
                Some(self.emit(Expr::Same { a, b }, ValueType::Bool, origin))
            }
            (Intrinsic::Cast, [value]) => {
                let operand = self.lower_expr(scope, *value)?;
                // an enum target decodes only the tags of its variants, so any other pattern is
                // `Unknown` and the result may be unknown whatever the operand was
                let (tags, flag) = match self.types.ty(expr) {
                    Some(Ty::Enum(decl)) => (Some(EnumLayout::of(self.design, *decl).tags()), Flag::Flagged),
                    _ => (None, self.flag(operand)),
                };
                Some(self.emit(Expr::LevelCast { expr: operand, to: ty.clone(), tags }, flag.wrap(ty), origin))
            }
            _ => {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "a function of §22.1 called with the wrong number of arguments");
                None
            }
        }
    }
}
