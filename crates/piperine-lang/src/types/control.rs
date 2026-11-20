//! `match` and `if let` (DIG-03 AC 4 to 6): a `match` over an enum, a `bool`, an integer or an
//! `Option`, with path, literal, `Some(p)`, `None`, or- and wildcard patterns, arms of one type
//! and every value covered; `if let Some(x) = e`.
//!
//! An identifier pattern that resolves to a unit variant or a constant is a path and binds
//! nothing (§1 rule 14); one that resolves to nothing binds, and is allowed only inside `Some`.

use std::collections::BTreeSet;

use crate::msl::lexer;
use crate::pom::{Arm, DeclId, DeclKind, ExprId, ExprKind, FileId, LitKind, PatId, PatKind, Resolution, UnOp};

use super::check::Checker;
use super::flow::Assigned;
use super::unsupported::Unsupported;
use super::Ty;

/// What one alternative of a pattern covers of its scrutinee.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Covers {
    /// Every value: `_`.
    All,
    /// One variant of an enum.
    Variant(DeclId),
    Bool(bool),
    /// One integer value, negative ones included (`-2`); `None` when it is a constant the
    /// checker does not fold.
    Int(Option<i128>),
    /// `None` of an `Option`.
    Nothing,
    /// `Some(p)`, with what `p` covers of the payload.
    Some(Vec<Covers>),
}

impl Checker<'_> {
    /// `match scrutinee { arms }`: its type is that of its arms, checked against `expected`
    /// when the position gives one.
    pub(super) fn match_expr(&mut self, expr: ExprId, expected: Option<&Ty>) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Match { scrutinee, arms } = &node.kind else { return None };
        let scrut = self.infer(*scrutinee);
        if let Some(t) = &scrut
            && !matches!(t, Ty::Enum(_) | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::Option(_))
        {
            let span = self.design.expr_node(*scrutinee).span;
            self.unsupported(file, span, &format!("`match` expression over `{}`", t.name(self.design)));
        }
        let mut covered = Vec::new();
        // A refused pattern is its own diagnostic: the coverage is then not judged.
        let mut patterns_typed = true;
        for arm in arms {
            if let Some(guard) = arm.guard {
                let span = self.design.expr_node(guard).span;
                self.unsupported(file, span, "a `match` arm guard");
            }
            if let Some(t) = &scrut {
                match self.pattern(file, arm.pat, t, false) {
                    Some(c) => covered.extend(c),
                    None => patterns_typed = false,
                }
            }
        }
        let ty = self.arms(arms, expected);
        if let Some(t) = &scrut
            && patterns_typed
            && !self.exhaustive(t, &covered)
        {
            let what = format!("a `match` covering every value of `{}`", t.name(self.design));
            self.mismatch_text(file, node.span, &what, "a `match` that misses some");
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

    /// Whether `covered` holds every value of `scrut`.
    fn exhaustive(&self, scrut: &Ty, covered: &[Covers]) -> bool {
        if covered.contains(&Covers::All) {
            return true;
        }
        match scrut {
            Ty::Bool => covered.contains(&Covers::Bool(true)) && covered.contains(&Covers::Bool(false)),
            // `None`, and the payloads of every `Some(p)` together covering the payload's type:
            // `Some(true)` and `Some(false)` cover a `bool` as `Some(_)` does
            Ty::Option(inner) => {
                let payloads: Vec<Covers> = covered.iter().filter_map(|c| match c {
                    Covers::Some(p) => Some(p.clone()),
                    _ => None,
                }).flatten().collect();
                covered.contains(&Covers::Nothing) && self.exhaustive(inner, &payloads)
            }
            Ty::Enum(e) => {
                let variants: BTreeSet<DeclId> = self.design.decl_data(*e).children.iter().copied().filter(|v| matches!(self.design.decl_data(*v).kind, DeclKind::Variant(_))).collect();
                variants.iter().all(|v| covered.contains(&Covers::Variant(*v)))
            }
            // every value of the type named by a literal: `0..2^n` unsigned, `-2^(n-1)..2^(n-1)`
            // signed, the negative literals of a signed type among them
            Ty::UInt(n) | Ty::Int(n) => {
                let (lo, hi) = match scrut {
                    Ty::Int(_) => (-(1i128 << (n - 1)), 1i128 << (n - 1)),
                    _ => (0, 1i128 << n),
                };
                let values: BTreeSet<i128> = covered.iter().filter_map(|c| match c {
                    Covers::Int(Some(v)) if (lo..hi).contains(v) => Some(*v),
                    _ => None,
                }).collect();
                *n <= 16 && values.len() as i128 == hi - lo
            }
            _ => false,
        }
    }

    /// What pattern `pat` covers of a scrutinee of type `scrut`, its bindings typed. `None` after
    /// a diagnostic. `inside_some` allows a binding (the payload of `Some(x)`).
    fn pattern(&mut self, file: FileId, pat: PatId, scrut: &Ty, inside_some: bool) -> Option<Vec<Covers>> {
        let node = self.design.pat_node(pat).clone();
        match &node.kind {
            PatKind::Wild => Some(vec![Covers::All]),
            PatKind::Ident { by_ref: false, mutable: false, sub: None, binding, res: Resolution::Unresolved, .. } if inside_some => {
                self.locals.insert(*binding, scrut.clone());
                Some(vec![Covers::All])
            }
            PatKind::Ident { by_ref: false, mutable: false, sub: None, res: res @ (Resolution::Decl(_) | Resolution::Builtin(_)), .. } => self.path_pattern(file, node.span, *res, scrut),
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                self.path_pattern(file, node.span, res, scrut)
            }
            PatKind::Lit(expr) => {
                match scrut {
                    Ty::Bool | Ty::UInt(_) | Ty::Int(_) => self.check(*expr, scrut),
                    other => {
                        let found = self.infer(*expr)?;
                        self.mismatch(file, node.span, other, &found);
                        return None;
                    }
                }
                Some(vec![self.literal_covers(*expr)])
            }
            PatKind::TupleStruct { path, pats } if matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Builtin(id)) if id.info().name == "Some") => {
                let Ty::Option(inner) = scrut else {
                    self.mismatch_text(file, node.span, &scrut.name(self.design), "an `Option`");
                    return None;
                };
                let [p] = pats.as_slice() else {
                    self.mismatch_text(file, node.span, "`Some` of one pattern", &format!("`Some` of {}", pats.len()));
                    return None;
                };
                let payload = self.pattern(file, *p, inner, true)?;
                Some(vec![Covers::Some(payload)])
            }
            PatKind::Or(alternatives) => {
                let mut covers = Vec::new();
                for alt in alternatives {
                    if matches!(self.design.pat_node(*alt).kind, PatKind::Ident { res: Resolution::Unresolved, .. }) {
                        let span = self.design.pat_node(*alt).span;
                        self.unsupported(file, span, "a binding in an or-pattern");
                        return None;
                    }
                    covers.extend(self.pattern(file, *alt, scrut, false)?);
                }
                Some(covers)
            }
            other => {
                self.unsupported(file, node.span, &format!("{} in a `match` arm", Unsupported::pat(other)));
                None
            }
        }
    }

    /// A path pattern: a unit variant of the scrutinee's enum, a constant of its type, or `None`.
    fn path_pattern(&mut self, file: FileId, span: crate::msl::Span, res: Resolution, scrut: &Ty) -> Option<Vec<Covers>> {
        match res {
            Resolution::Builtin(id) if id.info().name == "None" => match scrut {
                Ty::Option(_) => Some(vec![Covers::Nothing]),
                other => {
                    self.mismatch_text(file, span, &other.name(self.design), "`None`");
                    None
                }
            },
            Resolution::Decl(d) => match self.design.decl_data(d).kind.clone() {
                DeclKind::Variant(_) => {
                    let parent = self.design.decl_data(d).parent?;
                    let found = self.enum_type(parent, file, span)?;
                    if found != *scrut {
                        self.mismatch(file, span, scrut, &found);
                        return None;
                    }
                    Some(vec![Covers::Variant(d)])
                }
                DeclKind::Const(c) => {
                    self.reach(d);
                    let found = self.lower_type(c.ty)?;
                    if found != *scrut {
                        self.mismatch(file, span, scrut, &found);
                        return None;
                    }
                    Some(vec![Covers::Int(None)])
                }
                other => {
                    self.unsupported(file, span, &format!("{} as a pattern", Unsupported::item(&other)));
                    None
                }
            },
            _ => {
                self.unsupported(file, span, "this name as a pattern");
                None
            }
        }
    }

    /// What a literal pattern covers: a `bool`, or an integer by its value (a negated literal
    /// by none, which only a wildcard completes).
    fn literal_covers(&self, expr: ExprId) -> Covers {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Bool(b) => Covers::Bool(*b),
                LitKind::Int(text) => Covers::Int(lexer::int_value(text).and_then(|v| i128::try_from(v).ok())),
                _ => Covers::Int(None),
            },
            ExprKind::Paren(inner) => self.literal_covers(*inner),
            // `-2`: the pattern of a negative literal
            ExprKind::Unary { op: UnOp::Neg, expr } => match self.literal_covers(*expr) {
                Covers::Int(Some(v)) => Covers::Int(Some(-v)),
                _ => Covers::Int(None),
            },
            _ => Covers::Int(None),
        }
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
                self.pattern(file, *pat, &t, false);
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
