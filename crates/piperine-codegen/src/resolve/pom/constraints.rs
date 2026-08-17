//! Constraint-margin/cover-catalog *types* (DVC-08/DVT-40) and the margin
//! **producer** (constraints-on-vams): each authored `require` of a
//! module's `constraint begin … end` block lowers to a signed,
//! scale-normalized margin expression — `m ≥ 0` ⟺ satisfied — resolved
//! against the module's own scope exactly as its analog body is (same
//! constants, params, nodes; the same `resolve_expr`).
//!
//! The lowering rules (spec P2 "Margins" AC1/AC2):
//!
//! ```text
//! a <= b              →  (b − a) / scale
//! a >= b              →  (a − b) / scale
//! l <= a && a <= u    →  min(a − l, u − a) / scale
//! ```
//!
//! `scale = 1` absent a `tol` — no implicit normalization, so the division
//! is only emitted when a `tol` is authored. The grammar has no `in`
//! operator: the interval form is the compound predicate `l <= a && a <= u`
//! by construction, and the `&&` rule — the min of the parts' margins — IS
//! the interval rule. Predicate forms with no margin (anything but `<=`,
//! `>=`, `&&` of comparisons) are loud [`LowerError`]s naming the require —
//! never a silent `0.0` margin. All margins this feature produces are
//! pointwise (`QuantityClass::Pointwise`); reduced quantities arrive with
//! the deferred `var`/`target` statements.

use piperine_lang::ast::{BinOp, Expr as PomExpr};
use piperine_lang::ast::Expr;
use piperine_lang::pom::PomRequire;

use super::LowerCtx;
use super::expr::resolve_expr;

/// A quantity's analysis class — kept as a plain local copy since
/// `piperine_lang::pom::QuantityClass` was deleted with `ConstraintBlock`
/// (MD-45); nothing constructs a non-default value today (the producer is
/// gone), but the field shape is kept stable for whichever future task
/// rebuilds the producer on VAMS attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuantityClass {
    #[default]
    Pointwise,
    Reduced,
}

/// A temporal window (DVC-20) an enclosing event block scopes a margin/cover
/// point to — `[after, after + dur)` in simulation time. Local copy of the
/// type `piperine_lang::pom::ResolvedWindow` used to be (deleted with
/// `pom/constraint.rs`, MD-45); kept because `kernel`/`device`'s constraint
/// and coverage capabilities still read it through `LoweredMargin`/
/// `LoweredCoverPoint`, even though nothing constructs a non-`None` value
/// until a future phase rebuilds the producer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedWindow {
    pub after: f64,
    pub dur: f64,
}

impl ResolvedWindow {
    /// Whether simulation time `t` falls inside `[after, after + dur)`.
    pub fn contains(&self, t: f64) -> bool {
        t >= self.after && t < self.after + self.dur
    }
}

/// A `cover` point's bin partition (D10: literal by construction) — local
/// copy of the type `piperine_lang::pom::ResolvedCoverBins` used to be
/// (deleted with `pom/constraint.rs`, MD-45), kept for the same reason as
/// [`ResolvedWindow`].
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedCoverBins {
    Range { lo: f64, step: f64, hi: f64 },
    Set(Vec<f64>),
}

/// What kind of statement a lowered margin came from — a `require` names
/// its finding by the authored label; a `target` is unlabeled by grammar
/// and is named by position (`target#<stmt index>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginKind {
    Require,
    Target,
}

/// One lowered constraint margin: the signed, scale-normalized expression
/// (`expr ≥ 0` ⟺ satisfied) with its provenance — the authored label, the
/// statement kind, the pointwise/reduced class, the analyses its helpers
/// belong to (DVC-09), and the temporal window an enclosing event block
/// scopes it to (DVC-20).
#[derive(Debug, Clone)]
pub struct LoweredMargin {
    pub label: String,
    pub kind: MarginKind,
    pub class: QuantityClass,
    pub analyses: Vec<String>,
    pub window: Option<ResolvedWindow>,
    pub expr: PomExpr,
}

/// A `cover` point lowered to its evaluable (resolved) expression (D9: a
/// **separate** lowering from [`LoweredMargin`] — coverage is not a
/// margin, no signed distance, no argmin, so it gets its own catalog
/// entry rather than a `MarginKind` variant riding the margin machinery).
#[derive(Debug, Clone)]
pub struct LoweredCoverPoint {
    pub label: String,
    pub bins: ResolvedCoverBins,
    pub analyses: Vec<String>,
    pub window: Option<ResolvedWindow>,
    pub expr: PomExpr,
}

/// A `cover cross` lowered — no expression of its own (T33's bin-mapper
/// combines the *already-mapped* bin indices of its named points
/// host-side; nothing here needs JIT compilation).
#[derive(Debug, Clone)]
pub struct LoweredCoverCross {
    pub label: String,
    pub points: Vec<String>,
    pub joint_bin_count: usize,
}

// ─── Producer ────────────────────────────────────────────────────────────────

/// The margin lowering over a module's authored `require`s
/// (constraints-on-vams): each predicate becomes its signed,
/// scale-normalized [`LoweredMargin`], names resolved against `ctx`
/// exactly as the module's analog body resolves them.
pub(crate) struct ConstraintLowering;

impl ConstraintLowering {
    /// Lower every `require` to its margin expression. A predicate form
    /// with no margin (`<`, `>`, `==`, …) records a [`super::LowerError`]
    /// naming the require — the run then fails as a whole, never a silent
    /// `0.0` margin.
    pub(crate) fn lower(requires: &[PomRequire], ctx: &mut LowerCtx) -> Vec<LoweredMargin> {
        requires
            .iter()
            .map(|req| {
                let diff = Self::margin_of_pred(&req.name, &req.pred, ctx);
                let expr = match &req.tol {
                    None => diff,
                    Some(scale) => {
                        let scale = resolve_expr(scale, ctx);
                        PomExpr::Binary {
                            span: super::expr::nospan(),
                            op: BinOp::Div,
                            lhs: Box::new(diff),
                            rhs: Box::new(scale),
                        }
                    }
                };
                LoweredMargin {
                    label: req.name.clone(),
                    kind: MarginKind::Require,
                    class: QuantityClass::Pointwise,
                    analyses: Vec::new(),
                    window: None,
                    expr,
                }
            })
            .collect()
    }

    /// A `require` predicate's margin (the diff before any `tol`
    /// normalization): `<=`/`>=` lower to the signed difference; `&&` to
    /// the min of the parts' margins (the interval form `l <= a && a <= u`
    /// thereby lowers to `min(a − l, u − a)` — the `a in [l, u]` rule).
    fn margin_of_pred(label: &str, pred: &Expr, ctx: &mut LowerCtx) -> PomExpr {
        match pred {
            Expr::Binary { op: BinOp::Le, lhs, rhs, .. } => Self::difference(rhs, lhs, ctx),
            Expr::Binary { op: BinOp::Ge, lhs, rhs, .. } => Self::difference(lhs, rhs, ctx),
            Expr::Binary { op: BinOp::LogAnd, lhs, rhs, .. } => {
                let lm = Self::margin_of_pred(label, lhs, ctx);
                let rm = Self::margin_of_pred(label, rhs, ctx);
                Self::min(lm, rm)
            }
            _ => {
                ctx.errors.push(super::LowerError {
                    module: ctx.module_name.clone(),
                    what: "require predicate (only `<=`, `>=`, `&&` lower to a margin)",
                    name: label.to_string(),
                });
                // A placeholder that never escapes: `lower_bodies` fails
                // the whole run on the recorded error above.
                PomExpr::Number { span: super::expr::nospan(), value: piperine_lang::lexer::NumberLit::Real(0.0) }
            }
        }
    }

    /// `b − a` with both sides resolved (the `<=`/`>=` margin core).
    fn difference(b: &Expr, a: &Expr, ctx: &mut LowerCtx) -> PomExpr {
        Self::sub(resolve_expr(b, ctx), resolve_expr(a, ctx))
    }

    fn sub(l: PomExpr, r: PomExpr) -> PomExpr {
        PomExpr::Binary {
            span: super::expr::nospan(),
            op: BinOp::Sub,
            lhs: Box::new(l),
            rhs: Box::new(r),
        }
    }

    /// The binary `min` math call — the `&&`/interval margin combinator.
    fn min(l: PomExpr, r: PomExpr) -> PomExpr {
        PomExpr::Call {
            span: super::expr::nospan(),
            name: "min".to_string(),
            args: vec![l, r],
        }
    }
}
