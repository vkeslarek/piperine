//! A bare integer literal bound by a `let` takes the type its later uses fix (§1.4.4): in
//! `let mut total = 0; total += k;` with `k: u32`, `total` is a `u32`. The binding starts with
//! the default `i32` (§1.4.2) and stays *open* until a use decides it: a use that requires an
//! integer type fixes it to that type, any read that requires none closes it at `i32`.

use crate::pom::{DeclId, ExprId, ExprKind, Resolution};
use crate::msl::lexer;

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// Opens `binding` when its initializer is a bare integer literal that fits `i32`, so a later
    /// use may still give it another integer type.
    pub(super) fn wait_for_use(&mut self, binding: DeclId, init: ExprId) {
        let fits = self.integer_literal_waits(init) && self.integer_literal_value(init).is_some_and(|v| v <= i32::MAX as u128);
        if fits {
            self.open_literals.insert(binding, init);
        }
    }

    /// The value of the bare integer literal `expr`, through parentheses.
    fn integer_literal_value(&self, expr: ExprId) -> Option<u128> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.integer_literal_value(*inner),
            ExprKind::Lit(lit) => match &lit.kind {
                crate::pom::LitKind::Int(text) => lexer::int_value(text),
                _ => None,
            },
            _ => None,
        }
    }

    /// The open local `expr` names, through parentheses.
    pub(super) fn open_literal_local(&self, expr: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.open_literal_local(*inner),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if self.open_literals.contains_key(&decl) => Some(decl),
                _ => None,
            },
            _ => None,
        }
    }

    /// A use of an open local where `expected` is required: an integer type fixes the local and
    /// its literal to it (the literal is range-checked against the new type); any other type
    /// leaves the local open for the plain read, which closes it and reports the mismatch.
    /// Whether `expr` was such a use.
    pub(super) fn fix_open_literal(&mut self, expr: ExprId, expected: &Ty) -> bool {
        let Some(local) = self.open_literal_local(expr) else { return false };
        if !expected.is_integer() {
            return false;
        }
        let Some(literal) = self.open_literals.remove(&local) else { return false };
        if *expected != Ty::Int(32) {
            let file = self.file_of_expr(literal);
            let span = self.design.expr_node(literal).span;
            if let Some(text) = self.integer_literal_text(literal) {
                self.integer_literal(file, span, &text, Some(expected), false);
            }
            self.exprs[literal.index()] = Some(expected.clone());
            self.locals.insert(local, expected.clone());
        }
        true
    }
}
