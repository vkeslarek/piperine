//! `match` and `if let` (DIG-03 AC 4 to 6): a `match` over an enum, a `bool`, an integer or an
//! `Option`, with path, literal, `Some(p)`, `None`, or- and wildcard patterns, arms of one type
//! and every value covered; `if let Some(x) = e`.
//!
//! An identifier pattern that resolves to a unit variant or a constant is a path and binds
//! nothing (§1 rule 14); one that resolves to nothing binds, and is allowed only inside `Some`.

use std::collections::BTreeSet;

use crate::msl::lexer;
use crate::pom::{Arm, DeclId, DeclKind, ExprId, ExprKind, FileId, LitKind, PatId, PatKind, Resolution};

use super::check::Checker;
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
    /// One integer value; `None` when it is a constant the checker does not fold.
    Int(Option<u128>),
    /// `None` of an `Option`.
    Nothing,
    /// `Some(p)`, whether `p` covers every payload.
    Some(bool),
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
    fn arms(&mut self, arms: &[Arm], expected: Option<&Ty>) -> Option<Ty> {
        let ty = match expected {
            Some(t) => t.clone(),
            None => {
                let first = arms.iter().position(|a| !self.integer_literal_waits(a.body) && !self.is_none_literal(a.body)).unwrap_or(0);
                let Some(arm) = arms.get(first) else { return Some(Ty::Unit) };
                let t = self.infer(arm.body);
                for (i, arm) in arms.iter().enumerate() {
                    if i != first {
                        match &t {
                            Some(t) => self.check(arm.body, t),
                            None => {
                                self.infer(arm.body);
                            }
                        }
                    }
                }
                return t;
            }
        };
        for arm in arms {
            self.check(arm.body, &ty);
        }
        Some(ty)
    }

    /// Whether `covered` holds every value of `scrut`.
    fn exhaustive(&self, scrut: &Ty, covered: &[Covers]) -> bool {
        if covered.contains(&Covers::All) {
            return true;
        }
        match scrut {
            Ty::Bool => covered.contains(&Covers::Bool(true)) && covered.contains(&Covers::Bool(false)),
            Ty::Option(_) => covered.contains(&Covers::Nothing) && covered.contains(&Covers::Some(true)),
            Ty::Enum(e) => {
                let variants: BTreeSet<DeclId> = self.design.decl_data(*e).children.iter().copied().filter(|v| matches!(self.design.decl_data(*v).kind, DeclKind::Variant(_))).collect();
                variants.iter().all(|v| covered.contains(&Covers::Variant(*v)))
            }
            Ty::UInt(n) | Ty::Int(n) => {
                let values: BTreeSet<u128> = covered.iter().filter_map(|c| match c {
                    Covers::Int(Some(v)) => Some(*v),
                    _ => None,
                }).collect();
                *n <= 16 && values.len() as u128 == 1u128 << n
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
                let all = self.pattern(file, *p, inner, true)?.contains(&Covers::All);
                Some(vec![Covers::Some(all)])
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
                LitKind::Int(text) => Covers::Int(lexer::int_value(text)),
                _ => Covers::Int(None),
            },
            ExprKind::Paren(inner) => self.literal_covers(*inner),
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
