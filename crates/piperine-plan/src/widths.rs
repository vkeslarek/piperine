//! The layouts and bytes of the number widths (composite-values phase D, Q4 of the user: the ABI
//! gains no leaf, the IR type is the metadata that says these bytes are one number).
//!
//! `f32` is the `F32` leaf. `f16` is one `U16` field. `f128` is one `U64` field of `count` 2, and
//! an integer of `N > 64` bits one `U64` field of `count` `ceil(N/64)`, low limb first: the bit
//! pattern of spec §2's table (`Float<N>`: the IEEE 754 bits; an integer: two's complement at the
//! type's width, sign-extended to the limb boundary for `Int<N>`). Two pins of one type have equal
//! layouts (abi.md P4) because the layout is a function of the IR type alone.

use piperine_abi::raw::{Field, Layout, List, Str, leaf};
use piperine_ir::{Ir, ValueType, WideId};

static F32_FIELDS: [Field<'static>; 1] = [Field { name: Str::from_static("v"), leaf: leaf::F32, offset: 0, count: 1 }];
static F128_FIELDS: [Field<'static>; 1] = [Field { name: Str::from_static("v"), leaf: leaf::U64, offset: 0, count: 2 }];

/// `{ v: F32 @0 }`, size 4, align 4.
static F32: Layout<'static> = Layout { size: 4, align: 4, fields: List::from_slice(&F32_FIELDS) };
/// `{ v: U64 @0 x 2 }`, size 16, align 8: the layout of an `f128`.
static F128: Layout<'static> = Layout { size: 16, align: 8, fields: List::from_slice(&F128_FIELDS) };

/// The layout and the constant bytes of one number-width type.
pub struct NumberLayout;

impl NumberLayout {
    /// The static layout of `f32` and `f128`; `None` for every other type (`f16` is a plain `U16`,
    /// and a wide integer's layout depends on its width, so it has no static one).
    pub fn static_layout(ty: &ValueType) -> Option<&'static Layout<'static>> {
        match ty {
            ValueType::F32 => Some(&F32),
            ValueType::F128 => Some(&F128),
            _ => None,
        }
    }

    /// The leaf, the element count and the size in bytes of the one field a number-width type is:
    /// `None` for any other type.
    pub fn field_of(ty: &ValueType) -> Option<(u32, u32, u32)> {
        match ty {
            ValueType::F32 => Some((leaf::F32, 1, 4)),
            ValueType::F16 => Some((leaf::U16, 1, 2)),
            ValueType::F128 => Some((leaf::U64, 2, 16)),
            ValueType::UInt(width) | ValueType::Int(width) if *width > 64 => {
                let count = width.div_ceil(64);
                Some((leaf::U64, count, count * 8))
            }
            _ => None,
        }
    }

    /// The bytes of the float of type `ty` with the IEEE 754 bit pattern `bits`, little-endian;
    /// `None` for a type that is no `f32`, `f16` or `f128`, or a pattern with a bit past the width.
    pub fn float_bytes(ty: &ValueType, bits: u128) -> Option<Vec<u8>> {
        let width = ty.float_width().filter(|width| *width != 64)?;
        if width < 128 && bits >> width != 0 {
            return None;
        }
        Some(bits.to_le_bytes()[..(width / 8) as usize].to_vec())
    }

    /// The bytes of the wide constant `id` held as the integer type `ty` over 64 bits: its limbs,
    /// low limb first, each little-endian; `None` when `ty` is not such a type or the constant
    /// does not have its limb count.
    pub fn wide_bytes(ir: &Ir, ty: &ValueType, id: WideId) -> Option<Vec<u8>> {
        let (_, count, _) = Self::field_of(ty).filter(|_| ty.is_wide_integer())?;
        let limbs = &ir.wides().get(id.index())?.limbs;
        if limbs.len() != count as usize {
            return None;
        }
        Some(limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect())
    }
}
