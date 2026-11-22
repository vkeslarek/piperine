//! The layout of any pin that holds a value: a scalar's static layout (`CarrierLayout`) or a
//! composite's (`CompositeLayout`).

use piperine_abi::raw::{Field, Layout, List};
use piperine_ir::{Constant, Ir, ValueType};

use crate::composite::CompositeLayout;
use crate::digital::CarrierLayout;

/// What a carrier, a param or a record field of a given IR type is laid out as.
#[derive(Clone, Debug)]
pub enum PinLayout {
    /// A scalar (`bool`, `f64`, an integer, an enum that is its tag): one of `CarrierLayout`'s.
    Scalar(&'static Layout<'static>),
    /// A tuple, an array, a struct, an enum with a payload, an `Option`.
    Composite(CompositeLayout),
}

impl PinLayout {
    /// The layout of a pin holding `ty` in the design `ir`; `None` for a type no pin holds.
    pub fn in_ir(ir: &Ir, ty: &ValueType) -> Option<PinLayout> {
        match CarrierLayout::in_ir(ir, ty) {
            Some(layout) => Some(PinLayout::Scalar(layout)),
            None => CompositeLayout::of(ir, ty).map(PinLayout::Composite),
        }
    }

    /// The canonical bytes of `value` held as `ty` (abi.md P3); `None` when it is no value of `ty`.
    pub fn bytes_in(ir: &Ir, ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        match CarrierLayout::in_ir(ir, ty) {
            Some(_) => CarrierLayout::bytes_in(ir, ty, value),
            None => CompositeLayout::bytes(ir, ty, value),
        }
    }

    /// The size of the pin in bytes.
    pub fn size(&self) -> u32 {
        match self {
            PinLayout::Scalar(layout) => layout.size,
            PinLayout::Composite(layout) => layout.size(),
        }
    }

    /// The alignment of the pin in bytes.
    pub fn align(&self) -> u32 {
        match self {
            PinLayout::Scalar(layout) => layout.align,
            PinLayout::Composite(_) => 1,
        }
    }

    /// The ABI fields of the layout.
    pub fn fields(&self) -> Vec<Field<'_>> {
        match self {
            PinLayout::Scalar(layout) => layout.fields.iter().copied().collect(),
            PinLayout::Composite(layout) => layout.abi_fields(),
        }
    }

    /// The ABI layout over `fields` (`fields`).
    pub fn layout<'a>(&self, fields: &'a [Field<'a>]) -> Layout<'a> {
        Layout { size: self.size(), align: self.align(), fields: List::from_slice(fields) }
    }
}
