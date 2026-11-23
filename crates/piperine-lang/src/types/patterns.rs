//! One pattern against a value of a known type (§1.6): what it covers, and the names it binds
//! with their types. Literal, range, path, `Some(p)`, binding, `name @ p` and or-patterns here;
//! tuple, struct, variant and array patterns in `aggregate_patterns.rs`.
//!
//! An identifier pattern that resolves to a unit variant or a constant is a path and binds
//! nothing (§1 rule 14); one that resolves to nothing binds, at any depth (§1.6.2).

use crate::msl::Span;
use crate::pom::{BlockNode, DeclKind, ExprId, ExprKind, FileId, LitKind, Name, PatId, PatKind, Resolution, UnOp};

use super::check::Checker;
use super::cover::{Coverage, Covers};
use super::unsupported::Unsupported;
use super::Ty;

/// One name a pattern binds, with its type.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Bound {
    pub(super) name: String,
    pub(super) ty: Ty,
}

impl Bound {
    /// The name `name` bound to a value of type `ty`.
    pub(super) fn new(name: String, ty: Ty) -> Self {
        Bound { name, ty }
    }
}

/// What matching one pattern against a value covers of it and binds.
#[derive(Debug, Clone)]
pub(super) struct Matched {
    pub(super) covers: Vec<Covers>,
    pub(super) bound: Vec<Bound>,
}

impl Matched {
    /// A pattern that binds nothing and covers `covers`.
    pub(super) fn covering(covers: Covers) -> Self {
        Matched { covers: vec![covers], bound: Vec::new() }
    }

    /// A pattern covering the alternatives `covers` and binding `bound`.
    pub(super) fn new(covers: Vec<Covers>, bound: Vec<Bound>) -> Self {
        Matched { covers, bound }
    }
}

/// The value of a literal bound of a pattern.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum Number {
    Int(i128),
    Float(f64),
}

impl Checker<'_> {
    /// What pattern `pat` covers of a scrutinee of type `scrut`, its bindings typed. `None` after
    /// a diagnostic.
    pub(super) fn pattern(&mut self, file: FileId, pat: PatId, scrut: &Ty) -> Option<Matched> {
        let node = self.design.pat_node(pat).clone();
        match &node.kind {
            PatKind::Wild => Some(Matched::covering(Covers::All)),
            PatKind::Ident { name, sub, binding, res: Resolution::Unresolved, .. } => self.binder(file, node.span, name, *binding, *sub, scrut),
            PatKind::Ident { sub: None, res: res @ (Resolution::Decl(_) | Resolution::Builtin(_)), .. } => self.path_pattern(file, node.span, *res, scrut),
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                self.path_pattern(file, node.span, res, scrut)
            }
            PatKind::Lit(expr) => self.literal_pattern(file, node.span, *expr, scrut),
            PatKind::Range { lo, hi, inclusive } => self.range_pattern(file, node.span, *lo, *hi, *inclusive, scrut),
            PatKind::TupleStruct { path, pats } if matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Builtin(id)) if id.info().name == "Some") => {
                let Ty::Option(inner) = scrut else {
                    self.mismatch_text(file, node.span, &scrut.name(self.design), "an `Option`");
                    return None;
                };
                let [p] = pats.as_slice() else {
                    self.mismatch_text(file, node.span, "`Some` of one pattern", &format!("`Some` of {}", pats.len()));
                    return None;
                };
                let payload = self.pattern(file, *p, inner)?;
                Some(Matched { covers: vec![Covers::Some(payload.covers)], bound: payload.bound })
            }
            PatKind::Or(alternatives) => self.or_pattern(file, node.span, alternatives, scrut),
            PatKind::Tuple(pats) => self.tuple_pattern(file, node.span, pats, scrut),
            PatKind::TupleStruct { path, pats } => self.constructor_pattern(file, node.span, *path, pats, scrut),
            PatKind::Struct { path, fields, rest } => self.struct_pattern(file, node.span, *path, fields, *rest, scrut),
            PatKind::Array(pats) => self.array_pattern(file, node.span, pats, scrut),
            other => {
                self.unsupported(file, node.span, &format!("{} as a pattern", Unsupported::pat(other)));
                None
            }
        }
    }

    /// `pat` where §1.6.11 requires an irrefutable pattern (`what` names the place: a `let`, a
    /// `for`, a closure parameter): the pattern against a value of type `ty`, or `E0184` naming a
    /// value it does not match.
    pub(super) fn irrefutable_pattern(&mut self, file: FileId, pat: PatId, ty: &Ty, what: &str) -> Option<Matched> {
        let matched = self.pattern(file, pat, ty)?;
        let missing = Coverage::new(self.design, &self.layouts).missing(ty, &matched.covers);
        if let Some(missing) = missing {
            let span = self.design.pat_node(pat).span;
            let message = format!("{what} takes an irrefutable pattern: this one does not match `{missing}` (§1.6.11)");
            self.coded(file, "E0184", span, message, "refutable pattern");
            return None;
        }
        Some(matched)
    }

    /// `let pat = init;` and `let pat = init else { .. };` for a pattern that is not a plain name
    /// (§1.7.3, §1.7.6): without `else` the pattern is irrefutable (`E0184`); with it the pattern
    /// may be refutable and the `else` block must not complete (`E0185`).
    pub(super) fn let_pattern(&mut self, file: FileId, pat: PatId, ty: Option<crate::pom::TypeId>, init: Option<ExprId>, else_: Option<&BlockNode>) {
        let span = self.design.pat_node(pat).span;
        let Some(init) = init else {
            self.unsupported(file, span, "a `let` with a pattern and no value");
            return;
        };
        let annotated = ty.map(|t| self.lower_type(t));
        let value = match annotated {
            Some(Some(t)) => {
                self.check(init, &t);
                Some(t)
            }
            Some(None) => {
                self.infer(init);
                None
            }
            None => self.infer(init),
        };
        let Some(value) = value else { return };
        match else_ {
            None => {
                self.irrefutable_pattern(file, pat, &value, "a `let` without `else`");
            }
            Some(block) => {
                self.pattern(file, pat, &value);
                self.diverging_block(file, block);
            }
        }
    }

    /// The `else` block of a `let … else` (§1.7.6): it ends in `return`, `break`, `continue` or a
    /// `loop` without `break`, so it never completes; otherwise `E0185`. What it assigns is
    /// not assigned after the `let`.
    fn diverging_block(&mut self, file: FileId, block: &BlockNode) {
        let before = self.flow.assigned.clone();
        let found = self.statements(&block.stmts, None);
        self.flow.assigned = before;
        if found.is_some_and(|t| t != Ty::Never) {
            let message = "the `else` block of a `let … else` can complete: it must end in `return`, `break`, `continue` or a `loop` without `break` (§1.7.6)".to_string();
            self.coded(file, "E0185", block.span, message, "the block completes");
        }
    }

    /// A name or `name @ sub` (§1.6.2): it matches every value the sub-pattern does
    /// (every value without one) and binds the whole value. A name already bound in the pattern
    /// is `E0184` (§1.6.10).
    fn binder(&mut self, file: FileId, span: Span, name: &Name, binding: crate::pom::DeclId, sub: Option<PatId>, scrut: &Ty) -> Option<Matched> {
        let mut matched = match sub {
            Some(sub) => self.pattern(file, sub, scrut)?,
            None => Matched::covering(Covers::All),
        };
        if matched.bound.iter().any(|b| b.name == name.text) {
            self.coded(file, "E0184", span, format!("the name `{}` is bound twice in one pattern (§1.6.10)", name.text), "bound twice");
            return None;
        }
        self.locals.insert(binding, scrut.clone());
        matched.bound.push(Bound { name: name.text.clone(), ty: scrut.clone() });
        Some(matched)
    }

    /// `p | q`: matches when one alternative does, and every alternative binds the same names with
    /// the same types (§1.6.8), else `E0184`.
    fn or_pattern(&mut self, file: FileId, span: Span, alternatives: &[PatId], scrut: &Ty) -> Option<Matched> {
        let mut covers = Vec::new();
        let mut first: Option<Vec<Bound>> = None;
        for alternative in alternatives {
            let matched = self.pattern(file, *alternative, scrut)?;
            covers.extend(matched.covers);
            let mut names = matched.bound;
            names.sort_by(|a, b| a.name.cmp(&b.name));
            match &first {
                None => first = Some(names),
                Some(expected) if *expected != names => {
                    let message = "the alternatives of an or-pattern bind different names or types: each binds the same names with the same types (§1.6.8)".to_string();
                    self.coded(file, "E0184", span, message, "alternatives bind differently");
                    return None;
                }
                Some(_) => {}
            }
        }
        Some(Matched { covers, bound: first.unwrap_or_default() })
    }

    /// A literal pattern: a `bool`, an integer or a float matched against its own type.
    fn literal_pattern(&mut self, file: FileId, span: Span, expr: ExprId, scrut: &Ty) -> Option<Matched> {
        match scrut {
            Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::F64 => self.check(expr, scrut),
            other => {
                let found = self.infer(expr)?;
                self.mismatch(file, span, other, &found);
                return None;
            }
        }
        Some(Matched::covering(self.literal_covers(expr)))
    }

    /// `lo..=hi`, `lo..hi`, `lo..` and `..=hi` over an integer or a float (§1.6.4): the bounds are
    /// of the scrutinee's type, and the range is not empty (`E0184`). Any other scrutinee is a
    /// plain mismatch.
    fn range_pattern(&mut self, file: FileId, span: Span, lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool, scrut: &Ty) -> Option<Matched> {
        if !(scrut.is_integer() || *scrut == Ty::F64) {
            self.mismatch_text(file, span, &scrut.name(self.design), "a range pattern");
            return None;
        }
        for bound in lo.iter().chain(hi.iter()) {
            self.check(*bound, scrut);
        }
        let (low, high) = (lo.map(|e| self.literal_number(e)), hi.map(|e| self.literal_number(e)));
        if let (Some(Some(low)), Some(Some(high))) = (low, high)
            && (low > high || (!inclusive && low == high))
        {
            self.coded(file, "E0184", span, "an empty range pattern matches no value (§1.6.4)".to_string(), "empty range");
            return None;
        }
        Some(Matched::covering(Self::range_covers(scrut, low, high, inclusive)))
    }

    /// What a range pattern covers: the integers it holds, when every written bound is known;
    /// nothing the checker can list otherwise, and for a float.
    fn range_covers(scrut: &Ty, low: Option<Option<Number>>, high: Option<Option<Number>>, inclusive: bool) -> Covers {
        let (type_low, type_high) = match scrut {
            Ty::UInt(n) => (0, (1i128 << n) - 1),
            Ty::Int(n) => (-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1),
            _ => return Covers::Opaque,
        };
        let first = match low {
            None => Some(type_low),
            Some(Some(Number::Int(v))) => Some(v),
            Some(_) => None,
        };
        let last = match (high, inclusive) {
            (None, _) => Some(type_high),
            (Some(Some(Number::Int(v))), true) => Some(v),
            (Some(Some(Number::Int(v))), false) => Some(v - 1),
            (Some(_), _) => None,
        };
        match (first, last) {
            (Some(first), Some(last)) => Covers::Integers(first, last),
            _ => Covers::Opaque,
        }
    }

    /// The value of a literal bound, through parentheses and a minus; `None` for anything else (a
    /// `const` is not folded).
    fn literal_number(&self, expr: ExprId) -> Option<Number> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.literal_number(*inner),
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Int(text) => crate::msl::lexer::int_value(text).and_then(|v| i128::try_from(v).ok()).map(Number::Int),
                LitKind::Float { value, .. } => Some(Number::Float(*value)),
                _ => None,
            },
            ExprKind::Unary { op: UnOp::Neg, expr } => match self.literal_number(*expr)? {
                Number::Int(v) => Some(Number::Int(-v)),
                Number::Float(v) => Some(Number::Float(-v)),
            },
            _ => None,
        }
    }

    /// A path pattern: a unit variant of the scrutinee's enum, a constant of its type, or `None`.
    fn path_pattern(&mut self, file: FileId, span: Span, res: Resolution, scrut: &Ty) -> Option<Matched> {
        match res {
            Resolution::Builtin(id) if id.info().name == "None" => match scrut {
                Ty::Option(_) => Some(Matched::covering(Covers::Nothing)),
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
                    Some(Matched::covering(Covers::Variant(d)))
                }
                DeclKind::Const(c) => {
                    self.reach(d);
                    let found = self.lower_type(c.ty)?;
                    if found != *scrut {
                        self.mismatch(file, span, scrut, &found);
                        return None;
                    }
                    Some(Matched::covering(Covers::Opaque))
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

    /// What a literal pattern covers: a `bool`, or an integer by its value; a float covers nothing
    /// the checker can list.
    fn literal_covers(&self, expr: ExprId) -> Covers {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) if matches!(lit.kind, LitKind::Bool(_)) => match lit.kind {
                LitKind::Bool(b) => Covers::Bool(b),
                _ => Covers::Opaque,
            },
            _ => match self.literal_number(expr) {
                Some(Number::Int(v)) => Covers::Integers(v, v),
                _ => Covers::Opaque,
            },
        }
    }
}
