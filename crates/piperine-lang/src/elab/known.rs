//! Unknown-ness (part I §3.3): which lowered values may be unknown.
//!
//! The checker types `count + 1` as `u9`: a flag is not part of a type. The elaborator gives a
//! value that may be unknown the IR type `Flagged` (`flagged<u9>`), by one rule per form, read off
//! the operands already lowered:
//!
//! | Expression | Answer |
//! | --- | --- |
//! | literal, const, param, probe, `now()`, `analysis(Static)`, environment | plain |
//! | a carrier read, `Unknown` | flagged |
//! | `known_or`, `is_known`, `high` | plain |
//! | any other operator, cast, method or `if` | flagged when an operand, the condition or an arm is |
//! | a local | what its initializer was |
//! | a call | the result of the variant its argument pattern selects |
//!
//! `&&` and `||` decide control, so the IR takes them over known operands only. With an operand
//! that may be unknown they are the bitwise `&` and `|`, which §3.3.12 gives the same table; the
//! right operand then runs whatever the left gives, so one that can panic or emit is refused.
//! An `if` on a value that may be unknown is lowered as written: the generated code decides it,
//! and faults when it is unknown (`ir-rust` RS-11 AC 5).

use piperine_ir::{Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{BinOp, ExprId, ExprKind, StmtKind};
use crate::types::{Builtin, Callee};

/// Whether a value may be unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Flag {
    Plain,
    Flagged,
}

impl Flag {
    /// The flag a lowered value of type `ty` carries.
    pub(super) fn of(ty: &ValueType) -> Flag {
        if matches!(ty, ValueType::Flagged(_)) { Flag::Flagged } else { Flag::Plain }
    }

    /// Flagged when either is (§3.3.2: one unknown operand makes the result unknown).
    pub(super) fn or(self, other: Flag) -> Flag {
        self.max(other)
    }

    /// `base`, flagged when this is `Flagged` and the base is a scalar a flag can ride on
    /// (`bool`, `f64`, an integer): the IR's own rule (`piperine_ir` typing, `Typing::flag`).
    pub(super) fn wrap(self, base: ValueType) -> ValueType {
        let scalar = matches!(base, ValueType::Bool | ValueType::F64) || base.is_integer();
        if self == Flag::Flagged && scalar { ValueType::Flagged(Box::new(base)) } else { base }
    }
}

impl Elaboration<'_, '_> {
    /// The flag of a lowered expression.
    pub(super) fn flag(&self, expr: LoweredExpr) -> Flag {
        Flag::of(&self.type_of(expr))
    }

    /// The flag of a result that has these operands.
    pub(super) fn flags(&self, operands: &[LoweredExpr]) -> Flag {
        operands.iter().fold(Flag::Plain, |acc, e| acc.or(self.flag(*e)))
    }

    /// Why the right operand of an `&&` or `||` over a value that may be unknown cannot be
    /// lowered as a bitwise operator: it holds an integer `/` or `%` (which may panic) or a call
    /// of a function that emits a record. `None` when it can run unconditionally.
    pub(super) fn partial(&self, expr: ExprId) -> Option<&'static str> {
        let mut stack = vec![expr];
        while let Some(e) = stack.pop() {
            let node = self.design.expr_node(e);
            match &node.kind {
                ExprKind::Binary { op: BinOp::Div | BinOp::Rem, .. } if self.types.ty(e).is_some_and(|t| t.is_integer()) => return Some("an integer `/` or `%`"),
                ExprKind::Call { .. } if matches!(self.types.callee(e), Some(Callee::Function(f)) if self.types.emits(f).is_some()) => return Some("a call of a function that emits a record"),
                _ => {}
            }
            stack.extend(self.operands_of(&node.kind));
        }
        None
    }

    /// The direct sub-expressions of a POM expression, with those of its blocks' statements.
    fn operands_of(&self, kind: &ExprKind) -> Vec<ExprId> {
        let block = |stmts: &[crate::pom::StmtId]| -> Vec<ExprId> {
            stmts
                .iter()
                .filter_map(|s| match &self.design.stmt_node(*s).kind {
                    StmtKind::Let { init: Some(e), .. } | StmtKind::Expr { expr: e, .. } => Some(*e),
                    _ => None,
                })
                .collect()
        };
        match kind {
            ExprKind::Paren(e) | ExprKind::Unary { expr: e, .. } | ExprKind::Cast { expr: e, .. } => vec![*e],
            ExprKind::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
            ExprKind::Call { args, .. } => args.iter().map(|a| a.value).collect(),
            ExprKind::MethodCall { recv, args, .. } => std::iter::once(*recv).chain(args.iter().map(|a| a.value)).collect(),
            ExprKind::If { cond, then, else_ } => std::iter::once(*cond).chain(block(&then.stmts)).chain(*else_).collect(),
            ExprKind::Block(b) => block(&b.stmts),
            _ => Vec::new(),
        }
    }

    /// `x.known_or(d)`, `x.is_known()`, `x.high()` (§3.3.4): known values whatever `x` holds.
    /// `x.known()` is an `Option`, lowered as a pair (msl-digital T27); `c.prev()` is the
    /// carrier's previous value; an edge is an event (`events.rs`), never a value.
    pub(super) fn lower_flag_read(&mut self, scope: &mut Scope, expr: ExprId, builtin: Builtin, recv: ExprId, args: &[ExprId], base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match builtin {
            Builtin::KnownOr => {
                let value = self.lower_expr(scope, recv);
                let default = match args {
                    [d] => self.lower_expr(scope, *d),
                    _ => None,
                };
                let (value, default) = (value?, default?);
                if self.flag(default) == Flag::Flagged {
                    // The IR's `known_or` takes a known default; a default that may itself be
                    // unknown would make the result unknown again.
                    self.unsupported(scope.file, span, "a `known_or` default that may be unknown");
                    return None;
                }
                Some(self.emit(Expr::KnownOr { value, default }, base, origin))
            }
            Builtin::IsKnown => {
                let value = self.lower_expr(scope, recv)?;
                Some(self.emit(Expr::IsKnown(value), base, origin))
            }
            Builtin::High => {
                let value = self.lower_expr(scope, recv)?;
                Some(self.emit(Expr::High(value), base, origin))
            }
            Builtin::Known => {
                self.unsupported(scope.file, span, "`known()`, an `Option`,");
                None
            }
            Builtin::Prev => {
                let carrier = self.carrier_of(scope, recv)?;
                Some(self.previous(carrier, origin))
            }
            // The checker reports an event read as a `bool` first (part IV §13 rule 2).
            Builtin::Changed | Builtin::Rise | Builtin::Fall => {
                self.unsupported(scope.file, span, "an event read as a `bool`");
                None
            }
        }
    }
}
