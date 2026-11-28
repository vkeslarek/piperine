//! Sequences in a const expression: ranges and arrays read as iterators, and the adapters and reductions of Appendix H.7.

use piperine_ir::{Constant, Real};
use super::control::{Argument, FnValue};
use super::{CompositeKind, Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm, StructureForm, ValueForm};
use crate::pom::{BinOp, ExprId};
use crate::types::{Method, Ty};

impl ConstEval<'_, '_> {
    /// The elements of the iterator, array or range `value` is, in order. A range without both
    /// bounds, or over floats, is not an iterator (H.6 rule 3).
    pub(super) fn elements_of(&self, expr: ExprId, value: Constant) -> Result<Vec<Constant>, ConstFault> {
        let (kind, parts) = self.parts_of(expr, value, "an iteration over a value")?;
        match kind {
            CompositeKind::Array => Ok(parts),
            CompositeKind::Range { low: true, high: true, inclusive } => match (parts[0], parts[1]) {
                (Constant::Int(first), Constant::Int(last)) => {
                    let end = if inclusive { last + 1 } else { last };
                    if end.saturating_sub(first) > (1 << 24) {
                        return Err(ConstFault::Panic { expr, what: "an iteration over more than 2^24 values (§1 rule 11)".to_string() });
                    }
                    Ok((first..end).map(|v| Constant::Int(v)).collect())
                }
                _ => Err(ConstFault::refused(expr, ConstantForm::RangeWhoseBoundsAreNotIntegers)),
            },
            _ => Err(ConstFault::refused(expr, ConstantForm::IterationOverValueThatIsNotArray)),
        }
    }

    /// A row of Appendix H.5 to H.7 over an array, a range or an iterator: `Some` result, `None`
    /// when `method` is not one of those rows.
    pub(super) fn sequence_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Option<Result<Constant, ConstFault>> {
        let is_row = matches!(
            method,
            Method::ArrayLen | Method::ArrayIsEmpty | Method::ArrayIter | Method::ArrayContains | Method::ArrayFirst | Method::ArrayLast | Method::ArrayGet | Method::RangeContains | Method::RangeIsEmpty | Method::IteratorMap | Method::IteratorFilter | Method::IteratorFold | Method::IteratorSum | Method::IteratorCount | Method::IteratorZip | Method::IteratorEnumerate | Method::IteratorRev | Method::IteratorAny | Method::IteratorAll
        );
        is_row.then(|| self.run_row(expr, method, recv, args, bindings))
    }

    /// The closure or function an argument of a row is.
    pub(super) fn closure_argument(&mut self, expr: ExprId, arg: ExprId) -> Result<FnValue, ConstFault> {
        self.fn_value_of(arg).ok_or(ConstFault::refused(expr, ConstantForm::ArgumentThatIsNotClosureOrFunction))
    }

    /// `function` called on `values`.
    pub(super) fn call_with(&mut self, expr: ExprId, function: &FnValue, values: Vec<Constant>, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let arguments = values.into_iter().map(|value| Argument { value: Some(value), function: None }).collect();
        self.apply_value(expr, function.clone(), arguments, bindings)
    }

    /// `function` called on `values`, its result a `bool`.
    pub(super) fn test_with(&mut self, expr: ExprId, function: &FnValue, values: Vec<Constant>, bindings: &Bindings) -> Result<bool, ConstFault> {
        match self.call_with(expr, function, values, bindings)? {
            Constant::Bool(holds) => Ok(holds),
            _ => Err(ConstFault::refused(expr, ConstantForm::ClosureThatDoesNotGiveBool)),
        }
    }

    /// The `Some`/`None` of an optional element.
    pub(super) fn optional(&mut self, element: Option<Constant>) -> Constant {
        self.make(CompositeKind::Option, element.into_iter().collect())
    }

    /// One row, its receiver and arguments evaluated here.
    pub(super) fn run_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let receiver = self.fold(recv, bindings)?;
        let known = |value: bool| Ok(Constant::Bool(value));
        let count = |value: usize| Ok(Constant::Int(value as i128));
        match method {
            Method::ArrayLen => count(self.elements_of(expr, receiver)?.len()),
            Method::ArrayIsEmpty => known(self.elements_of(expr, receiver)?.is_empty()),
            Method::ArrayIter => {
                let elements = self.elements_of(expr, receiver)?;
                Ok(self.make(CompositeKind::Array, elements))
            }
            Method::ArrayContains => {
                let sought = self.fold(args[0], bindings)?;
                for element in self.elements_of(expr, receiver)? {
                    if self.equal(element, sought) {
                        return known(true);
                    }
                }
                known(false)
            }
            Method::ArrayFirst => {
                let elements = self.elements_of(expr, receiver)?;
                Ok(self.optional(elements.first().copied()))
            }
            Method::ArrayLast => {
                let elements = self.elements_of(expr, receiver)?;
                Ok(self.optional(elements.last().copied()))
            }
            Method::ArrayGet => {
                let elements = self.elements_of(expr, receiver)?;
                match self.fold(args[0], bindings)? {
                    Constant::Int(at) => Ok(self.optional(usize::try_from(at).ok().and_then(|at| elements.get(at)).copied())),
                    _ => Err(ConstFault::refused(expr, StructureForm::IndexThatIsNotInteger)),
                }
            }
            Method::RangeContains => self.range_contains(expr, receiver, args[0], bindings),
            Method::RangeIsEmpty => {
                let (kind, parts) = self.parts_of(expr, receiver, "`is_empty` of a value")?;
                match (kind, parts.as_slice()) {
                    (CompositeKind::Range { low: true, high: true, inclusive }, [lo, hi]) => match self.ordering(*lo, *hi) {
                        Some(order) => known(!(if inclusive { order.is_le() } else { order.is_lt() })),
                        None => known(true),
                    },
                    _ => Err(ConstFault::refused(expr, ValueForm::IsEmptyOfRangeWithMissingBound)),
                }
            }
            Method::IteratorMap | Method::IteratorFilter | Method::IteratorAny | Method::IteratorAll => {
                let function = self.closure_argument(expr, args[0])?;
                let elements = self.elements_of(expr, receiver)?;
                self.adapt(expr, method, &function, elements, bindings)
            }
            Method::IteratorFold => {
                let mut accumulator = self.fold(args[0], bindings)?;
                let function = self.closure_argument(expr, args[1])?;
                for element in self.elements_of(expr, receiver)? {
                    accumulator = self.call_with(expr, &function, vec![accumulator, element], bindings)?;
                }
                Ok(accumulator)
            }
            Method::IteratorSum => self.sum(expr, receiver),
            Method::IteratorCount => count(self.elements_of(expr, receiver)?.len()),
            Method::IteratorZip => {
                let other = self.fold(args[0], bindings)?;
                let (first, second) = (self.elements_of(expr, receiver)?, self.elements_of(expr, other)?);
                let pairs = first.into_iter().zip(second).map(|(a, b)| self.make(CompositeKind::Tuple, vec![a, b])).collect();
                Ok(self.make(CompositeKind::Array, pairs))
            }
            Method::IteratorEnumerate => {
                let elements = self.elements_of(expr, receiver)?;
                let pairs = elements.into_iter().enumerate().map(|(k, x)| self.make(CompositeKind::Tuple, vec![Constant::Int(k as i128), x])).collect();
                Ok(self.make(CompositeKind::Array, pairs))
            }
            Method::IteratorRev => {
                let mut elements = self.elements_of(expr, receiver)?;
                elements.reverse();
                Ok(self.make(CompositeKind::Array, elements))
            }
            _ => Err(ConstFault::refused(expr, ConstantForm::RowOutsideAppendixRows)),
        }
    }

    /// `map`, `filter`, `any` and `all`: the closure runs once per element in order; `any` and `all`
    /// stop at the element that decides (H.7 rule 10).
    pub(super) fn adapt(&mut self, expr: ExprId, method: Method, function: &FnValue, elements: Vec<Constant>, bindings: &Bindings) -> Result<Constant, ConstFault> {
        match method {
            Method::IteratorMap => {
                let mut mapped = Vec::with_capacity(elements.len());
                for element in elements {
                    mapped.push(self.call_with(expr, function, vec![element], bindings)?);
                }
                Ok(self.make(CompositeKind::Array, mapped))
            }
            Method::IteratorFilter => {
                let mut kept = Vec::new();
                for element in elements {
                    if self.test_with(expr, function, vec![element], bindings)? {
                        kept.push(element);
                    }
                }
                Ok(self.make(CompositeKind::Array, kept))
            }
            Method::IteratorAny => {
                for element in elements {
                    if self.test_with(expr, function, vec![element], bindings)? {
                        return Ok(Constant::Bool(true));
                    }
                }
                Ok(Constant::Bool(false))
            }
            _ => {
                for element in elements {
                    if !self.test_with(expr, function, vec![element], bindings)? {
                        return Ok(Constant::Bool(false));
                    }
                }
                Ok(Constant::Bool(true))
            }
        }
    }

    /// `sum()`: the elements added with `+` from the first, starting from zero (H.7 rule 9).
    pub(super) fn sum(&mut self, expr: ExprId, receiver: Constant) -> Result<Constant, ConstFault> {
        let elements = self.elements_of(expr, receiver)?;
        let zero = match self.typed(expr) {
            Some(Ty::F64) => Constant::Real(Real::new(0.0)),
            _ => Constant::Int(0),
        };
        let mut total = zero;
        for element in elements {
            total = self.known_binary(expr, BinOp::Add, (expr, total), (expr, element))?;
        }
        Ok(total)
    }

    /// `r.contains(x)` (H.6 rule 1): each bound the range has is tested; no float is in a range
    /// with a NaN bound.
    pub(super) fn range_contains(&mut self, expr: ExprId, range: Constant, sought: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.fold(sought, bindings)?;
        let (kind, parts) = self.parts_of(expr, range, "`contains` of a value")?;
        let CompositeKind::Range { low, high, inclusive } = kind else {
            return Err(ConstFault::refused(expr, ConstantForm::ContainsOfValueThatIsNotRange));
        };
        let mut holds = true;
        let mut bounds = parts.into_iter();
        if low {
            let lo = bounds.next().ok_or(ConstFault::refused(expr, ConstantForm::RangeWithNoLowBound))?;
            holds &= self.ordering(lo, value).is_some_and(|o| o.is_le());
        }
        if high {
            let hi = bounds.next().ok_or(ConstFault::refused(expr, ConstantForm::RangeWithNoHighBound))?;
            holds &= self.ordering(value, hi).is_some_and(|o| if inclusive { o.is_le() } else { o.is_lt() });
        }
        Ok(Constant::Bool(holds))
    }
}
