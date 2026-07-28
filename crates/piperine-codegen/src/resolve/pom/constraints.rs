//! Constraint margin lowering (DVC-08): each `require`/`target` comparison
//! becomes a signed, scale-normalized margin expression — `m ≥ 0` ⟺
//! satisfied — resolved against the module's own scope exactly as its
//! analog body is (same constants, enum values, instance ports, optional
//! params).
//!
//! The lowering rules (spec P1 "Margins" AC1):
//!
//! ```text
//! a <= b            →  (b − a) / scale
//! a >= b            →  (a − b) / scale
//! a in [l, u]       →  min(u − a, a − l) / scale
//! ```
//!
//! `scale = 1` absent a `tol` — no implicit normalization, so the division
//! is only emitted when a `tol` is authored. The grammar has no `in`
//! operator: the interval form is the compound predicate `l <= a && a <= u`
//! by construction, and the `&&` rule — the min of the parts' margins — IS
//! the interval rule. Reduced-class statements lower like the rest (the
//! class travels with the margin); the kernel stage keeps only the
//! pointwise ones (D7).

use std::collections::HashMap;

use piperine_lang::parse::ast::{BinaryOp, ConstraintStmt, Expr as PomExpr, Literal, TargetCmp};
use piperine_lang::pom::{ConstraintBlock, QuantityClass};

use super::LowerCtx;
use super::expr::{resolve_expr, subst_expr};

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
/// statement kind, the pointwise/reduced class, and the analyses its
/// helpers belong to (DVC-09).
#[derive(Debug, Clone)]
pub struct LoweredMargin {
    pub label: String,
    pub kind: MarginKind,
    pub class: QuantityClass,
    pub analyses: Vec<String>,
    pub expr: PomExpr,
}

/// The margin lowering over one [`ConstraintBlock`]: `var` statements bind
/// into a substitution map (authored order — elaboration already proved
/// every reference resolves), `require`/`target` statements become
/// [`LoweredMargin`]s.
pub(crate) struct ConstraintLowering<'a> {
    block: &'a ConstraintBlock,
}

impl<'a> ConstraintLowering<'a> {
    /// Lower every `require`/`target` of `block` to its margin expression,
    /// resolving names against `ctx` exactly as the module's analog body
    /// does. Predicate forms with no margin (anything but `<=`, `>=`,
    /// `&&`) record a [`super::LowerError`] — never a silent `0.0` margin.
    pub(crate) fn lower(block: &'a ConstraintBlock, ctx: &mut LowerCtx) -> Vec<LoweredMargin> {
        Self { block }.lower_stmts(ctx)
    }

    fn lower_stmts(&self, ctx: &mut LowerCtx) -> Vec<LoweredMargin> {
        let mut bindings: HashMap<String, PomExpr> = HashMap::new();
        let mut margins = Vec::new();
        // Leaves are yielded depth-first in authored order (event blocks
        // descend). The positional `target#<index>` label uses the leaf
        // index — the same scheme the host catalog uses, so a target keeps
        // its label whether read from a margin entry or the catalog
        // (DVC-19: a target inside `@ dc { … }` is still the Nth leaf).
        for (leaf_index, (stmt, class)) in self.block.leaves().enumerate() {
            match stmt {
                ConstraintStmt::Var { name, value, .. } => {
                    let bound = subst_expr(value, &bindings);
                    bindings.insert(name.clone(), resolve_expr(&bound, ctx));
                }
                ConstraintStmt::Require { name, pred, .. } => {
                    let pred = subst_expr(pred, &bindings);
                    let expr = self.margin_of_pred(name, &pred, ctx);
                    margins.push(LoweredMargin {
                        label: name.clone(),
                        kind: MarginKind::Require,
                        class: class.class,
                        analyses: class.analyses.clone(),
                        expr,
                    });
                }
                ConstraintStmt::Target { expr, cmp, level, tol, .. } => {
                    let margin = self.margin_of_target(expr, *cmp, level, tol.as_ref(), &bindings, ctx);
                    margins.push(LoweredMargin {
                        label: format!("target#{leaf_index}"),
                        kind: MarginKind::Target,
                        class: class.class,
                        analyses: class.analyses.clone(),
                        expr: margin,
                    });
                }
                ConstraintStmt::Event { .. } => {}
            }
        }
        margins
    }

    /// A `require` predicate's margin (scale 1 — a `require` carries no
    /// `tol`): `<=`/`>=` lower to the signed difference; `&&` to the min of
    /// the parts' margins (the interval form `l <= a && a <= u` thereby
    /// lowers to `min(u − a, a − l)` — the `a in [l, u]` rule).
    fn margin_of_pred(&self, label: &str, pred: &PomExpr, ctx: &mut LowerCtx) -> PomExpr {
        match pred {
            PomExpr::Binary(l, BinaryOp::Le, r) => Self::difference(r, l, ctx),
            PomExpr::Binary(l, BinaryOp::Ge, r) => Self::difference(l, r, ctx),
            PomExpr::Binary(l, BinaryOp::And, r) => {
                let lm = self.margin_of_pred(label, l, ctx);
                let rm = self.margin_of_pred(label, r, ctx);
                Self::min(lm, rm)
            }
            other => {
                ctx.errors.push(super::LowerError {
                    module: ctx.module_name.clone(),
                    what: "require predicate (only `<=`, `>=`, `&&` lower to a margin)",
                    name: format!("{label} ({})", super::expr::expr_to_name(other)),
                });
                PomExpr::Literal(Literal::Real(0.0))
            }
        }
    }

    /// A `target`'s margin: `expr <= level` → `(level − expr)/scale`,
    /// `expr >= level` → `(expr − level)/scale`; the division is emitted
    /// only when a `tol` is authored (`scale = 1` otherwise — no implicit
    /// normalization).
    fn margin_of_target(
        &self,
        expr: &PomExpr,
        cmp: TargetCmp,
        level: &PomExpr,
        tol: Option<&PomExpr>,
        bindings: &HashMap<String, PomExpr>,
        ctx: &mut LowerCtx,
    ) -> PomExpr {
        let a = resolve_expr(&subst_expr(expr, bindings), ctx);
        let b = resolve_expr(&subst_expr(level, bindings), ctx);
        let diff = match cmp {
            TargetCmp::Le => Self::sub(b, a),
            TargetCmp::Ge => Self::sub(a, b),
        };
        match tol {
            None => diff,
            Some(scale) => {
                let scale = resolve_expr(&subst_expr(scale, bindings), ctx);
                PomExpr::Binary(Box::new(diff), BinaryOp::Div, Box::new(scale))
            }
        }
    }

    /// `b − a` with both sides resolved (the `<=`/`>=` margin core).
    fn difference(b: &PomExpr, a: &PomExpr, ctx: &mut LowerCtx) -> PomExpr {
        Self::sub(resolve_expr(b, ctx), resolve_expr(a, ctx))
    }

    fn sub(l: PomExpr, r: PomExpr) -> PomExpr {
        PomExpr::Binary(Box::new(l), BinaryOp::Sub, Box::new(r))
    }

    /// The binary `min` math call — the `&&`/interval margin combinator.
    fn min(l: PomExpr, r: PomExpr) -> PomExpr {
        PomExpr::Call(Box::new(PomExpr::Ident("min".to_string())), vec![l, r])
    }
}
