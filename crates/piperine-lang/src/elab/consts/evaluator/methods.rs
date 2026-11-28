//! Method calls of a const expression: a row of Appendix H applied to the constants of its receiver and arguments (§3.7).

use super::{Bindings, ConstEval, ConstFault};
use piperine_ir::{Constant, Real};
use crate::refusal::forms::{ConstantForm, ValueForm};
use crate::pom::{ExprId, ExprKind};
use crate::types::{Callee, Method, Ty};
use super::control::{Argument, FnValue};
use super::CompositeKind;

impl ConstEval<'_, '_> {
    /// A method call: a row of the Appendix H catalogue applied to
    /// its receiver and arguments.
    pub(super) fn method(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        // a method of the project, or the `impl` a bound picks for the concrete receiver: the function's body over the receiver,
        // then the arguments (generics T35, `dispatch.rs`)
        let called = match self.types.callee(expr) {
            Some(Callee::Method(function)) => Some(function),
            Some(Callee::Bound { item }) => self.typed(recv).and_then(|receiver| self.bound_implementation(expr, item, &receiver)),
            _ => None,
        };
        if let Some(function) = called {
            let mut arguments = Vec::new();
            for operand in std::iter::once(&recv).chain(args) {
                arguments.push(match self.fn_value_of(*operand) {
                    Some(function) => Argument { value: None, function: Some(function) },
                    None => Argument { value: Some(self.fold(*operand, bindings)?), function: None },
                });
            }
            return self.apply_value(expr, FnValue::Named(function), arguments, bindings);
        }
        if let Some(method) = self.types.method(expr) {
            // the rows over arrays, ranges, iterators and `Option`s take closures and composites (`composite.rs`)
            if let Some(result) = self.sequence_row(expr, method, recv, args, bindings) {
                return result;
            }
            if let Some(result) = self.option_row(expr, method, recv, args, bindings) {
                return result;
            }
        }
        if let Some(levels) = self.encoded_levels(expr, recv, args, bindings) {
            return levels;
        }
        let receiver = self.fold(recv, bindings);
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.fold(*arg, bindings));
        }
        if self.types.builtin(expr).is_some() {
            return Err(ConstFault::refused(expr, ConstantForm::MethodInConstExpression));
        }
        let Some(method) = self.types.method(expr) else {
            return Err(ConstFault::refused(expr, ValueForm::MethodOutsideCatalogue));
        };
        let (receiver, values) = (receiver?, values.into_iter().collect::<Result<Vec<_>, _>>()?);
        self.known_method(expr, recv, args, method, receiver, values)
    }

    /// `x.encode()` of a `T: Repr` parameter at `bool`, an integer or `f64` (§2 rules 2 and 3): the levels of the receiver,
    /// least significant first, as an array of `bool`. `None` when the call is no such `encode`.
    pub(super) fn encoded_levels(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Option<Result<Constant, ConstFault>> {
        let ExprKind::MethodCall { method, .. } = &self.design.expr_node(expr).kind else { return None };
        if method.text != "encode" || !args.is_empty() || self.types.method(expr).is_some() || self.types.builtin(expr).is_some() {
            return None;
        }
        let width = match self.typed(recv)? {
            Ty::Bool => 1,
            Ty::UInt(bits) | Ty::Int(bits) => u32::from(bits),
            Ty::F64 => 64,
            _ => return None,
        };
        let word = match self.fold(recv, bindings) {
            Ok(Constant::Bool(high)) => i128::from(high),
            Ok(Constant::Int(value)) => value,
            Ok(Constant::Real(real)) => i128::from(real.get().to_bits()),
            Ok(_) => return Some(Err(ConstFault::refused(expr, ConstantForm::EncodeOfValueThatIsNoScalar))),
            Err(fault) => return Some(Err(fault)),
        };
        let levels = (0..width).map(|level| Constant::Bool((word >> level) & 1 == 1)).collect();
        Some(Ok(self.make(CompositeKind::Array, levels)))
    }

    /// A catalogue row applied to constants.
    pub(super) fn known_method(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], method: Method, receiver: Constant, values: Vec<Constant>) -> Result<Constant, ConstFault> {
        if let Some(number) = self.number_method(expr, recv, args, method, receiver, &values) {
            return number;
        }
        let crossed = || Err(ConstFault::Panic { expr, what: "a `clamp` whose lower bound is above its upper bound".to_string() });
        if let Constant::Int(x) = receiver {
            let ints: Vec<i128> = values.iter().filter_map(|v| if let Constant::Int(i) = v { Some(*i) } else { None }).collect();
            return match (method, ints.as_slice()) {
                (Method::Min, [y]) => Ok(Constant::Int(x.min(*y))),
                (Method::Max, [y]) => Ok(Constant::Int(x.max(*y))),
                (Method::Clamp, [lo, hi]) if lo > hi => crossed(),
                (Method::Clamp, [lo, hi]) => Ok(Constant::Int(x.max(*lo).min(*hi))),
                (Method::RemEuclid, [0]) => Err(ConstFault::Panic { expr, what: "a `rem_euclid` by zero".to_string() }),
                (Method::RemEuclid, [d]) => Ok(Constant::Int(x.rem_euclid(*d))),
                (Method::IntAbs | Method::Signum | Method::Pow | Method::SaturatingAdd | Method::SaturatingSub | Method::SaturatingMul, _) => {
                    let Some(width) = self.width(expr) else { return Err(ConstFault::refused(expr, ConstantForm::MethodOfInteger)) };
                    match (method, ints.as_slice()) {
                        // Arithmetic wraps (§1): `(-128i8).abs()` is `-128`, `2u8.pow(9)` is 0.
                        (Method::IntAbs, []) => Ok(width.wrap(x.abs())),
                        (Method::Signum, []) => Ok(Constant::Int(x.signum())),
                        (Method::Pow, [e]) => {
                            let (mut base, mut acc, mut e) = (x, 1i128, *e);
                            while e > 0 {
                                if e & 1 == 1 {
                                    acc = width.wrapped(acc.wrapping_mul(base));
                                }
                                base = width.wrapped(base.wrapping_mul(base));
                                e >>= 1;
                            }
                            Ok(width.wrap(acc))
                        }
                        (Method::SaturatingAdd, [y]) => Ok(Constant::Int(width.clamp(x + *y))),
                        (Method::SaturatingSub, [y]) => Ok(Constant::Int(width.clamp(x - *y))),
                        (Method::SaturatingMul, [y]) => Ok(Constant::Int(width.clamp(x.saturating_mul(*y)))),
                        _ => Err(ConstFault::refused(expr, ConstantForm::MethodOfInteger)),
                    }
                }
                _ => Err(ConstFault::refused(expr, ConstantForm::MethodOfInteger)),
            };
        }
        let x = Self::real(recv, receiver)?;
        let mut reals = Vec::with_capacity(values.len());
        for (arg, value) in args.iter().zip(values) {
            reals.push(Self::real(*arg, value)?);
        }
        let result = match (method, reals.as_slice()) {
            (Method::Exp, []) => x.exp(),
            (Method::Ln, []) => x.ln(),
            (Method::Sqrt, []) => x.sqrt(),
            (Method::Abs, []) => x.abs(),
            (Method::Floor, []) => x.floor(),
            (Method::Min, [y]) => x.min(*y),
            (Method::Max, [y]) => x.max(*y),
            // `f64::clamp` panics on crossed or NaN bounds, as the compiled code does.
            (Method::Clamp, [lo, hi]) if lo.is_nan() || hi.is_nan() || lo > hi => return crossed(),
            (Method::Clamp, [lo, hi]) => x.clamp(*lo, *hi),
            // Constant from msl-digital T27 on, with the `Option` pair.
            _ => return Err(ConstFault::refused(expr, ConstantForm::MethodInConstExpression)),
        };
        Ok(Constant::Real(Real::new(result)))
    }
}
