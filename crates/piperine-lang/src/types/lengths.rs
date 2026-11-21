//! The integers the checker folds from a const expression before any value exists: the length of
//! an array (§1.2.5) and the discriminant of an enum variant (§1.3.7). The elaborator's
//! `ConstEval` reads the finished type table, so it cannot decide these; this folder takes what a
//! literal, a parenthesis, an `as` cast, integer arithmetic and the name of a `const` give.

use crate::msl::lexer;
use crate::pom::{BinOp, DeclKind, ExprId, ExprKind, LitKind, Resolution};

use super::check::Checker;

/// The deepest chain of `const` names the folder follows.
const NESTING: u8 = 16;

impl Checker<'_> {
    /// The length an array type or an array repeat gives, when `expr` folds to one.
    pub(super) fn fold_length(&self, expr: ExprId) -> Option<u64> {
        u64::try_from(self.fold_integer(expr, NESTING)?).ok()
    }

    /// The value of the const expression `expr` when it is a non-negative integer the folder
    /// computes; `None` for anything else, an overflow or a division by zero included.
    pub(super) fn fold_integer(&self, expr: ExprId, nesting: u8) -> Option<u128> {
        let nesting = nesting.checked_sub(1)?;
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Int(text) if !text.ends_with("f64") => lexer::int_value(text),
                _ => None,
            },
            ExprKind::Paren(inner) | ExprKind::Cast { expr: inner, .. } => self.fold_integer(*inner, nesting),
            ExprKind::Binary { op, lhs, rhs } => {
                let (lhs, rhs) = (self.fold_integer(*lhs, nesting)?, self.fold_integer(*rhs, nesting)?);
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
                    DeclKind::Const(data) => self.fold_integer(data.value?, nesting),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }
}
