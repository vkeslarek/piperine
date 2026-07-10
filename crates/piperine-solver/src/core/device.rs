use num_complex::Complex64;

use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::dc::DcAnalysisState;
use crate::analysis::noise::Noise;
use crate::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use crate::analog::AnalogReference;
use crate::digital::LogicValue;
use crate::digital::interface::EventSink;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::solver::Context;

/// The purely analog contract.
pub trait AnalogDevice: Send + Sync {
    // ── Analog lifecycle ──────────────────────────────────────────────────────
    fn limiting_active(&self) -> bool { false }
    fn bound_step_hint(&self) -> f64 { f64::INFINITY }
    fn read_opvars(&self) -> Vec<(String, f64)> { Vec::new() }
    fn set_temperature(&mut self, _t: f64) {}
    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context) {}
    /// Called after each accepted solution point. Devices that couple into
    /// the digital world (A2D bridges, analog event detectors) emit their
    /// value-changes through `sink` — the same write-only façade digital
    /// devices use — so the analog contract never names the scheduler's
    /// concrete queue type.
    fn accept_timestep(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context, _nets: &[LogicValue], _sink: &mut dyn EventSink) {}

    // ── Analog loading ────────────────────────────────────────────────────────
    fn load_dc(
        &mut self,
        _state: &DcAnalysisState,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> { Vec::new() }

    fn load_ac(
        &mut self,
        _dc_op: &DcAnalysisResult,
        _ac_ctx: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> { Vec::new() }

    fn load_transient(
        &mut self,
        _states: &TransientAnalysisState,
        _tran_ctx: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> { Vec::new() }

    fn noise_current_psd(
        &mut self,
        _dc_point: &DcAnalysisResult,
        _ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> { Vec::new() }
}

pub use crate::digital::interface::DigitalDevice;

/// Unified simulation device — acts as a downcaster to domain-specific logic.
///
/// Pure-analog devices return `Some` for `as_analog` and `None` for `as_digital`.
/// Pure-digital devices return `None` for `as_analog` and `Some` for `as_digital`.
/// Mixed-signal devices return `Some` for both.
pub trait Device: Send + Sync {
    fn device_name(&self) -> &str;
    
    fn as_analog(&mut self) -> Option<&mut dyn AnalogDevice> { None }
    fn as_analog_ref(&self) -> Option<&dyn AnalogDevice> { None }
    fn as_digital(&mut self) -> Option<&mut dyn DigitalDevice> { None }
    fn as_digital_ref(&self) -> Option<&dyn DigitalDevice> { None }
}
