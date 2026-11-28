//! What a definition defers to each instantiation (Q1 (c), part I §3 rule 9, §3.8.9, GN-07): the
//! `where` of a generic function, an array length built from const parameters meeting another
//! length, and a const expression over const parameters. The checker accepts a generic length
//! wherever a length is wanted and records the obligation under the item; the elaborator
//! discharges it when it instantiates the item (T33), naming the item and its arguments in the
//! error. A call substitutes the arguments it infers and folds the lengths they decide.

use std::collections::BTreeMap;

use crate::msl::Span;
use crate::pom::{DeclId, ExprId, FileId};

use super::check::Checker;
use super::{ConstTerm, GenericValue, Length, Substitution, Ty};

/// One check a generic item owes at each instantiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Obligation {
    /// The `where` of a generic function: false for an instantiation is `E0127`.
    Where {
        /// The function.
        item: DeclId,
        /// The condition, a `bool` const expression over its const parameters.
        condition: ExprId,
    },
    /// Two array lengths the definition accepted because one is built from const parameters; they
    /// are compared by value for the instantiation.
    SameLength {
        /// Where the two lengths met.
        file: FileId,
        /// The span where they met.
        span: Span,
        /// The length that was wanted.
        expected: Length,
        /// The length that was found.
        found: Length,
    },
    /// A const expression over const parameters (an array length `N + 1`), which must fold for the
    /// instantiation.
    Constant {
        /// The expression.
        expr: ExprId,
        /// Where the expression is written.
        file: FileId,
        /// The expression's span.
        span: Span,
    },
}

impl Checker<'_> {
    /// Records `obligation` under the generic function being checked, when there is one.
    pub(super) fn defer(&mut self, obligation: Obligation) {
        if let Some(item) = self.state.generics.generic_item {
            let list = self.state.result.obligations.entry(item).or_default();
            if !list.contains(&obligation) {
                list.push(obligation);
            }
        }
    }

    /// Whether a value of type `found` may stand where `expected` is wanted ([`Ty::fits`]); each pair
    /// of lengths the definition could only accept, not decide, is recorded as an obligation at `at`.
    pub(super) fn compatible(&mut self, at: ExprId, found: &Ty, expected: &Ty) -> bool {
        let (file, span) = (self.file_of_expr(at), self.state.design.expr_node(at).span);
        self.compatible_at(file, span, found, expected)
    }

    /// [`Checker::compatible`] where the two meet at `span` of `file`, which no expression is.
    pub(super) fn compatible_at(&mut self, file: FileId, span: Span, found: &Ty, expected: &Ty) -> bool {
        if !found.fits(expected) {
            return false;
        }
        for (found, expected) in found.deferred_lengths(expected) {
            self.defer(Obligation::SameLength { file, span, expected, found });
        }
        true
    }

    /// Records the const expression `expr` over const parameters under the generic function being
    /// checked: it must fold for each instantiation.
    pub(super) fn defer_constant(&mut self, expr: ExprId) {
        let (file, span) = (self.file_of_expr(expr), self.state.design.expr_node(expr).span);
        self.defer(Obligation::Constant { expr, file, span });
    }

    /// `ty` with the arguments of `substitution` replaced, and each array length that is an
    /// expression over const parameters folded now that their values are known.
    pub(super) fn substitute(&self, substitution: &Substitution, ty: &Ty) -> Ty {
        let values: BTreeMap<DeclId, u128> = substitution
            .entries()
            .filter_map(|(parameter, value)| match value {
                GenericValue::Const(ConstTerm::Value(n)) => u128::try_from(*n).ok().map(|n| (parameter, n)),
                _ => None,
            })
            .collect();
        self.fold_lengths(&substitution.apply(ty), &values)
    }

    /// Folds the lengths of `ty` that are expressions, with the const parameters in `values`.
    fn fold_lengths(&self, ty: &Ty, values: &BTreeMap<DeclId, u128>) -> Ty {
        match ty {
            Ty::Array(element, length) => {
                let folded = match length {
                    Length::Generic(ConstTerm::Expr(expr)) => super::value::lengths::IntegerFolder::with_values(self.state.design, values.clone()).fold(*expr, 16).and_then(|n| u64::try_from(n).ok()).map_or_else(|| length.clone(), Length::Fixed),
                    other => other.clone(),
                };
                Ty::Array(Box::new(self.fold_lengths(element, values)), folded)
            }
            Ty::Option(inner) => Ty::Option(Box::new(self.fold_lengths(inner, values))),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|item| self.fold_lengths(item, values)).collect()),
            Ty::Fn(params, ret) => Ty::Fn(params.iter().map(|param| self.fold_lengths(param, values)).collect(), Box::new(self.fold_lengths(ret, values))),
            other => other.clone(),
        }
    }
}
