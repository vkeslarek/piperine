//! The discrete slots of an assembly, read-only: what a `DELAY` bound and a `CROSS` tolerance are
//! read from.

use super::cell::RealCell;
use super::plan::TermPlan;

/// The assembly's discrete slots and which of them the discrete side has written.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SlotView<'a> {
    /// The bytes of each slot.
    pub bytes: &'a [Vec<u8>],
    /// Whether the discrete side has written each slot since the assembly was made.
    pub written: &'a [bool],
}

impl SlotView<'_> {
    /// The `F64` at byte 0 of `slot`; `None` while the discrete side has not written it.
    pub fn written_real(&self, slot: usize) -> Option<f64> {
        self.written[slot].then(|| RealCell::read(&self.bytes[slot]))
    }

    /// The bound of a `DELAY` term: the `F64` at byte 0 of its `max` slot, or unbounded. A slot the
    /// parameter cone has not written yet (the operating point's first solve comes before the cone's
    /// first run) is unbounded too: its zero bytes are no bound.
    pub fn bound_of(&self, term: &TermPlan) -> f64 {
        term.max.and_then(|slot| self.written_real(slot)).unwrap_or(f64::INFINITY)
    }
}
