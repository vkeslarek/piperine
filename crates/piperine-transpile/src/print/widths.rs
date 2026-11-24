//! The number widths as Rust (composite-values phase D): how `f32`, `f16`, `f128` and an integer
//! wider than 64 bits are spelled in generated code. `f32` is Rust's own `f32`; the others are the
//! types of the embedded numeric library (`rt::Float16`, `rt::Float128`, `rt::WideUInt<N, L>`,
//! `rt::WideInt<N, L>`, with `L = ceil(N/64)` limbs).

use std::collections::BTreeSet;

use piperine_ir::{Expr, ExprId, Ir, TypeKind, ValueType};

use super::value::{Piece, Prec, ValuePrinter};
use crate::error::Error;

/// The Rust spelling of the number-width types.
pub struct NumberTypes;

impl NumberTypes {
    /// The Rust type of `ty`, which is one of the number widths (`f32`, `f16`, `f128`, an integer
    /// over 64 bits); any other type is spelled as the IR spells it (callers dispatch on the
    /// number widths first).
    pub fn spelled(ty: &ValueType) -> String {
        match ty {
            ValueType::F32 => "f32".to_string(),
            ValueType::F16 => "rt::Float16".to_string(),
            ValueType::F128 => "rt::Float128".to_string(),
            ValueType::UInt(width) if *width > 64 => format!("rt::WideUInt<{width}, {}>", width.div_ceil(64)),
            ValueType::Int(width) if *width > 64 => format!("rt::WideInt<{width}, {}>", width.div_ceil(64)),
            other => other.to_string(),
        }
    }
}

impl NumberTypes {
    /// Whether any type the design writes is a number width needing the embedded library: `f16`,
    /// `f128` or an integer wider than 64 bits, anywhere in a param, local, carrier, stream field,
    /// function or expression, through aggregates. (`f32` is Rust's own type and needs nothing.)
    pub fn used_by(ir: &Ir) -> bool {
        let mut seen = BTreeSet::new();
        let mut types: Vec<&ValueType> = ir.expression_types().collect();
        for body in ir.bodies() {
            types.extend(body.params().iter().map(|param| &param.ty));
            types.extend(body.locals().iter().map(|local| &local.ty));
            types.extend(body.carriers().iter().map(|carrier| &carrier.ty));
            types.extend(body.streams().iter().flat_map(|stream| stream.fields.iter().map(|field| &field.ty)));
            types.extend(body.reactions().iter().flat_map(|reaction| reaction.locals.iter().map(|local| &local.ty)));
        }
        types.into_iter().any(|ty| Self::within(ir, ty, &mut seen))
    }

    /// Whether `ty` is, or holds through tuples, arrays, options and the fields of arena types
    /// (each walked once through `seen`), a type that needs the embedded library.
    fn within(ir: &Ir, ty: &ValueType, seen: &mut BTreeSet<usize>) -> bool {
        match ty {
            ValueType::F16 | ValueType::F128 => true,
            ValueType::UInt(width) | ValueType::Int(width) => *width > 64,
            ValueType::Tuple(parts) => parts.iter().any(|part| Self::within(ir, part, seen)),
            ValueType::Array(element, _) | ValueType::Option(element) => Self::within(ir, element, seen),
            ValueType::Fn(params, ret) => params.iter().chain(std::iter::once(&**ret)).any(|part| Self::within(ir, part, seen)),
            ValueType::Struct(id) | ValueType::Enum(id) if seen.insert(id.index()) => match &ir.type_data(*id).kind {
                TypeKind::Struct { fields, .. } => fields.iter().any(|(_, field)| Self::within(ir, field, seen)),
                TypeKind::Enum { variants, .. } => variants.iter().any(|variant| variant.fields.iter().any(|(_, field)| Self::within(ir, field, seen))),
            },
            ValueType::Struct(_) | ValueType::Enum(_) | ValueType::F64 | ValueType::F32 | ValueType::Bool | ValueType::Unit => false,
        }
    }
}

/// What kind of number a value of a type is, for `as` (§1.4.5).
#[derive(Clone, Copy, PartialEq, Eq)]
enum NumberClass {
    /// `f16`, `f32`, `f64`, `f128`.
    Float,
    /// `UInt<N>`, `Int<N>` of any width.
    Integer,
}

impl NumberClass {
    /// The class of `ty`, `None` for a type `as` treats otherwise (a `bool`, an enum).
    fn of(ty: &ValueType) -> Option<NumberClass> {
        match ty {
            ValueType::F64 | ValueType::F32 | ValueType::F16 | ValueType::F128 => Some(NumberClass::Float),
            ValueType::UInt(_) | ValueType::Int(_) => Some(NumberClass::Integer),
            _ => None,
        }
    }
}

impl ValuePrinter<'_> {
    /// Whether `ty` is one of the number widths this file prints: `f32`, `f16`, `f128` or an
    /// integer over 64 bits.
    pub(super) fn is_number_width(ty: &ValueType) -> bool {
        ty.is_other_float() || ty.is_wide_integer()
    }

    /// The wide integer type `ty` as the path its associated functions are called through:
    /// `rt::WideUInt::<200, 4>`.
    fn wide_path(&self, ty: &ValueType, e: ExprId) -> Result<String, Error> {
        match ty {
            ValueType::UInt(width) if *width > 64 => Ok(format!("rt::WideUInt::<{width}, {}>", width.div_ceil(64))),
            ValueType::Int(width) if *width > 64 => Ok(format!("rt::WideInt::<{width}, {}>", width.div_ceil(64))),
            other => Err(self.unsupported(e, &format!("a wide integer of type {other}"))),
        }
    }

    /// A literal of a number width: `Expr::FloatBits` and `Expr::Wide` by the type of their
    /// position, and an `Expr::Int` of an integer wider than 64 bits.
    pub(super) fn number_literal(&self, e: ExprId) -> Result<Piece, Error> {
        let ty = self.ir.expr_type(e);
        let text = match (self.ir.expr(e), ty) {
            (Expr::FloatBits(bits), ValueType::F32) => format!("f32::from_bits(0x{bits:x}_u32)"),
            (Expr::FloatBits(bits), ValueType::F16) => format!("rt::Float16::from_bits(0x{bits:x}_u16)"),
            (Expr::FloatBits(bits), ValueType::F128) => format!("rt::Float128::from_bits(0x{bits:x}_u128)"),
            (Expr::Wide(id), _) => {
                let limbs: Vec<String> = self.ir.wide(*id).limbs.iter().map(|limb| format!("0x{limb:x}_u64")).collect();
                format!("{}::from_limbs(&[{}])", self.wide_path(ty, e)?, limbs.join(", "))
            }
            (Expr::Int(value), ValueType::UInt(_)) => format!("{}::from_u128({value}_u128)", self.wide_path(ty, e)?),
            (Expr::Int(value), ValueType::Int(_)) => format!("{}::from_i128({value}_i128)", self.wide_path(ty, e)?),
            (_, other) => return Err(self.unsupported(e, &format!("a literal of type {other}"))),
        };
        Ok(Piece::new(text, Prec::Atom))
    }

    /// The value of a number-width param nobody wrote: zero.
    pub(super) fn number_zero(ty: &ValueType) -> Option<String> {
        match ty {
            ValueType::F32 => Some("0.0_f32".to_string()),
            ValueType::F16 => Some("rt::Float16::from_bits(0)".to_string()),
            ValueType::F128 => Some("rt::Float128::from_bits(0)".to_string()),
            ValueType::UInt(width) if *width > 64 => Some(format!("rt::WideUInt::<{width}, {}>::ZERO", width.div_ceil(64))),
            ValueType::Int(width) if *width > 64 => Some(format!("rt::WideInt::<{width}, {}>::ZERO", width.div_ceil(64))),
            _ => None,
        }
    }

    /// `expr as to` when either side is a number width, through `rt::Convert` (each result
    /// rounded, wrapped or clamped once from its source); `None` when neither is. In `analog`
    /// code the tangent of a differentiable source is dropped, as an integer cast drops it (QD4).
    pub(super) fn number_cast(&self, e: ExprId, expr: ExprId, to: &ValueType) -> Result<Option<Piece>, Error> {
        let from = self.ir.expr_type(expr);
        if !Self::is_number_width(from) && !Self::is_number_width(to) {
            return Ok(None);
        }
        let mut operand = self.piece(expr)?.at(Prec::Atom);
        if *from == ValueType::F64 && !self.plain {
            operand = format!("{operand}.value()");
        }
        let converted = match (NumberClass::of(from), NumberClass::of(to)) {
            (Some(NumberClass::Float), Some(NumberClass::Float)) => format!("rt::Convert::float_to_float::<{}, {}>({operand})", self.convert_type(from, e)?, self.convert_type(to, e)?),
            (Some(NumberClass::Integer), Some(NumberClass::Float)) => format!("rt::Convert::integer_to_float::<{}, {}>({operand})", self.convert_type(from, e)?, self.convert_type(to, e)?),
            (Some(NumberClass::Float), Some(NumberClass::Integer)) => format!("rt::Convert::float_to_integer::<{}, {}>({operand})", self.convert_type(from, e)?, self.convert_type(to, e)?),
            (Some(NumberClass::Integer), Some(NumberClass::Integer)) => format!("rt::Convert::integer_to_integer::<{}, {}>({operand})", self.convert_type(from, e)?, self.convert_type(to, e)?),
            // a `bool` or an enum without fields to a wide integer (the narrow targets are the caller's)
            (None, Some(NumberClass::Integer)) => {
                let path = self.wide_path(to, e)?;
                let signed = matches!(to, ValueType::Int(_));
                match (from, signed) {
                    (ValueType::Bool, false) => format!("{path}::from_u64(u64::from({operand}))"),
                    (ValueType::Bool, true) => format!("{path}::from_i64(i64::from({operand}))"),
                    (ValueType::Enum(_), false) => format!("{path}::from_u64(({operand} as i64) as u64)"),
                    (ValueType::Enum(_), true) => format!("{path}::from_i64({operand} as i64)"),
                    (other, _) => return Err(self.unsupported(e, &format!("a cast from {other} to {to}"))),
                }
            }
            _ => return Err(self.unsupported(e, &format!("a cast from {from} to {to}"))),
        };
        let text = if *to == ValueType::F64 && !self.plain { format!("R::constant({converted})") } else { converted };
        Ok(Some(Piece::new(text, Prec::Atom)))
    }

    /// The Rust type `rt::Convert` is instantiated with: the plain `f64` for a real, whatever the
    /// code's own real is.
    fn convert_type(&self, ty: &ValueType, e: ExprId) -> Result<String, Error> {
        match ty {
            ValueType::F64 => Ok("f64".to_string()),
            other => self.rust_type(other, &self.path).map_err(|_| self.unsupported(e, &format!("the type {other}"))),
        }
    }

    /// The printed exponent of `pow` on a wide integer: the library takes a `u32`, the IR's
    /// exponent is an `rt::UInt<32>`.
    pub(super) fn wide_exponent(&self, exponent: ExprId) -> Result<String, Error> {
        Ok(format!("{}.get() as u32", self.piece(exponent)?.at(Prec::Atom)))
    }
}
