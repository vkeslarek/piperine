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
//! - `logic::<T>("..")` is the value `ConstEval` folds from its pattern (`ConstEval::logic`).
//! - `matches_bits(s, "..")` is the conjunction, over the levels the pattern cares about, of
//!   `s.level(k) == Some(bit)` (§22.1.4 rule 2: "a loop over `level`"), `true` for a pattern of
//!   only `x`.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, LevelPart, Origin, UnaryOp, ValueType};

use super::consts::{Bindings, Folded};
use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{ExprId, ExprKind};
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
                    Some(Ty::Enum(decl)) => (self.types.enum_tag(*decl).map(|t| t.discriminants.clone()), Flag::Flagged),
                    _ => (None, self.flag(operand)),
                };
                Some(self.emit(Expr::LevelCast { expr: operand, to: ty.clone(), tags }, flag.wrap(ty), origin))
            }
            (Intrinsic::Logic, _) => match self.consts.folded(expr, &Bindings::new()) {
                Ok(Folded::Known(value)) => Some(self.constant_expr(value, ty, origin)),
                Ok(Folded::Unknown) => Some(self.unknown_expr(ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `logic` pattern");
                    None
                }
            },
            (Intrinsic::MatchesBits, [value, pattern]) => self.lower_matches_bits(scope, *value, *pattern, origin),
            _ => {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "a function of §22.1 called with the wrong number of arguments");
                None
            }
        }
    }

    /// `matches_bits(value, pattern)`: known `true` exactly when every level the pattern cares
    /// about is known and holds its bit (`x` matches any level, `0` and `1` only a known level of
    /// that value). The pattern's leftmost character is the highest level (§22.1.4 rule 3).
    fn lower_matches_bits(&mut self, scope: &mut Scope, value: ExprId, pattern: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let text = self.pattern_text(pattern)?;
        let levels = text.chars().count() as u32;
        let mut terms = Vec::new();
        for (position, character) in text.chars().enumerate() {
            let bit = match character {
                '0' => false,
                '1' => true,
                _ => continue,
            };
            terms.push(self.level_matches(scope, value, levels - 1 - position as u32, bit, &origin)?);
        }
        let mut terms = terms.into_iter();
        let Some(first) = terms.next() else {
            return Some(self.emit(Expr::Bool(true), ValueType::Bool, origin));
        };
        Some(terms.fold(first, |lhs, rhs| self.emit(Expr::Binary { op: BinaryOp::And, lhs, rhs }, ValueType::Bool, origin.clone())))
    }

    /// `value.level(level) == Some(bit)`: the level is known and holds `bit`. The value is read
    /// again for each half, where it stands, as `x.known()` reads its receiver twice.
    fn level_matches(&mut self, scope: &mut Scope, value: ExprId, level: u32, bit: bool, origin: &Origin) -> Option<LoweredExpr> {
        let mut half = |this: &mut Self, part: LevelPart| {
            let read = this.lower_expr(scope, value)?;
            let index = this.emit(Expr::Int(i128::from(level)), ValueType::UInt(32), origin.clone());
            Some(this.emit(Expr::Level { value: read, index, part }, ValueType::Bool, origin.clone()))
        };
        let is = half(self, LevelPart::Is)?;
        let holds = half(self, LevelPart::Value)?;
        let holds = if bit { holds } else { self.emit(Expr::Unary { op: UnaryOp::Not, expr: holds }, ValueType::Bool, origin.clone()) };
        Some(self.emit(Expr::Binary { op: BinaryOp::And, lhs: is, rhs: holds }, ValueType::Bool, origin.clone()))
    }

    /// The text of a pattern argument, a string literal (`E0702` otherwise, which the checker
    /// reported).
    pub(super) fn pattern_text(&self, pattern: ExprId) -> Option<String> {
        match &self.design.expr_node(pattern).kind {
            ExprKind::Lit(lit) => lit.str_value(),
            _ => None,
        }
    }
}
