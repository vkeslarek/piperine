//! Ranges, iterators and the rows of Appendix H.5 to H.7 that take a closure or another sequence
//! (docs/spec Part I §1.5.11, §1.8.9, H.5 to H.7).
//!
//! A range expression is a [`Ty::Range`]; `iter()` of an array and the adaptors give a
//! [`Ty::Iterator`]. Every element is a value: a closure takes the element, never a reference.
//! The rows with a fixed signature are in `catalogue.rs`; the rows with a closure, a second
//! sequence or a condition on the element type are judged here.

use crate::refusal::forms::ValueForm;
use crate::msl::Span;
use crate::pom::{Arg, ExprId, ExprKind, FileId};

use super::check::Checker;
use super::derives::DerivedTrait;
use super::{Method, Ty};

impl Checker<'_> {
    /// The range expression `lo..hi` with some bounds missing: both bounds one integer or float
    /// type (§1.5.11); it is iterable (H.6 rule 3) when both bounds are there and the type is an
    /// integer. `..` has no type to give the elements and is outside the subset.
    pub(super) fn range_expr(&mut self, file: FileId, span: Span, lo: Option<ExprId>, hi: Option<ExprId>) -> Option<Ty> {
        let (first, second) = match (lo, hi) {
            (Some(lo), Some(hi)) => self.literal_last(lo, hi),
            (Some(only), None) | (None, Some(only)) => (only, only),
            (None, None) => {
                self.refuse(file, span, ValueForm::RangeWithoutBound);
                return None;
            }
        };
        let element = self.infer(first)?;
        if !(element.is_integer() || element == Ty::F64) {
            let span = self.state.design.expr_node(first).span;
            self.mismatch_text(file, span, "an integer or a float type", &element.name(self.state.design));
            return None;
        }
        if second != first {
            self.check(second, &element);
        }
        let iterable = lo.is_some() && hi.is_some() && element.is_integer();
        Some(Ty::Range { element: Box::new(element), iterable })
    }

    /// The element type and reversibility of `ty` read as an iterator (H.6 rule 3, H.7 rule 1): an
    /// iterator, or a range over an integer type with both bounds. `None` for anything else.
    fn sequence_of(ty: &Ty) -> Option<(&Ty, bool)> {
        match ty {
            Ty::Iterator { element, reversible } => Some((element, *reversible)),
            Ty::Range { element, iterable: true } => Some((element, true)),
            _ => None,
        }
    }

    /// The element type a `for` binds over `iter`: an array gives its elements by value, a range
    /// and an iterator their elements (§1.8.9). Anything else is `E0180`.
    pub(super) fn iteration_element(&mut self, iter: ExprId) -> Option<Ty> {
        let found = self.infer(iter)?;
        match &found {
            Ty::Array(element, _) => Some((**element).clone()),
            other => match Self::sequence_of(other) {
                Some((element, _)) => Some(element.clone()),
                None => {
                    let file = self.file_of_expr(iter);
                    let span = self.state.design.expr_node(iter).span;
                    self.mismatch_text(file, span, "an array, an integer range or an iterator", &found.name(self.state.design));
                    None
                }
            },
        }
    }

    /// A row of H.5 to H.7 that takes a closure, a value, another sequence, or has a condition on
    /// the element type: `contains` of an array, `map`, `filter`, `fold`, `sum`, `zip`, `any`, `all`.
    pub(super) fn sequence_row(&mut self, expr: ExprId, row: Method, recv: &Ty, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let result = match (row, recv) {
            (Method::ArrayContains, Ty::Array(element, _)) => self.array_contains(expr, element, args)?,
            (_, receiver) => {
                let (element, reversible) = Self::sequence_of(receiver)?;
                let (element, reversible) = (element.clone(), reversible);
                self.iterator_row(expr, row, &element, reversible, args, (file, span))?
            }
        };
        self.state.result.methods.insert(expr, row);
        Some(result)
    }

    /// `a.contains(x)` (H.5): `x` has the element type, which must be `PartialEq`.
    fn array_contains(&mut self, expr: ExprId, element: &Ty, args: &[Arg]) -> Option<Ty> {
        self.apply(expr, std::slice::from_ref(element), args)?;
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        self.require_trait(file, span, element, DerivedTrait::PartialEq);
        Some(Ty::Bool)
    }

    /// The closure-taking rows of H.7 over an iterator of `element`.
    fn iterator_row(&mut self, expr: ExprId, row: Method, element: &Ty, reversible: bool, args: &[Arg], at: (FileId, Span)) -> Option<Ty> {
        let (file, span) = at;
        let iterator = |item: Ty, reversible: bool| Ty::Iterator { element: Box::new(item), reversible };
        match row {
            Method::IteratorSum => {
                self.apply(expr, &[], args)?;
                if !(element.is_integer() || *element == Ty::F64) {
                    self.mismatch_text(file, span, "an integer or a float element type", &element.name(self.state.design));
                    return None;
                }
                Some(element.clone())
            }
            Method::IteratorMap => {
                let body = self.closure_argument(expr, args, std::slice::from_ref(element), None)?;
                Some(iterator(body, reversible))
            }
            Method::IteratorFilter => {
                self.closure_argument(expr, args, std::slice::from_ref(element), Some(&Ty::Bool))?;
                Some(iterator(element.clone(), false))
            }
            Method::IteratorAny | Method::IteratorAll => {
                self.closure_argument(expr, args, std::slice::from_ref(element), Some(&Ty::Bool))?;
                Some(Ty::Bool)
            }
            Method::IteratorFold => self.fold(expr, element, args),
            Method::IteratorZip => self.zip(expr, element, reversible, args),
            _ => None,
        }
    }

    /// `it.fold(init, f)` (H.7): `f` takes the accumulator and the element and gives the
    /// accumulator. An integer literal `init` over an integer element has the element's type.
    fn fold(&mut self, expr: ExprId, element: &Ty, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let [init, function] = args else {
            self.arity_mismatch(file, span, 2, args);
            return None;
        };
        let accumulator = if element.is_integer() && self.integer_literal_waits(init.value) {
            self.check(init.value, element);
            element.clone()
        } else {
            self.infer(init.value)?
        };
        self.closure_of(expr, function, &[accumulator.clone(), element.clone()], Some(&accumulator))?;
        Some(accumulator)
    }

    /// `it.zip(other)` (H.7): `other` is an iterator; the pairs end when either does.
    fn zip(&mut self, expr: ExprId, element: &Ty, reversible: bool, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let [other] = args else {
            self.arity_mismatch(file, span, 1, args);
            return None;
        };
        let found = self.infer(other.value)?;
        let Some((second, second_reversible)) = Self::sequence_of(&found) else {
            let span = self.state.design.expr_node(other.value).span;
            self.mismatch_text(file, span, "an iterator", &found.name(self.state.design));
            return None;
        };
        Some(Ty::Iterator { element: Box::new(Ty::Tuple(vec![element.clone(), second.clone()])), reversible: reversible && second_reversible })
    }

    /// The one closure argument of a row: parameters `params`, body checked against `result` when
    /// given. Gives the body's type.
    fn closure_argument(&mut self, expr: ExprId, args: &[Arg], params: &[Ty], result: Option<&Ty>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let [function] = args else {
            self.arity_mismatch(file, span, 1, args);
            return None;
        };
        self.closure_of(expr, function, params, result)
    }

    /// `argument` as a closure with parameters `params`; a value that is not a closure is outside
    /// the subset (a function pointer argument is the closure's own form).
    fn closure_of(&mut self, expr: ExprId, argument: &Arg, params: &[Ty], result: Option<&Ty>) -> Option<Ty> {
        if !matches!(self.state.design.expr_node(argument.value).kind, ExprKind::Closure { .. }) {
            let file = self.file_of_expr(expr);
            self.infer(argument.value);
            self.refuse(file, self.state.design.expr_node(argument.value).span, ValueForm::IteratorRowWithArgumentThatIsNot);
            return None;
        }
        self.closure_typed(argument.value, params, result)
    }
}
