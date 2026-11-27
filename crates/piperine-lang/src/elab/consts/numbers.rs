//! The const evaluator over the number widths (composite-values phase D, CV-29 to CV-32, CV-36):
//! `f32`, `f16`, `f128` and the integers wider than 64 bits. A value of one of these types folds
//! to the value it has at run time: `f32` is Rust's `f32` (the generated code computes with it),
//! `f16` and `f128` are the library `piperine-numbers` (the generated code embeds the same files)
//! and a wide integer is a [`DynamicInteger`] (modulo `2^N`, §1 rule 6). A float of one of the
//! three formats is held as `Constant::FloatBits`, read by the type of its position; a wide
//! integer is `Constant::Wide`, a handle into this evaluator's arena that the elaborator copies
//! into the IR in the same order (`Elaboration::sync_wides`), so the handles are the IR's.
//!
//! Every entry returns `None` when the types involved are not number widths, so the evaluator's
//! own arms (`f64`, integers up to 64 bits) run unchanged for them.

use std::cmp::Ordering;

use piperine_ir::{Constant, Real, WideData, WideId};
use piperine_numbers::convert::NumberConversion;
use piperine_numbers::decimal::DecimalText;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::half::Float16;
use piperine_numbers::quad::Float128;

use super::floats::Ieee754;
use super::integers::DynamicInteger;
use super::{ConstEval, ConstFault};
use crate::refusal::forms::ConstantForm;
use crate::msl::lexer::NumberLiteral;
use crate::pom::{BinOp, ExprId, LitKind, UnOp};
use crate::types::{FloatText, Method, Ty};

impl ConstEval<'_, '_> {
    /// The wide constants made since the last call, in handle order: the elaborator copies them
    /// into the IR so a handle of the evaluator is the IR's (`Elaboration::sync_wides`).
    pub(in crate::elab) fn take_new_wides(&mut self) -> Vec<WideData> {
        let fresh = self.wides[self.wides_taken..].to_vec();
        self.wides_taken = self.wides.len();
        fresh
    }

    /// The wide constant of the value `value`: a handle into the arena (a repeated value reuses its handle).
    pub(in crate::elab) fn wide_constant(&mut self, value: &DynamicInteger) -> Constant {
        let data = WideData { limbs: value.canonical_limbs() };
        let at = match self.wides.iter().position(|held| *held == data) {
            Some(at) => at,
            None => {
                self.wides.push(data);
                self.wides.len() - 1
            }
        };
        Constant::Wide(WideId(at as u32))
    }

    /// The integer `constant` is, read at the type `ty`.
    fn integer_of(&self, ty: &Ty, constant: Constant) -> Option<DynamicInteger> {
        let (width, signed) = ty.integer_shape()?;
        match constant {
            Constant::Int(value) => Some(DynamicInteger::from_i128(width, signed, value)),
            Constant::Wide(id) => self.wides.get(id.index()).map(|data| DynamicInteger::from_pattern(width, signed, &data.limbs)),
            _ => None,
        }
    }

    /// The constant of an integer of type `ty`: a handle when wider than 64 bits.
    fn integer_constant(&mut self, value: &DynamicInteger) -> Constant {
        match value.to_i128() {
            Some(small) if value.width() <= 64 => Constant::Int(small),
            _ => self.wide_constant(value),
        }
    }

    /// The bit pattern of the float `constant` of format `ty`.
    fn float_pattern(constant: Constant) -> Option<u128> {
        match constant {
            Constant::FloatBits(bits) => Some(bits),
            Constant::Real(real) => Some(u128::from(real.get().to_bits())),
            _ => None,
        }
    }

    /// The constant of the float of `ty` with bit pattern `pattern`.
    fn float_constant(ty: &Ty, pattern: u128) -> Constant {
        match ty {
            Ty::F64 => Constant::Real(Real::new(f64::from_bits(pattern as u64))),
            _ => Constant::FloatBits(pattern),
        }
    }

    fn panic(expr: ExprId, what: &str) -> ConstFault {
        ConstFault::Panic { expr, what: what.to_string() }
    }

    /// A literal of a number width: a float read in its own format from its text (A.8 rule 8,
    /// never through `f64`), an integer wider than 64 bits.
    pub(super) fn number_literal(&mut self, expr: ExprId, kind: &LitKind) -> Option<Result<Constant, ConstFault>> {
        let ty = self.typed(expr)?;
        if let Some(format) = FloatText::format(&ty).filter(|_| ty.is_other_float()) {
            return Some(self.float_literal(expr, kind, format));
        }
        let (width, signed) = ty.integer_shape().filter(|(width, _)| *width > 64)?;
        let LitKind::Int(text) = kind else { return None };
        let value = NumberLiteral::new(text).int_value()?;
        let integer = DynamicInteger::from_pattern(width, signed, &[value as u64, (value >> 64) as u64]);
        Some(Ok(self.wide_constant(&integer)))
    }

    /// The float literal `kind` of `format`: from its decimal text, or, with an SI suffix, from the
    /// `f64` the suffix scaled.
    fn float_literal(&self, expr: ExprId, kind: &LitKind, format: BinaryFormat) -> Result<Constant, ConstFault> {
        let nearest = match kind {
            LitKind::Float { value, raw } if !NumberLiteral::new(raw).float_suffix().is_empty() && NumberLiteral::new(raw).float_suffix() != "f32" => {
                Some(NumberConversion::float_to_float(BinaryFormat::DOUBLE, u128::from(value.to_bits()), format))
            }
            LitKind::Float { raw, .. } => DecimalText::nearest(&Self::digits(raw), format),
            LitKind::Int(text) => DecimalText::nearest(&Self::digits(text), format),
            _ => None,
        };
        nearest.map(Constant::FloatBits).ok_or_else(|| ConstFault::refused(expr, ConstantForm::FloatLiteral))
    }

    /// The text of a number literal without its underscores and its `f32`/`f64` suffix.
    fn digits(text: &str) -> String {
        text.trim_end_matches("f32").trim_end_matches("f64").chars().filter(|c| *c != '_').collect()
    }

    /// A unary operator over a number width.
    pub(super) fn number_unary(&mut self, expr: ExprId, op: UnOp, operand: ExprId, value: Constant) -> Option<Result<Constant, ConstFault>> {
        let ty = self.typed(operand)?;
        if !ty.is_number_width() {
            return None;
        }
        if ty.is_other_float() {
            return Some(match (op, Self::float_pattern(value)) {
                (UnOp::Neg, Some(pattern)) => Ok(Constant::FloatBits(Self::negated(&ty, pattern))),
                _ => Err(ConstFault::refused(expr, ConstantForm::OperatorOnFloat)),
            });
        }
        let integer = self.integer_of(&ty, value)?;
        let result = match op {
            UnOp::Neg => integer.wrapping_neg(),
            UnOp::Not => integer.complement(),
        };
        Some(Ok(self.integer_constant(&result)))
    }

    /// The pattern of `-x` for the float `pattern` of type `ty`: the sign bit flipped (NaN included).
    fn negated(ty: &Ty, pattern: u128) -> u128 {
        let sign = FloatText::format(ty).map_or(0, |format| format.sign_bit());
        pattern ^ sign
    }

    /// A binary operator when either operand is a number width.
    pub(super) fn number_binary(&mut self, expr: ExprId, op: BinOp, (lhs, left): (ExprId, Constant), (rhs, right): (ExprId, Constant)) -> Option<Result<Constant, ConstFault>> {
        let ty = self.typed(lhs)?;
        if ty.is_other_float() {
            return Some(self.float_binary(expr, op, &ty, left, right));
        }
        if ty.is_wide_integer() {
            let amount_ty = self.typed(rhs)?;
            return Some(self.integer_binary_wide(expr, op, &ty, left, (&amount_ty, right)));
        }
        None
    }

    /// An operator over two floats of one of the three formats.
    fn float_binary(&self, expr: ExprId, op: BinOp, ty: &Ty, left: Constant, right: Constant) -> Result<Constant, ConstFault> {
        let (Some(a), Some(b)) = (Self::float_pattern(left), Self::float_pattern(right)) else {
            return Err(ConstFault::refused(expr, ConstantForm::OperatorOverFloatAndAnotherType));
        };
        match ty {
            Ty::F32 => Self::float_operation::<f32>(expr, op, a, b),
            Ty::F16 => Self::float_operation::<Float16>(expr, op, a, b),
            _ => Self::float_operation::<Float128>(expr, op, a, b),
        }
    }

    /// `a op b` over the patterns of two values of the float type `T`, with IEEE 754 comparisons.
    fn float_operation<T: Ieee754>(expr: ExprId, op: BinOp, a: u128, b: u128) -> Result<Constant, ConstFault> {
        let (x, y) = (T::from_pattern(a), T::from_pattern(b));
        let arithmetic = |value: T| Ok(Constant::FloatBits(value.pattern()));
        match op {
            BinOp::Add => arithmetic(x + y),
            BinOp::Sub => arithmetic(x - y),
            BinOp::Mul => arithmetic(x * y),
            BinOp::Div => arithmetic(x / y),
            BinOp::Lt => Ok(Constant::Bool(x < y)),
            BinOp::Le => Ok(Constant::Bool(x <= y)),
            BinOp::Gt => Ok(Constant::Bool(x > y)),
            BinOp::Ge => Ok(Constant::Bool(x >= y)),
            BinOp::Eq => Ok(Constant::Bool(x == y)),
            BinOp::Ne => Ok(Constant::Bool(x != y)),
            _ => Err(ConstFault::refused(expr, ConstantForm::OperatorOnFloat)),
        }
    }

    /// An operator over two integers wider than 64 bits (the shift amount of any integer type).
    fn integer_binary_wide(&mut self, expr: ExprId, op: BinOp, ty: &Ty, left: Constant, (amount_ty, right): (&Ty, Constant)) -> Result<Constant, ConstFault> {
        let Some(a) = self.integer_of(ty, left) else { return Err(ConstFault::refused(expr, ConstantForm::IntegerOperatorOverNonInteger)) };
        if matches!(op, BinOp::Shl | BinOp::Shr) {
            let Some(amount) = self.integer_of(amount_ty, right) else { return Err(ConstFault::refused(expr, ConstantForm::ShiftByNonInteger)) };
            let Some(count) = amount.shift_amount() else { return Err(Self::panic(expr, "a shift by a negative amount")) };
            let shifted = if op == BinOp::Shl { a.shift_left(count) } else { a.shift_right(count) };
            return Ok(self.integer_constant(&shifted));
        }
        let Some(b) = self.integer_of(ty, right) else { return Err(ConstFault::refused(expr, ConstantForm::IntegerOperatorOverNonInteger)) };
        let ordering = a.compare(&b);
        let result = match op {
            BinOp::Lt => return Ok(Constant::Bool(ordering == Ordering::Less)),
            BinOp::Le => return Ok(Constant::Bool(ordering != Ordering::Greater)),
            BinOp::Gt => return Ok(Constant::Bool(ordering == Ordering::Greater)),
            BinOp::Ge => return Ok(Constant::Bool(ordering != Ordering::Less)),
            BinOp::Eq => return Ok(Constant::Bool(ordering == Ordering::Equal)),
            BinOp::Ne => return Ok(Constant::Bool(ordering != Ordering::Equal)),
            BinOp::Add => a.wrapping_add(&b),
            BinOp::Sub => a.wrapping_sub(&b),
            BinOp::Mul => a.wrapping_mul(&b),
            BinOp::Div => a.divide(&b).ok_or_else(|| Self::panic(expr, "an integer division by zero"))?,
            BinOp::Rem => a.remainder(&b).ok_or_else(|| Self::panic(expr, "an integer remainder by zero"))?,
            BinOp::BitAnd => a.and(&b),
            BinOp::BitOr => a.or(&b),
            BinOp::BitXor => a.xor(&b),
            _ => return Err(ConstFault::refused(expr, ConstantForm::OperatorOnIntegers)),
        };
        Ok(self.integer_constant(&result))
    }

    /// A catalogue row applied to a receiver that is a number width.
    pub(super) fn number_method(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], method: Method, receiver: Constant, values: &[Constant]) -> Option<Result<Constant, ConstFault>> {
        let ty = self.typed(recv)?;
        if ty.is_other_float() {
            return Some(self.float_method(expr, &ty, method, receiver, values));
        }
        if ty.is_wide_integer() {
            let arg_types: Vec<Ty> = args.iter().filter_map(|arg| self.typed(*arg)).collect();
            return Some(self.integer_method(expr, &ty, method, receiver, values, &arg_types));
        }
        None
    }

    /// An H.2 row over a float of one of the three formats.
    fn float_method(&self, expr: ExprId, ty: &Ty, method: Method, receiver: Constant, values: &[Constant]) -> Result<Constant, ConstFault> {
        let Some(pattern) = Self::float_pattern(receiver) else { return Err(ConstFault::refused(expr, ConstantForm::MethodOfFloatOverNonFloat)) };
        let arguments: Vec<u128> = values.iter().filter_map(|value| Self::float_pattern(*value)).collect();
        let powi = || match values {
            [Constant::Int(exponent)] => i32::try_from(*exponent).ok(),
            _ => None,
        };
        match ty {
            Ty::F32 => Self::float_row::<f32>(expr, method, pattern, &arguments, powi()),
            Ty::F16 => Self::float_row::<Float16>(expr, method, pattern, &arguments, powi()),
            _ => Self::float_row::<Float128>(expr, method, pattern, &arguments, powi()),
        }
    }

    /// The H.2 row `method` of the float type `T` over its receiver and arguments.
    fn float_row<T: Ieee754>(expr: ExprId, method: Method, pattern: u128, arguments: &[u128], exponent: Option<i32>) -> Result<Constant, ConstFault> {
        let x = T::from_pattern(pattern);
        let float = |value: T| Ok(Constant::FloatBits(value.pattern()));
        match (method, arguments) {
            (Method::IsNan, []) => Ok(Constant::Bool(x.is_nan())),
            (Method::IsFinite, []) => Ok(Constant::Bool(x.is_finite())),
            (Method::IsInfinite, []) => Ok(Constant::Bool(x.is_infinite())),
            (Method::Powi, []) => exponent.map_or_else(|| Err(ConstFault::refused(expr, ConstantForm::PowiByNonConstantExponent)), |e| float(x.power_integer(e))),
            (Method::Clamp, [low, high]) => x.clamp(T::from_pattern(*low), T::from_pattern(*high)).map_or_else(|| Err(Self::panic(expr, "a `clamp` whose bounds are crossed or NaN")), float),
            (_, []) => x.unary(method).map_or_else(|| Err(ConstFault::refused(expr, ConstantForm::MethodOfFloat)), float),
            (_, [other]) => x.binary(method, T::from_pattern(*other)).map_or_else(|| Err(ConstFault::refused(expr, ConstantForm::MethodOfFloat)), float),
            _ => Err(ConstFault::refused(expr, ConstantForm::MethodOfFloat)),
        }
    }

    /// An H.1 row over an integer wider than 64 bits.
    fn integer_method(&mut self, expr: ExprId, ty: &Ty, method: Method, receiver: Constant, values: &[Constant], arg_types: &[Ty]) -> Result<Constant, ConstFault> {
        let Some(x) = self.integer_of(ty, receiver) else { return Err(ConstFault::refused(expr, ConstantForm::MethodOfIntegerOverNonInteger)) };
        let same: Vec<DynamicInteger> = values.iter().filter_map(|value| self.integer_of(ty, *value)).collect();
        let result = match (method, same.as_slice()) {
            (Method::Pow, _) => {
                let exponent = values.first().zip(arg_types.first()).and_then(|(value, ty)| self.integer_of(ty, *value)).and_then(|e| e.shift_amount());
                x.power(exponent.ok_or_else(|| ConstFault::refused(expr, ConstantForm::PowByNonConstantExponent))?)
            }
            (Method::Min, [y]) => x.minimum(y),
            (Method::Max, [y]) => x.maximum(y),
            (Method::Clamp, [low, high]) => x.clamp(low, high).ok_or_else(|| Self::panic(expr, "a `clamp` whose lower bound is above its upper bound"))?,
            (Method::RemEuclid, [d]) => x.remainder_euclid(d).ok_or_else(|| Self::panic(expr, "a `rem_euclid` by zero"))?,
            (Method::IntAbs, []) => x.absolute(),
            (Method::Signum, []) => x.signum(),
            (Method::SaturatingAdd, [y]) => x.saturating_add(y),
            (Method::SaturatingSub, [y]) => x.saturating_sub(y),
            (Method::SaturatingMul, [y]) => x.saturating_mul(y),
            _ => return Err(ConstFault::refused(expr, ConstantForm::MethodOfInteger)),
        };
        Ok(self.integer_constant(&result))
    }

    /// `operand as target` when either type is a number width: every pair rounded, wrapped or
    /// clamped once from the source (§1.4.5), never through an intermediate format.
    pub(super) fn number_cast(&mut self, expr: ExprId, source: &Ty, value: Constant) -> Option<Result<Constant, ConstFault>> {
        let target = self.typed(expr)?;
        if !source.is_number_width() && !target.is_number_width() {
            return None;
        }
        Some(self.cast_number(expr, source, &target, value))
    }

    /// The conversion of `value` of type `source` to `target`.
    fn cast_number(&mut self, expr: ExprId, source: &Ty, target: &Ty, value: Constant) -> Result<Constant, ConstFault> {
        let refused = || ConstFault::refused(expr, ConstantForm::Cast);
        if let Some(from) = FloatText::format(source) {
            let pattern = Self::float_pattern(value).ok_or_else(refused)?;
            if let Some(to) = FloatText::format(target) {
                return Ok(Self::float_constant(target, NumberConversion::float_to_float(from, pattern, to)));
            }
            let (width, signed) = target.integer_shape().ok_or_else(refused)?;
            let integer = DynamicInteger::from_float(width, signed, from, pattern);
            return Ok(self.integer_constant(&integer));
        }
        let integer = match (source, value) {
            (Ty::Bool, Constant::Bool(flag)) => {
                let (width, signed) = target.integer_shape().unwrap_or((1, false));
                DynamicInteger::from_i128(width, signed, i128::from(flag))
            }
            // an enum without fields converts to its discriminant
            (_, Constant::Int(discriminant)) if source.integer_shape().is_none() => DynamicInteger::from_i128(128, true, discriminant),
            (_, constant) => self.integer_of(source, constant).ok_or_else(refused)?,
        };
        if let Some(to) = FloatText::format(target) {
            return Ok(Self::float_constant(target, integer.to_float(to)));
        }
        let (width, signed) = target.integer_shape().ok_or_else(refused)?;
        let converted = integer.convert(width, signed);
        Ok(self.integer_constant(&converted))
    }
}
