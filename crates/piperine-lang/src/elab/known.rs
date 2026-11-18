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
use piperine_ir::Constant;

use super::consts::Bindings;
use crate::pom::{BinOp, DeclKind, ExprId, ExprKind};
use crate::types::{Builtin, Callee, Method, Ty};

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
    /// lowered as a bitwise operator: it can panic or emit a record, and the bitwise form runs it
    /// whatever the left operand gives. It can when it holds, written in it, in a `match` arm or
    /// in the body of a function it calls (followed transitively): an integer `/` or `%`, a shift
    /// by a signed amount that does not fold to one `>= 0`, an integer `clamp` whose bounds do
    /// not fold to `lo <= hi` (`rt::int` panics on each), or a call of a function that emits.
    /// `None` when it can run unconditionally.
    pub(super) fn partial(&mut self, expr: ExprId) -> Option<&'static str> {
        let mut stack = vec![expr];
        let mut entered = std::collections::BTreeSet::new();
        while let Some(e) = stack.pop() {
            let kind = self.design.expr_node(e).kind.clone();
            let integer = |x: ExprId| self.types.ty(x).is_some_and(|t| t.is_integer());
            match &kind {
                ExprKind::Binary { op: BinOp::Div | BinOp::Rem, .. } if integer(e) => return Some("an integer `/` or `%`"),
                ExprKind::Binary { op: BinOp::Shl | BinOp::Shr, rhs, .. } if matches!(self.types.ty(*rhs), Some(Ty::Int(_))) => {
                    if !matches!(self.consts.value(*rhs, &Bindings::new()), Ok(Constant::Int(n)) if n >= 0) {
                        return Some("a shift by a signed amount");
                    }
                }
                ExprKind::MethodCall { args, .. } if self.types.method(e) == Some(Method::Clamp) && integer(e) => {
                    let bounds = match args.as_slice() {
                        [lo, hi] => (self.consts.value(lo.value, &Bindings::new()), self.consts.value(hi.value, &Bindings::new())),
                        _ => return Some("an integer `clamp`"),
                    };
                    if !matches!(bounds, (Ok(Constant::Int(lo)), Ok(Constant::Int(hi))) if lo <= hi) {
                        return Some("an integer `clamp`");
                    }
                }
                ExprKind::Call { .. } => {
                    if let Some(Callee::Function(f)) = self.types.callee(e) {
                        if self.types.emits(f).is_some() {
                            return Some("a call of a function that emits a record");
                        }
                        // the callee's body runs too: walk it once
                        if entered.insert(f)
                            && let DeclKind::Fn(function) = &self.design.decl_data(f).kind
                            && let Some(body) = &function.body
                        {
                            stack.extend(crate::pom::block_exprs(self.design, body));
                        }
                    }
                }
                _ => {}
            }
            // every sub-expression, `match` scrutinees, guards and arms included
            stack.extend(crate::pom::expr_children(&kind, self.design));
        }
        None
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
            // Typed by the checker (T2); its lowering to a level pair is T27.
            Builtin::Level => {
                self.unsupported(scope.file, span, "method `level`");
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
