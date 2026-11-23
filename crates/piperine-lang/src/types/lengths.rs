//! The integers the checker folds from a const expression before any value exists: the length of
//! an array (§1.2.5) and the discriminant of an enum variant (§1.3.7). The elaborator's
//! `ConstEval` reads the finished type table, so it cannot decide these; this folder takes what a
//! literal, a parenthesis, an `as` cast, integer arithmetic and the name of a `const` give.

use crate::msl::lexer;
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, LitKind, Resolution};

use super::widths::FloatText;
use super::check::Checker;
use super::{Length, Ty};

/// The deepest chain of `const` names the folder follows.
const NESTING: u8 = 16;

impl Checker<'_> {
    /// `base[range]` over an array (§1.5.9): `e[a..b]`, `e[a..=b]`, `e[..b]`, `e[a..]` and `e[..]`
    /// are the array of the elements `a` to `b - 1`, of type `[T; b - a]`. A bound that is no
    /// const expression is `E0188` naming it; a range that ends beyond the array or starts after
    /// its end is `E0104` (§1.5.25).
    pub(super) fn range_index(&mut self, expr: ExprId, base: ExprId, range: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let found = self.infer(base)?;
        let Ty::Array(element, Length::Fixed(length)) = &found else {
            let base_span = self.design.expr_node(base).span;
            self.mismatch_text(file, base_span, "an array", &found.name(self.design));
            return None;
        };
        let ExprKind::Range { lo, hi, inclusive } = self.design.expr_node(range).kind.clone() else { return None };
        let low = self.range_bound(file, lo, 0)?;
        let high = match hi {
            None => u128::from(*length),
            Some(hi) => self.range_bound(file, Some(hi), 0)? + u128::from(inclusive),
        };
        if low > high || high > u128::from(*length) {
            let message = format!("the range `{low}..{high}` is outside an array of {length} elements, or starts after its end (§1.5.25)");
            self.coded(file, "E0104", span, message, "range out of the array");
            return None;
        }
        Some(Ty::array((**element).clone(), (high - low) as u64))
    }

    /// The folded bound of a range index, `absent` when it is not written; `E0188` naming a bound
    /// that is no const expression.
    fn range_bound(&mut self, file: crate::pom::FileId, bound: Option<ExprId>, absent: u128) -> Option<u128> {
        let Some(bound) = bound else { return Some(absent) };
        match self.fold_integer(bound, NESTING) {
            Some(value) => Some(value),
            None => {
                let span = self.design.expr_node(bound).span;
                let name = self.expression_name(bound);
                self.coded(file, "E0188", span, format!("the bound {name} of a range index is not a const expression (§1.5.9)"), "not a const expression");
                None
            }
        }
    }

    /// The length an array type or an array repeat gives, when `expr` folds to one.
    pub(super) fn fold_length(&self, expr: ExprId) -> Option<u64> {
        u64::try_from(self.fold_integer(expr, NESTING)?).ok()
    }

    /// The value of the const expression `expr` when it is a non-negative integer the folder
    /// computes; `None` for anything else, an overflow or a division by zero included.
    pub(super) fn fold_integer(&self, expr: ExprId, nesting: u8) -> Option<u128> {
        IntegerFolder::of(self.design).fold(expr, nesting)
    }
}

/// Folds a const expression to a non-negative integer from the POM alone (no type table).
pub(super) struct IntegerFolder<'d> {
    /// The design the expression is read from.
    pub(super) design: &'d crate::pom::Design,
    /// The value of each const parameter the expression may name (an instantiation's arguments).
    values: std::collections::BTreeMap<DeclId, u128>,
}

impl<'d> IntegerFolder<'d> {
    /// A folder that knows no const parameter.
    pub(super) fn of(design: &'d crate::pom::Design) -> Self {
        IntegerFolder { design, values: std::collections::BTreeMap::new() }
    }

    /// A folder that reads each const parameter in `values` as its value.
    pub(super) fn with_values(design: &'d crate::pom::Design, values: std::collections::BTreeMap<DeclId, u128>) -> Self {
        IntegerFolder { design, values }
    }
}

impl IntegerFolder<'_> {
    /// The value of the const expression `expr` when it is a non-negative integer, following at
    /// most `nesting` names of `const`s; `None` for anything else, an overflow or a division by
    /// zero included.
    pub(super) fn fold(&self, expr: ExprId, nesting: u8) -> Option<u128> {
        let nesting = nesting.checked_sub(1)?;
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Int(text) if !FloatText::is(text) => lexer::int_value(text),
                _ => None,
            },
            ExprKind::Paren(inner) | ExprKind::Cast { expr: inner, .. } => self.fold(*inner, nesting),
            // `{ N + 1 }`: a block of one expression is that expression (§3.8.5)
            ExprKind::Block(block) if let [only] = block.stmts.as_slice() && let crate::pom::StmtKind::Expr { expr: inner, semi: false } = &self.design.stmt_node(*only).kind => self.fold(*inner, nesting),
            ExprKind::Binary { op, lhs, rhs } => {
                let (lhs, rhs) = (self.fold(*lhs, nesting)?, self.fold(*rhs, nesting)?);
                match op {
                    BinOp::Add => lhs.checked_add(rhs),
                    BinOp::Sub => lhs.checked_sub(rhs),
                    BinOp::Mul => lhs.checked_mul(rhs),
                    BinOp::Div => lhs.checked_div(rhs),
                    BinOp::Rem => lhs.checked_rem(rhs),
                    BinOp::Shl => u32::try_from(rhs).ok().and_then(|by| lhs.checked_shl(by)).filter(|shifted| shifted >> rhs == lhs),
                    BinOp::Shr => u32::try_from(rhs).ok().map(|by| lhs.checked_shr(by).unwrap_or(0)),
                    _ => None,
                }
            }
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => match &self.design.decl_data(decl).kind {
                    DeclKind::Const(data) => self.fold(data.value?, nesting),
                    DeclKind::GenericParam(_) | DeclKind::Param(_) => self.values.get(&decl).copied(),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }
}
