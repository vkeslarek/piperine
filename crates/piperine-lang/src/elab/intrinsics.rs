//! The functions of §22.1 that read the characters of a `str`, which no definition can, so they stay intrinsic
//! (`types::intrinsic`); `cast` and `same` are ordinary generic functions of `std/values.msl` since `generics` T55:
//!
//! - `logic(p)` is the `Logic<N>` struct whose two planes the pattern names.
//! - `matches_bits(s, p)` compares the planes of `s` against the masks of the pattern; `x` constrains no level.

use piperine_ir::{Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::ExprId;
use crate::types::{ConstTerm, GenericValue, Intrinsic, LogicPattern, Nominal, Ty};
use piperine_ir::BinaryOp;

impl Elaboration<'_, '_> {
    /// A call of an intrinsic whose call expression is `expr` and its arguments `args`.
    pub(super) fn lower_intrinsic(&mut self, scope: &mut Scope, expr: ExprId, intrinsic: Intrinsic, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        match (intrinsic, args) {
            (Intrinsic::Logic, [pattern]) => self.lower_logic_pattern(scope, expr, *pattern, origin),
            (Intrinsic::MatchesBits, [subject, pattern]) => self.lower_matches_bits(scope, expr, *subject, *pattern, origin),
            (Intrinsic::TableModel, _) => self.lower_table_model(scope, expr, args, origin),
            (Intrinsic::ParseTable | Intrinsic::ParseHex, _) => {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "`parse_table` or `parse_hex` outside a `const`: a run never reads a file (§20)");
                None
            }
            _ => {
                self.unsupported(scope.file, self.design.expr_node(expr).span, "a function of §22.1 called with the wrong number of arguments");
                None
            }
        }
    }

    /// The levels of the `Logic<N>` type `ty` (after the instance's substitution), `None` for any other type.
    fn logic_levels(&self, ty: &Ty) -> Option<u32> {
        let Ty::Struct(Nominal { arguments, .. }) = ty else { return None };
        match arguments.first() {
            Some(GenericValue::Const(ConstTerm::Value(width))) => u32::try_from(*width).ok(),
            _ => None,
        }
    }

    /// The pattern a call names: its text (a literal, or a `str` the front end knows), parsed, and `E0702` for a character
    /// that names no level or a length other than `levels` (§22.1.4 rule 2). `None` after a diagnostic.
    fn call_pattern(&mut self, scope: &Scope, pattern: ExprId, levels: u32) -> Option<LogicPattern> {
        let span = self.design.expr_node(pattern).span;
        let Some(text) = self.text_of(scope, pattern) else {
            self.unsupported(scope.file, span, "a pattern that is not a text known where it is read");
            return None;
        };
        let parsed = match LogicPattern::parse(&text) {
            Ok(parsed) => parsed,
            Err(character) => {
                let message = format!("the pattern `{text}` holds `{character}`, which is none of `0`, `1`, `x` and `z` (§22.1.4 rule 2)");
                self.report(scope.file, crate::msl::Diagnostic::new("E0702", message, span, "not a level of a pattern"));
                return None;
            }
        };
        if parsed.width() != levels as usize {
            let message = format!("the pattern `{text}` has {} characters, and a `Logic<{levels}>` has {levels} levels (§22.1.4 rule 2)", parsed.width());
            self.report(scope.file, crate::msl::Diagnostic::new("E0702", message, span, "one character per level"));
            return None;
        }
        Some(parsed)
    }

    /// `logic(p)`: the struct of the two planes the pattern names.
    fn lower_logic_pattern(&mut self, scope: &mut Scope, expr: ExprId, pattern: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let ty = self.typed(expr)?;
        let (Some(levels), Some(stored)) = (self.logic_levels(&ty), self.value_type(&ty)) else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a `logic` call whose type is not a `Logic<N>` of known width");
            return None;
        };
        let (binary, value) = self.call_pattern(scope, pattern, levels)?.planes();
        let ValueType::Struct(id) = stored else { return None };
        let plane = ValueType::UInt(levels);
        let fields = vec![self.emit(Expr::Int(binary as i128), plane.clone(), origin.clone()), self.emit(Expr::Int(value as i128), plane, origin.clone())];
        Some(self.emit(Expr::StructOf { ty: id, fields }, stored, origin))
    }

    /// `matches_bits(s, p)`: both planes of `s` under the masks of the pattern equal what the pattern wants.
    fn lower_matches_bits(&mut self, scope: &mut Scope, expr: ExprId, subject: ExprId, pattern: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let subject_type = self.typed(subject)?;
        let Some(levels) = self.logic_levels(&subject_type) else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a `matches_bits` subject that is not a `Logic<N>` of known width");
            return None;
        };
        let masks = self.call_pattern(scope, pattern, levels)?.masks();
        let subject = self.lower_expr(scope, subject)?;
        let plane = ValueType::UInt(levels);
        let mut checks = Vec::new();
        for (index, mask, wanted) in [(0u32, masks.binary_mask, masks.binary_wanted), (1, masks.value_mask, masks.value_wanted)] {
            let field = self.emit(Expr::Field { expr: subject, index }, plane.clone(), origin.clone());
            let mask = self.emit(Expr::Int(mask as i128), plane.clone(), origin.clone());
            let wanted = self.emit(Expr::Int(wanted as i128), plane.clone(), origin.clone());
            let held = self.emit(Expr::Binary { op: BinaryOp::BitAnd, lhs: field, rhs: mask }, plane.clone(), origin.clone());
            checks.push(self.emit(Expr::Binary { op: BinaryOp::Eq, lhs: held, rhs: wanted }, ValueType::Bool, origin.clone()));
        }
        Some(self.emit(Expr::Binary { op: BinaryOp::And, lhs: checks[0], rhs: checks[1] }, ValueType::Bool, origin))
    }
}
