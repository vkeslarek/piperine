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

/// Unified simulation device — covers analog (continuous) and digital (discrete) behavior.
///
/// Any device may implement any subset of these methods; the rest default to no-ops.
/// Pure-analog devices leave the `digital_*` methods at their defaults.
/// Pure-digital devices leave the analog loading methods at their defaults.
/// Mixed-signal devices implement both sides.
///
/// DAG topology is built from all devices in the circuit; purely analog devices have
/// no digital nets so they are naturally isolated in the topology graph.
pub trait Device: Send + Sync {
    fn device_name(&self) -> &str;

    // ── Analog lifecycle ──────────────────────────────────────────────────────
    fn limiting_active(&self) -> bool { false }
    fn bound_step_hint(&self) -> f64 { f64::INFINITY }
    fn read_opvars(&self) -> Vec<(String, f64)> { Vec::new() }
    fn set_temperature(&mut self, _t: f64) {}
    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context) {}
    fn accept_timestep(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context) {}

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

    // ── Digital lifecycle ─────────────────────────────────────────────────────
    /// Called once before simulation begins.
    ///
    /// Devices initialize internal state variables and schedule any t=0 events
    /// (e.g. power-on reset values) into `event_queue`.  The circuit runner
    /// collects these events and seeds the `DigitalState` before the first
    /// `evaluate_until_stable` call.
    fn digital_init(&mut self, _event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {}
    /// Number of f64 slots this device needs in the shared digital state buffer.
    fn digital_state_size(&self) -> usize { 0 }

    // ── Digital topology ──────────────────────────────────────────────────────
    fn digital_input_nets(&self) -> &[DigitalNet] { &[] }
    fn digital_output_nets(&self) -> &[DigitalNet] { &[] }

    fn has_digital_input_on(&self, changed: &HashSet<DigitalNet>) -> bool {
        self.digital_input_nets().iter().any(|n| changed.contains(n))
    }

    /// Whether this device's digital body samples analog quantities (A2D).
    /// Such devices receive no digital input event when only an analog
    /// voltage moved, so the mixed-signal loop must re-evaluate them
    /// explicitly after every accepted analog solution.
    fn samples_analog(&self) -> bool { false }

    /// Digital evaluation — called during the event-driven phase.
    ///
    /// `nets` — current logic state for all digital nets.
    /// `analog_voltages` — per-analog-terminal voltages (available for analog-dependent
    ///   digital logic; currently always `&[]` but wired for future mixed-signal use).
    /// `event_queue` — future events emitted by this device.
    fn eval_discrete(
        &mut self,
        _t: f64,
        _nets: &[LogicValue],
        _analog_voltages: &[f64],
        _event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {}

    /// Phase 1 of a synchronous digital delta cycle (Verilator-style
    /// two-phase register commit — see `topology::DigitalState::evaluate_dag_ordered`).
    /// Detects clock edges against `nets` and, if a clocked block fires,
    /// commits this device's register writes using the *pre-settle* net
    /// snapshot — never touching output nets. Every device in the delta
    /// cycle runs this phase before any device runs [`Device::digital_comb_phase`],
    /// so a chain of registers samples the same pre-edge values instead of
    /// racing each other within one edge (SPEC §9 non-blocking semantics).
    /// Returns whether a clocked block fired (forces a comb re-evaluation
    /// even when no input net changed).
    fn digital_seq_phase(&mut self, _t: f64, _nets: &[LogicValue], _analog_voltages: &[f64]) -> bool {
        false
    }

    /// Phase 2: recompute this device's combinational outputs from live
    /// `nets` and its (possibly just-committed) register banks, emitting
    /// change events. Does not redo edge detection or register writes.
    ///
    /// Default: forwards to [`Device::eval_discrete`] — correct for any
    /// device that never overrides `digital_seq_phase` (pure combinational
    /// devices, test mocks, simple external models), since there is then no
    /// separate register phase to race against.
    fn digital_comb_phase(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: &[f64],
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.eval_discrete(t, nets, analog_voltages, event_queue);
    }
}
