use ndarray::ArrayView1;
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Reverse;

use num_complex::Complex64;

use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::dc::DcAnalysisState;
use crate::analysis::noise::Noise;
use crate::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use crate::analog::AnalogReference;
use crate::digital::{DigitalEvent, DigitalNet, LogicValue};
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
    fn accept_timestep(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context, _nets: &[LogicValue], _event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {}

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

/// The purely digital contract.
pub trait DigitalDevice: Send + Sync {
    // ── Digital lifecycle ─────────────────────────────────────────────────────
    fn digital_init(&mut self, _event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {}
    fn digital_state_size(&self) -> usize { 0 }

    // ── Digital topology ──────────────────────────────────────────────────────
    fn digital_input_nets(&self) -> &[DigitalNet] { &[] }
    fn digital_output_nets(&self) -> &[DigitalNet] { &[] }

    fn has_digital_input_on(&self, changed: &HashSet<DigitalNet>) -> bool {
        self.digital_input_nets().iter().any(|n| changed.contains(n))
    }

    

    fn eval_discrete(
        &mut self,
        _t: f64,
        _nets: &[LogicValue],
        _analog_voltages: ArrayView1<f64>,
        _event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {}

    fn digital_seq_phase(&mut self, _t: f64, _nets: &[LogicValue], _analog_voltages: ArrayView1<f64>) -> bool {
        false
    }

    fn digital_comb_phase(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: ArrayView1<f64>,
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.eval_discrete(t, nets, analog_voltages, event_queue);
    }
}

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
