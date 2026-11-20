//! `match` and `if let` (§1.6, §1.8): a `match` over an enum, a `bool`, an integer, a float or an
//! `Option`, with path, literal, range, `Some(p)`, `None`, or-, `@` and binding patterns, a guard
//! per arm, arms of one type and every value covered; `if let Some(x) = e`.
//!
//! An identifier pattern that resolves to a unit variant or a constant is a path and binds
//! nothing (§1 rule 14); one that resolves to nothing binds, at any depth (§1.6.2).

use crate::pom::{Arm, ExprId, ExprKind, PatKind, Resolution};

use super::check::Checker;
use super::cover::Coverage;
use super::flow::Assigned;
use super::Ty;

impl Checker<'_> {
    /// `match scrutinee { arms }`: its type is that of its arms, checked against `expected`
    /// when the position gives one.
    pub(super) fn match_expr(&mut self, expr: ExprId, expected: Option<&Ty>) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Match { scrutinee, arms } = &node.kind else { return None };
        let scrut = self.infer(*scrutinee);
        if let Some(t) = &scrut
            && !matches!(t, Ty::Enum(_) | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::F64 | Ty::Option(_))
        {
            let span = self.design.expr_node(*scrutinee).span;
            self.unsupported(file, span, &format!("`match` expression over `{}`", t.name(self.design)));
        }
        let mut covered = Vec::new();
        // A refused pattern is its own diagnostic: the coverage is then not judged.
        let mut patterns_typed = true;
        for arm in arms {
            let Some(t) = &scrut else { continue };
            match self.pattern(file, arm.pat, t) {
                Some(matched) => {
                    // §1.8.3: the guard sees the pattern's names; a guarded arm covers nothing (§1.8.6).
                    match arm.guard {
                        Some(guard) => self.check(guard, &Ty::Bool),
                        None => covered.extend(matched.covers),
                    }
                }
                None => patterns_typed = false,
            }
        }
        let ty = self.arms(arms, expected);
        if let Some(t) = &scrut
            && patterns_typed
            && let Some(missing) = Coverage::new(self.design).missing(t, &covered)
        {
            let what = format!("a `match` covering every value of `{}`", t.name(self.design));
            self.mismatch_text(file, node.span, &what, &format!("a `match` that misses `{missing}`"));
        }
        ty
    }

    /// The type of the arms: `expected`, else the first arm that does not wait for another's type.
    /// Each arm starts from the state after the scrutinee; the state after the `match` is where
    /// they meet.
    fn arms(&mut self, arms: &[Arm], expected: Option<&Ty>) -> Option<Ty> {
        let before = self.flow.assigned.clone();
        let mut after_arms = Vec::new();
        let ty = self.arm_types(arms, expected, &before, &mut after_arms);
        self.flow.assigned = Assigned::join(after_arms);
        ty
    }

    /// The type of each arm body, checked in order from `before`; the state each leaves goes to
    /// `after_arms`.
    fn arm_types(&mut self, arms: &[Arm], expected: Option<&Ty>, before: &Assigned, after_arms: &mut Vec<Assigned>) -> Option<Ty> {
        let ty = match expected {
            Some(t) => t.clone(),
            None => return self.inferred_arms(arms, before, after_arms),
        };
        for arm in arms {
            self.check(arm.body, &ty);
            after_arms.push(self.flow.end_branch(before));
        }
        Some(ty)
    }

    /// The type of the arms when the position gives none: the first arm that does not wait for
    /// another's type and does not diverge gives it, and the others are checked against it. All
    /// arms diverging is `!`.
    fn inferred_arms(&mut self, arms: &[Arm], before: &Assigned, after_arms: &mut Vec<Assigned>) -> Option<Ty> {
        let first = arms.iter().position(|a| !self.integer_literal_waits(a.body) && !self.is_none_literal(a.body)).unwrap_or(0);
        if arms.is_empty() {
            return Some(Ty::Unit);
        }
        let order = std::iter::once(first).chain((0..arms.len()).filter(|i| *i != first));
        // `None`: no arm has given a type yet; `Some(None)`: the one that did had a diagnostic.
        let mut typed: Option<Option<Ty>> = None;
        for i in order {
            let body = arms[i].body;
            match &typed {
                None => {
                    let t = self.infer(body);
                    if t != Some(Ty::Never) {
                        typed = Some(t);
                    }
                }
                Some(Some(t)) => self.check(body, &t.clone()),
                Some(None) => {
                    self.infer(body);
                }
            }
            after_arms.push(self.flow.end_branch(before));
        }
        typed.unwrap_or(Some(Ty::Never))
    }

    /// The condition of an `if`: a `bool`, or `let Some(x) = e` (DIG-03 AC 6) whose binding is
    /// in scope in the first block.
    pub(super) fn condition(&mut self, cond: ExprId) {
        let node = self.design.expr_node(cond).clone();
        let ExprKind::Let { pat, expr } = &node.kind else {
            self.check(cond, &Ty::Bool);
            return;
        };
        let file = self.file_of_expr(cond);
        let pat_node = self.design.pat_node(*pat).clone();
        let is_some = matches!(&pat_node.kind, PatKind::TupleStruct { path, pats } if pats.len() == 1 && matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Builtin(id)) if id.info().name == "Some"));
        match self.infer(*expr) {
            Some(t @ Ty::Option(_)) if is_some => {
                self.pattern(file, *pat, &t);
            }
            Some(Ty::Option(_)) => self.unsupported(file, pat_node.span, "`if let` with a pattern other than `Some(p)`"),
            Some(other) => {
                let span = self.design.expr_node(*expr).span;
                self.mismatch_text(file, span, "an `Option`", &other.name(self.design));
            }
            None => {}
        }
        self.exprs[cond.index()] = Some(Ty::Bool);
    }
}
