//! The two functions of §22.1 that are generic over `Repr` and name no `Logic` (`same`, `cast`)
//! lowered by their normative contract at the concrete type the checker gave them
//! (`types::intrinsic`), until `generics` lets them compile from `std/values.msl` (spec Q1
//! (b)). Nothing outside this module and `types::intrinsic` knows they are special: when
//! `generics` lands, a differential test runs the stdlib source against this lowering and this
//! module is deleted. `logic` and `matches_bits` are `P0001` until `Logic` (T45).
//!
//! - `same(a, b)` is `Expr::Same`, a `bool` (§22.1.4).
//! - `cast::<A, B>(a)` is `Expr::LevelCast`, an `Option<B>`: the levels of `a` under `B`, `None` for a
//!   tag that names no variant of an enum `B`, which lists its tags (§22.1.3).

use piperine_ir::{Expr, ExprId as LoweredExpr, LevelPart, Origin, ValueType};

use super::compound::Pair;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::ExprId;
use crate::types::{Intrinsic, Nominal, Ty};

impl Elaboration<'_, '_> {
    /// A call of `same`, whose call expression is `expr` and its arguments `args`; a `cast` is an
    /// `Option` and lowers as a pair (`lower_cast`).
    pub(super) fn lower_intrinsic(&mut self, scope: &mut Scope, expr: ExprId, intrinsic: Intrinsic, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        match (intrinsic, args) {
            (Intrinsic::Same, [a, b]) => {
                let (a, b) = (self.lower_expr(scope, *a)?, self.lower_expr(scope, *b)?);
                Some(self.emit(Expr::Same { a, b }, ValueType::Bool, origin))
            }
            _ => {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "a function of §22.1 called with the wrong number of arguments, or a `cast` read as a whole value");
                None
            }
        }
    }

    /// `cast::<A, B>(a)` (§22.1.3) as the pair an `Option<B>` is in value code: the flag and the payload
    /// are the two halves of one `LevelCast`, which is `None` for a tag that names no variant of an enum
    /// `B` (it lists the tags of its variants). `None` after a diagnostic.
    pub(super) fn lower_cast(&mut self, scope: &mut Scope, expr: ExprId, value: ExprId) -> Option<Pair> {
        let origin = self.expr_origin(expr);
        let Some(Ty::Option(to)) = self.types.ty(expr).cloned() else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a `cast` the checker did not type as an `Option`");
            return None;
        };
        let Some(target) = self.value_type(&to) else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a `cast` to a type that has no value type in the IR");
            return None;
        };
        let tags = match &*to {
            Ty::Enum(Nominal { decl, .. }) => self.types.enum_tag(*decl).map(|t| t.discriminants.clone()),
            _ => None,
        };
        let operand = self.lower_expr(scope, value)?;
        let packed = self.emit(Expr::LevelCast { expr: operand, to: target.clone(), tags }, ValueType::Option(Box::new(target.clone())), origin.clone());
        let is = self.emit(Expr::OptionPart { option: packed, part: LevelPart::Is }, ValueType::Bool, origin.clone());
        let val = self.emit(Expr::OptionPart { option: packed, part: LevelPart::Value }, target, origin);
        Some(Pair { is, val, literal: None })
    }
}
