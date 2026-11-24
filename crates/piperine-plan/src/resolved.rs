//! The pins of a resolved net (resolved-nets RN-03, abi.md §9 D1): a carrier of a resolved nature is the value pin
//! (the nature's `V`, read), one drive pin (`<carrier>.drive`, the driven type `X` followed by a plain `driving: BOOL`
//! byte, `RELAX`, written) and, when the body calls `others()`, one others pin (`<carrier>.others`, `V`, read). A
//! resolver body reads each drive net through a pin of the same drive layout, so the nets of a resolved carrier have
//! equal layouts on every pin (abi.md P4).

use piperine_abi::raw::{Field, Layout, List, Str, leaf};
use piperine_ir::{Ir, ValueType};

use crate::pin::PinLayout;

/// The layout of a drive net: the fields of the driven type `X`, then `driving: BOOL` at the next offset (a driver
/// writes it 1 with every value it writes; 0 is a drive absent, abi.md §9 D1).
#[derive(Clone, Debug)]
pub struct DriveLayout {
    value: PinLayout,
    /// The offset of the `driving` byte: the end of the value bytes.
    driving_at: u32,
}

impl DriveLayout {
    /// The name of the plain byte that marks a drive present.
    pub const DRIVING: &'static str = "driving";

    /// The drive layout of the driven type `driven` in the design `ir`; `None` for a type no pin holds.
    pub fn of(ir: &Ir, driven: &ValueType) -> Option<DriveLayout> {
        let value = PinLayout::in_ir(ir, driven)?;
        let driving_at = value.size();
        Some(DriveLayout { value, driving_at })
    }

    /// The offset of the `driving` byte inside a drive net.
    pub fn driving_at(&self) -> u32 {
        self.driving_at
    }

    /// The size of a drive net in bytes: the value bytes, the `driving` byte, rounded up to the alignment.
    pub fn size(&self) -> u32 {
        (self.driving_at + 1).next_multiple_of(self.align())
    }

    /// The alignment of a drive net: the value's.
    pub fn align(&self) -> u32 {
        self.value.align()
    }

    /// The ABI fields: those of the value, then `driving`.
    pub fn fields(&self) -> Vec<Field<'_>> {
        let mut fields = self.value.fields();
        fields.push(Field { name: Str::from_static(Self::DRIVING), leaf: leaf::BOOL, offset: self.driving_at, count: 1 });
        fields
    }

    /// The ABI layout over `fields` (`fields`).
    pub fn layout<'a>(&self, fields: &'a [Field<'a>]) -> Layout<'a> {
        Layout { size: self.size(), align: self.align(), fields: List::from_slice(fields) }
    }
}
