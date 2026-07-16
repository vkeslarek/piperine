use crate::analog::AnalogReference;
use crate::digital::LogicValue;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::solver::Context;
use std::ops::Deref;

/// The read-only state an element sees while stamping the DC system: the analog
/// solution history **and** the digital net snapshot it may read (D2A — an
/// analog stamp that depends on digital logic reads it here, with no device-side
/// cache). Derefs to the analog history so existing history access is unchanged.
pub struct DcAnalysisState<'a> {
    history: &'a CircularArrayBuffer2<f64>,
    /// Every digital net's logic value for this solve, indexed by `DigitalNet`.
    pub digital: &'a [LogicValue],
    /// Source-stepping homotopy scale (SPICE): every forced source value is
    /// multiplied by this. `1.0` in normal operation; the DC solver ramps it
    /// `0 → 1` while tracking a hard operating point. Elements that drive forced
    /// sources read it here instead of a mutable `Context` field.
    pub src_scale: f64,
}

impl<'a> DcAnalysisState<'a> {
    pub fn new(
        history: &'a CircularArrayBuffer2<f64>,
        digital: &'a [LogicValue],
        src_scale: f64,
    ) -> Self {
        Self { history, digital, src_scale }
    }

    /// The analog solution history buffer.
    pub fn history(&self) -> &CircularArrayBuffer2<f64> {
        self.history
    }
}

impl Deref for DcAnalysisState<'_> {
    type Target = CircularArrayBuffer2<f64>;
    fn deref(&self) -> &Self::Target {
        self.history
    }
}

pub trait DcAnalysis {
    fn load_dc(
        &mut self,
        dc_circuit_state: &DcAnalysisState<'_>,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn initial_dc_values(&mut self, _context: &Context) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}

