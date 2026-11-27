//! The rules of the number widths: a float of a width other than `f64` and an integer wider than 64
//! bits, as constants and literals (Q4 of the composite values, phase D).
//!
//! A `FloatBits` constant holds the IEEE 754 bit pattern of its type and no bit past the type's
//! width. A `Wide` constant names an entry of the arena of wide constants and is a value of an
//! integer type over 64 bits: it holds `ceil(N/64)` limbs, and for `UInt<N>` no bit past `N`, for
//! `Int<N>` the sign extended to the limb boundary.

use super::walk::{CodeWalk, ExpressionVisit};
use super::{IrReader, TypeCheck};
use crate::ids::WideId;
use crate::model::ValueType;

impl IrReader<'_> {
    /// Whether `bits` is a bit pattern of the float type `ty`: no bit set past its width.
    pub(super) fn float_bits_fit(&self, ty: &ValueType, bits: u128) -> bool {
        match ty.float_width() {
            Some(128) => true,
            Some(width) => bits >> width == 0,
            None => false,
        }
    }

    /// Whether the wide constant `id` is a value of the integer type `ty`, wider than 64 bits.
    pub(super) fn wide_fits(&self, ty: &ValueType, id: WideId) -> bool {
        let Some(data) = self.ir.wides.get(id.index()) else { return false };
        let (width, signed) = match ty {
            ValueType::UInt(width) if *width > 64 => (*width, false),
            ValueType::Int(width) if *width > 64 => (*width, true),
            _ => return false,
        };
        let limbs = width.div_ceil(64) as usize;
        if data.limbs.len() != limbs {
            return false;
        }
        let used = width % 64;
        if used == 0 {
            return true;
        }
        let top = data.limbs[limbs - 1];
        let high_bits = top >> used;
        let extension = u64::MAX >> used;
        if signed {
            let negative = top >> (used - 1) & 1 == 1;
            high_bits == if negative { extension } else { 0 }
        } else {
            high_bits == 0
        }
    }
}

impl TypeCheck<'_> {
    /// A `FloatBits` literal is a value of `f32`, `f16` or `f128` with no bit past the width
    /// (checked on the first visit of the literal).
    pub(super) fn float_literal(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, bits: u128) {
        let (origin, ty) = (&visit.origin, visit.recorded_type);
        if !visit.first {
            return;
        }
        if !ty.is_other_float() {
            self.wrong(walk, origin.clone(), format!("a float bit literal must have the type `f32`, `f16` or `f128`, found {ty}"));
        } else if !self.reader.float_bits_fit(ty, bits) {
            self.wrong(walk, origin.clone(), format!("the bit pattern 0x{bits:x} has a bit past the width of {ty}"));
        }
    }

    /// A `Wide` literal names an entry of the arena and is a value of an integer type over 64 bits
    /// (checked on the first visit of the literal).
    pub(super) fn wide_literal(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, id: WideId) {
        let (origin, ty) = (&visit.origin, visit.recorded_type);
        if !visit.first {
            return;
        }
        if self.reader.ir.wides.get(id.index()).is_none() {
            self.wrong(walk, origin.clone(), format!("wide constant {} is outside the arena ({} entries)", id.0, self.reader.ir.wides.len()));
        } else if !ty.is_wide_integer() {
            self.wrong(walk, origin.clone(), format!("a wide literal must have an integer type wider than 64 bits, found {ty}"));
        } else if !self.reader.wide_fits(ty, id) {
            self.wrong(walk, origin.clone(), format!("wide constant {} is not a canonical value of {ty}", id.0));
        }
    }
}
