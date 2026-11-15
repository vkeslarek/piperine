//! The digital half of a body's plan (spec RS-12, design "Phase B: from a body to a shape"):
//! what the discrete forms of the IR become in the ABI. Everything here is appended after the
//! M1 entries of `plan.rs`, so an M1 body's pins, functions and their indices do not move.
//!
//! Carriers (RS-12 AC 1): one `DISCRETE` pin per carrier, in declaration order, after every M1
//! pin; `PORT` for an `in` or `out` port, `PRIVATE` for a `signal`. Its layout is one field `v`
//! with a known byte (abi.md P3): `BIT` for `bool`, `REAL` for `f64`, and for `UInt<N>`/`Int<N>`
//! the narrowest of `U8` to `U64` or `I8` to `I64` that holds `N` bits, the known byte right
//! after the value and the size rounded up to the value's alignment, as `BIT`, `BYTE` and
//! `REAL` have it. An initial value gives `HAS_INIT` with its canonical bytes: the value
//! little-endian, the known byte 1, every padding byte 0. An integer param takes the same
//! layout and default bytes as a carrier of its type.

use piperine_abi::layout::{BIT, BYTE, REAL, leaf_size};
use piperine_abi::raw::{Field, Layout, List, Str, field_flag, leaf, pin_flag, role};
use piperine_abi::shape::{PinSpec, ShapeBuf};
use piperine_ir::{Body, CarrierRole, Constant, ValueType};

/// The layout of a discrete value of a given IR type, and its bytes.
pub struct CarrierLayout;

static U16_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U16, 2)];
static U32_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U32, 4)];
static U64_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U64, 8)];
static I8_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I8, 1)];
static I16_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I16, 2)];
static I32_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I32, 4)];
static I64_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I64, 8)];

/// `{ v: U16 @0, known @2 }`, size 4, align 2.
static U16: Layout<'static> = Layout { size: 4, align: 2, fields: List::from_slice(&U16_FIELDS) };
/// `{ v: U32 @0, known @4 }`, size 8, align 4.
static U32: Layout<'static> = Layout { size: 8, align: 4, fields: List::from_slice(&U32_FIELDS) };
/// `{ v: U64 @0, known @8 }`, size 16, align 8.
static U64: Layout<'static> = Layout { size: 16, align: 8, fields: List::from_slice(&U64_FIELDS) };
/// `{ v: I8 @0, known @1 }`, size 2, align 1.
static I8: Layout<'static> = Layout { size: 2, align: 1, fields: List::from_slice(&I8_FIELDS) };
/// `{ v: I16 @0, known @2 }`, size 4, align 2.
static I16: Layout<'static> = Layout { size: 4, align: 2, fields: List::from_slice(&I16_FIELDS) };
/// `{ v: I32 @0, known @4 }`, size 8, align 4.
static I32: Layout<'static> = Layout { size: 8, align: 4, fields: List::from_slice(&I32_FIELDS) };
/// `{ v: I64 @0, known @8 }`, size 16, align 8.
static I64: Layout<'static> = Layout { size: 16, align: 8, fields: List::from_slice(&I64_FIELDS) };

impl CarrierLayout {
    /// The one field `v` of a layout: `leaf` at offset 0, its known byte at `known`.
    const fn field(leaf: u32, known: u32) -> Field<'static> {
        Field { name: Str::from_static("v"), leaf, offset: 0, count: 1, flags: field_flag::HAS_KNOWN, known }
    }

    /// The width in bytes of the value of an `N`-bit integer: the narrowest of 1, 2, 4 and 8.
    fn width(n: u32) -> u32 {
        match n {
            0..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// The layout of a carrier (or a discrete param) holding `ty`; `None` for a type no carrier
    /// holds (`check()` refuses it on a carrier).
    pub fn of(ty: &ValueType) -> Option<&'static Layout<'static>> {
        match *ty {
            ValueType::Bool => Some(&BIT),
            ValueType::F64 => Some(&REAL),
            ValueType::UInt(n) => Some(match Self::width(n) {
                1 => &BYTE,
                2 => &U16,
                4 => &U32,
                _ => &U64,
            }),
            ValueType::Int(n) => Some(match Self::width(n) {
                1 => &I8,
                2 => &I16,
                4 => &I32,
                _ => &I64,
            }),
            _ => None,
        }
    }

    /// The canonical bytes of `value` held as `ty` (abi.md P3): the value little-endian at
    /// offset 0, the known byte 1, padding 0. `None` when `value` is not a value of `ty`.
    pub fn bytes(ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        let layout = Self::of(ty)?;
        let field = layout.fields.get(0)?;
        let value: Vec<u8> = match (ty, value) {
            (ValueType::Bool, Constant::Bool(b)) => vec![u8::from(b)],
            (ValueType::F64, Constant::Real(r)) => r.get().to_le_bytes().to_vec(),
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Int(v)) => {
                let (lo, hi) = ty.integer_range()?;
                if v < lo || v > hi {
                    return None;
                }
                v.to_le_bytes()[..leaf_size(field.leaf)?].to_vec()
            }
            _ => return None,
        };
        let mut bytes = vec![0u8; layout.size as usize];
        bytes[..value.len()].copy_from_slice(&value);
        bytes[field.known as usize] = 1;
        Some(bytes)
    }
}

/// The discrete entries of a body's shape.
pub(crate) struct Digital;

impl Digital {
    /// Adds one pin per carrier of `body`, in declaration order; returns the index of the first
    /// (the index the next pin would get when the body has none). The body passes `check()`, so
    /// every carrier holds `bool`, `f64` or an integer type.
    pub(crate) fn carrier_pins(s: &mut ShapeBuf, body: Body<'_>) -> u32 {
        let first = s.shape().pins.len() as u32;
        for (i, c) in body.carriers().iter().enumerate() {
            let at = format!("{}:{}:{}", c.origin.file, c.origin.line, c.origin.col);
            let Some(layout) = CarrierLayout::of(&c.ty) else {
                panic!("body {}: carrier {} at {at} holds {}, which no pin holds; check() refuses it", body.name(), c.name, c.ty)
            };
            let init = match c.init {
                Some(value) => CarrierLayout::bytes(&c.ty, value).unwrap_or_else(|| {
                    panic!("body {}: carrier {} at {at}: initial value {value:?} is not a value of {}", body.name(), c.name, c.ty)
                }),
                None => Vec::new(),
            };
            let pin_role = match c.role {
                CarrierRole::In | CarrierRole::Out => role::PORT,
                CarrierRole::Signal => role::PRIVATE,
            };
            let flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            let pin = s.pin(PinSpec { origin: &at, flags, init: &init, ..PinSpec::discrete(&c.name, pin_role, layout) });
            debug_assert_eq!(pin, first + i as u32);
        }
        first
    }
}
