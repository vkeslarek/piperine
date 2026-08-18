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

use piperine_lang::ast::{BinOp, CoverBins, Expr as PomExpr, TargetCmp};
use piperine_lang::ast::Expr;
use piperine_lang::pom::PomConstraintStmt;

use super::LowerCtx;
use super::expr::{resolve_expr, subst_expr};

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

/// The reduced-helper names — an expression containing one of these is a
/// `QuantityClass::Reduced` quantity, host-computed once per completed AC
/// sweep (D7): `ac_unity_gain_freq`/`ac_phase_margin`/`ac_gain_margin`
/// (the three reductions `piperine-api`'s `reduced.rs` implements).
const REDUCED_HELPERS: &[&str] = &["ac_unity_gain_freq", "ac_phase_margin", "ac_gain_margin"];

/// Whether `expr` contains a reduced helper call — the pointwise/reduced
/// classification rule (DVC-09's shape, re-derived on the VAMS surface):
/// reduced quantities exist once per completed analysis, so they never
/// reach the kernel.
fn is_reduced(expr: &Expr) -> bool {
    match expr {
        Expr::Call { name, args, .. } => {
            REDUCED_HELPERS.contains(&name.as_str()) || args.iter().any(is_reduced)
        }
        Expr::Unary { operand, .. } => is_reduced(operand),
        Expr::Binary { lhs, rhs, .. } => is_reduced(lhs) || is_reduced(rhs),
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            is_reduced(cond) || is_reduced(then_branch) || is_reduced(else_branch)
        }
        _ => false,
    }
}

/// The margin lowering over a module's authored constraint statements
/// (`constraint-statements`): each `require`/`target` becomes its signed,
/// scale-normalized [`LoweredMargin`], `var`s bind substitution entries,
/// and `cover`s land in the separate coverage catalog — names resolved
/// against `ctx` exactly as the module's analog body resolves them.
/// The accumulated scope an enclosing `@ …` group imposes on its
/// contained statements (DVC-19/20): the analysis set and the
/// temporal window.
#[derive(Default)]
struct ScopeCtx {
analyses: Vec<String>,
window: Option<ResolvedWindow>,
}

pub(crate) struct ConstraintLowering;

impl ConstraintLowering {
    /// Lower every margin statement (`require`/`target`) of `stmts`. A
    /// predicate form with no margin (`<`, `>`, `==`, …) records a
    /// [`super::LowerError`] naming the require — the run then fails as a
    /// whole, never a silent `0.0` margin. A `target`'s label is its
    /// statement position (`target#<index>`, the index across ALL
    /// statements, matching the original dv-core convention).
    pub(crate) fn lower(
        stmts: &[PomConstraintStmt],
        ctx: &mut LowerCtx,
    ) -> Vec<LoweredMargin> {
        Self::lower_scoped(stmts, &ScopeCtx::default(), ctx)
    }

    fn lower_scoped(
        stmts: &[PomConstraintStmt],
        scope: &ScopeCtx,
        ctx: &mut LowerCtx,
    ) -> Vec<LoweredMargin> {
        let mut bindings: std::collections::HashMap<String, PomExpr> =
            std::collections::HashMap::new();
        let mut margins = Vec::new();
        let mut index = 0usize;
        for stmt in stmts {
            let stmt_index = index;
            index += 1;
            match stmt {
                PomConstraintStmt::Var { name, expr, .. } => {
                    // Authored order: a var may read earlier vars (the
                    // substitution composes), never a later one — an
                    // unknown name survives to `resolve_expr` and fails
                    // loud at compile.
                    let bound = subst_expr(expr, &bindings);
                    bindings.insert(name.clone(), resolve_expr(&bound, ctx));
                }
                PomConstraintStmt::Require(req) => {
                    let pred = subst_expr(&req.pred, &bindings);
                    let expr = Self::margin_of_pred(&req.name, &pred, ctx);
                    let expr = Self::normalized(expr, req.tol.as_ref(), &bindings, ctx);
                    margins.push(LoweredMargin {
                        label: req.name.clone(),
                        kind: MarginKind::Require,
                        class: QuantityClass::Pointwise,
                        analyses: scope.analyses.clone(),
                        window: scope.window,
                        expr,
                    });
                }
                PomConstraintStmt::Target { expr, cmp, level, tol, .. } => {
                    // Substitute first: the class and the margin read the
                    // same var-expanded operands — `target ugbw >= 1e4`
                    // where `var ugbw = ac_unity_gain_freq(…)` is Reduced
                    // (the helper appears once the binding expands, DVC-09
                    // through the var), so it never reaches the kernel
                    // (D7). An enclosing scope's analyses ride along.
                    let expr = subst_expr(expr, &bindings);
                    let level = subst_expr(level, &bindings);
                    let tol = tol.as_ref().map(|scale| subst_expr(scale, &bindings));
                    let class = if is_reduced(&expr)
                        || is_reduced(&level)
                        || tol.as_ref().is_some_and(is_reduced)
                    {
                        QuantityClass::Reduced
                    } else {
                        QuantityClass::Pointwise
                    };
                    let expr = Self::margin_of_target(*cmp, &expr, &level, tol.as_ref(), ctx);
                    let mut analyses = scope.analyses.clone();
                    if class == QuantityClass::Reduced && !analyses.iter().any(|a| a == "ac") {
                        analyses.push("ac".to_string());
                    }
                    margins.push(LoweredMargin {
                        label: format!("target#{stmt_index}"),
                        kind: MarginKind::Target,
                        class,
                        analyses,
                        window: scope.window,
                        expr,
                    });
                }
                PomConstraintStmt::Scope { analysis, after, dur, stmts, .. } => {
                    // The window's bounds are elaboration constants — a
                    // non-const bound fails loud naming the group. `dur`
                    // absent is open-ended (`dur = ∞`).
                    let window = match after {
                        None => scope.window,
                        Some(after_expr) => {
                            let a = crate::resolve::pom_eval_const(after_expr, &|_| None)
                                .unwrap_or_else(|_| {
                                    ctx.errors.push(super::LowerError {
                                        module: ctx.module_name.clone(),
                                        what: "scope window `after` (an elaboration constant)",
                                        name: format!("@ {analysis}"),
                                    });
                                    0.0
                                });
                            let d = match dur {
                                None => f64::INFINITY,
                                Some(d_expr) => {
                                    crate::resolve::pom_eval_const(d_expr, &|_| None).unwrap_or_else(|_| {
                                        ctx.errors.push(super::LowerError {
                                            module: ctx.module_name.clone(),
                                            what: "scope window `dur` (an elaboration constant)",
                                            name: format!("@ {analysis}"),
                                        });
                                        0.0
                                    })
                                }
                            };
                            Some(ResolvedWindow { after: a, dur: d })
                        }
                    };
                    let inner = ScopeCtx {
                        analyses: {
                            let mut a = scope.analyses.clone();
                            if !a.iter().any(|x| x == analysis) {
                                a.push(analysis.clone());
                            }
                            a
                        },
                        window,
                    };
                    margins.extend(Self::lower_scoped(stmts, &inner, ctx));
                    // Every contained statement consumes a `target#<n>`
                    // index slot (numbering crosses scopes).
                    index += Self::count_statements(stmts);
                }
                PomConstraintStmt::Cover { .. } | PomConstraintStmt::CoverCross { .. } => {}
            }
        }
        margins
    }

    /// The number of statements in a scope group's tree (for
    /// `target#<n>` numbering: every statement, at any depth, consumes
    /// one index).
    fn count_statements(stmts: &[PomConstraintStmt]) -> usize {
        stmts
            .iter()
            .map(|s| match s {
                PomConstraintStmt::Scope { stmts, .. } => 1 + Self::count_statements(stmts),
                _ => 1,
            })
            .sum()
    }

    /// Lower every `cover`/`cover cross` of `stmts` (T33's shape): a
    /// point's bins const-evaluate to literal `f64` edges here (the
    /// bin-mapper never re-evaluates authored expressions); a cross names
    /// existing points and carries their joint bin count — an unknown
    /// point name fails loud.
    pub(crate) fn lower_cover(
        stmts: &[PomConstraintStmt],
        ctx: &mut LowerCtx,
    ) -> (Vec<LoweredCoverPoint>, Vec<LoweredCoverCross>) {
        let mut points = Vec::new();
        let mut crosses = Vec::new();
        for stmt in stmts {
            if let PomConstraintStmt::Scope { stmts: inner, .. } = stmt {
                let (p, c) = Self::lower_cover(inner, ctx);
                points.extend(p);
                crosses.extend(c);
                continue;
            }
            match stmt {
                PomConstraintStmt::Cover { name, expr, bins, .. } => {
                    points.push(LoweredCoverPoint {
                        label: name.clone(),
                        bins: Self::resolve_bins(name, bins, ctx),
                        analyses: Vec::new(),
                        window: None,
                        expr: resolve_expr(expr, ctx),
                    });
                }
                PomConstraintStmt::CoverCross { name, points: named, .. } => {
                    let mut joint = 1usize;
                    for p in named {
                        let Some(point) = points.iter().find(|pt| pt.label == *p) else {
                            // Loud: the run fails on this error — the
                            // placeholder count never escapes.
                            ctx.errors.push(super::LowerError {
                                module: ctx.module_name.clone(),
                                what: "cover cross point (must name a declared cover point)",
                                name: format!("{name} → {p}"),
                            });
                            continue;
                        };
                        joint *= match &point.bins {
                            ResolvedCoverBins::Range { lo, step, hi } => {
                                ((*hi - *lo) / *step).round() as usize + 1
                            }
                            ResolvedCoverBins::Set(vs) => vs.len(),
                        };
                    }
                    crosses.push(LoweredCoverCross {
                        label: name.clone(),
                        points: named.clone(),
                        joint_bin_count: joint,
                    });
                }
                _ => {}
            }
        }
        (points, crosses)
    }

    /// Const-evaluate a cover point's bins to literal edges — a non-const
    /// edge or a non-positive step is a loud error naming the point.
    fn resolve_bins(
        name: &str,
        bins: &CoverBins,
        ctx: &mut LowerCtx,
    ) -> ResolvedCoverBins {
        let eval = |e: &Expr| -> Option<f64> {
            crate::resolve::pom_eval_const(e, &|_| None).ok()
        };
        match bins {
            CoverBins::Range { lo, step, hi } => {
                match (eval(lo), eval(step), eval(hi)) {
                    (Some(lo), Some(step), Some(hi)) if step > 0.0 && hi >= lo => {
                        ResolvedCoverBins::Range { lo, step, hi }
                    }
                    _ => {
                        ctx.errors.push(super::LowerError {
                            module: ctx.module_name.clone(),
                            what: "cover bins (const [lo:step:hi] with step > 0)",
                            name: name.to_string(),
                        });
                        ResolvedCoverBins::Range { lo: 0.0, step: 1.0, hi: 0.0 }
                    }
                }
            }
            CoverBins::Set(elems) => {
                let values: Vec<f64> = elems.iter().filter_map(eval).collect();
                if values.len() != elems.len() {
                    ctx.errors.push(super::LowerError {
                        module: ctx.module_name.clone(),
                        what: "cover bins (const `in {…}` values)",
                        name: name.to_string(),
                    });
                }
                ResolvedCoverBins::Set(values)
            }
        }
    }

    /// Apply the optional `tol` normalization: division only when authored
    /// (`scale = 1` absent — no implicit normalization).
    fn normalized(
        diff: PomExpr,
        tol: Option<&Expr>,
        bindings: &std::collections::HashMap<String, PomExpr>,
        ctx: &mut LowerCtx,
    ) -> PomExpr {
        match tol {
            None => diff,
            Some(scale) => {
                let scale = resolve_expr(&subst_expr(scale, bindings), ctx);
                PomExpr::Binary {
                    span: super::expr::nospan(),
                    op: BinOp::Div,
                    lhs: Box::new(diff),
                    rhs: Box::new(scale),
                }
            }
        }
    }

    /// A `require` predicate's margin: `<=`/`>=` lower to the signed
    /// difference; `&&` to the min of the parts' margins (the interval
    /// form `l <= a && a <= u` thereby lowers to `min(a − l, u − a)` —
    /// the `a in [l, u]` rule).
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

    /// A `target`'s margin over its already-substituted operands (the
    /// caller substitutes var bindings once, so class and margin read the
    /// same tree): `expr <= level` → `level − expr`, `expr >= level` →
    /// `expr − level` (the `tol` division applies via
    /// [`Self::normalized`]'s rule — division only when authored).
    fn margin_of_target(
        cmp: TargetCmp,
        expr: &Expr,
        level: &Expr,
        tol: Option<&Expr>,
        ctx: &mut LowerCtx,
    ) -> PomExpr {
        let a = resolve_expr(expr, ctx);
        let b = resolve_expr(level, ctx);
        let diff = match cmp {
            TargetCmp::Le => Self::sub(b, a),
            TargetCmp::Ge => Self::sub(a, b),
        };
        match tol {
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
