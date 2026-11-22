//! The edges of a [`Clock`](../../e2e/integrated/src/lib.msl): the instants and the print rows the
//! closed forms and the reference models of the scenarios are written against.

/// A clock of frequency `frequency` hertz that starts low and first changes at half a period, so rising
/// edge `k` (from 1) falls at `(k - 1/2) / f` and falling edge `k` at `k / f`.
#[derive(Debug, Clone, Copy)]
pub struct ClockEdges {
    /// The clock's frequency, in hertz.
    pub frequency: f64,
}

impl ClockEdges {
    /// A clock of `frequency` hertz.
    pub fn of(frequency: f64) -> ClockEdges {
        ClockEdges { frequency }
    }

    /// The instant of rising edge `k`, counted from 1.
    pub fn rising(&self, k: usize) -> f64 {
        (k as f64 - 0.5) / self.frequency
    }

    /// The instant of falling edge `k`, counted from 1.
    pub fn falling(&self, k: usize) -> f64 {
        k as f64 / self.frequency
    }
}
