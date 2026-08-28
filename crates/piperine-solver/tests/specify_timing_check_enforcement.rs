//! T21 (LRMW3-07, spec P2 AC2): `specify` timing-check enforcement through
//! the findings channel — `CircuitInstance::consume_timing_check_violations`,
//! the exact call `analyses::transient`'s `accept_timestep` makes. Built at
//! the solver level (no codegen involved — `TimingCheckTable` is populated
//! directly, mirroring `piperine-codegen`'s own T20 population code) with
//! hand-scripted `Element`s driving `clk`/`d`, the same fixture shape
//! `digital_settle_subscriber.rs` already uses for accept-hook-driven
//! digital tests.

use piperine_solver::abi::{
    AnalogDevice, DigitalDevice, DigitalNet, DigitalPorts, Element, ElementCapabilities, EventSink, Introspect,
    LogicValue, Netlist, TimingCheckKind, TimingCheckLimit,
};
use piperine_solver::abi::SolverDomain;
use piperine_solver::prelude::{CheckPosture, CircuitInstance, ValidationSeverity};

/// Fires a scripted `(time, value)` sequence onto one net through the
/// ordinary accept-hook `EventSink` path — each entry fires (in order)
/// once `t` reaches its time.
struct ScriptedDriver {
    net: DigitalNet,
    script: Vec<(f64, LogicValue)>,
    next: usize,
}
impl AnalogDevice for ScriptedDriver {}
impl DigitalDevice for ScriptedDriver {}
impl Introspect for ScriptedDriver {}
impl Element for ScriptedDriver {
    fn name(&self) -> &str {
        "scripted_driver"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
    fn accept_timestep(
        &mut self,
        _state: &piperine_solver::abi::CircularArrayBuffer2<f64>,
        t: f64,
        _nets: &[LogicValue],
        sink: &mut dyn EventSink,
    ) {
        while self.next < self.script.len() && self.script[self.next].0 <= t {
            sink.emit(self.net, self.script[self.next].1, 0.0);
            self.next += 1;
        }
    }
}
// A no-op digital consumer so `clk`/`d` are genuine digital nets in a
// circuit with `ElementCapabilities::DIGITAL` present (the constraint/
// timing-check consume gates skip everything when no element declares
// DIGITAL at all).
struct Sink0;
impl AnalogDevice for Sink0 {}
impl DigitalDevice for Sink0 {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: &[] }
    }
}
impl Introspect for Sink0 {}
impl Element for Sink0 {
    fn name(&self) -> &str {
        "sink0"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

const CLK: DigitalNet = DigitalNet(0);
const D: DigitalNet = DigitalNet(1);

/// A clean run: `d` settles 5ns before `clk`'s rising edge — well past a
/// 2ns setup limit.
fn compliant_circuit() -> CircuitInstance {
    let devices: Vec<Box<dyn Element>> = vec![
        Box::new(ScriptedDriver { net: D, script: vec![(0.0, LogicValue::Zero)], next: 0 }),
        Box::new(ScriptedDriver { net: CLK, script: vec![(0.0, LogicValue::Zero)], next: 0 }),
        Box::new(Sink0),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("compliant", devices, Netlist::new());
    circuit.digital_state = piperine_solver::abi::DigitalState::new(2);
    circuit.digital_state.timing_checks.set(
        D,
        TimingCheckLimit { kind: TimingCheckKind::Setup, limit: 2e-9, paired_net: Some(CLK) },
    );
    circuit
}

#[test]
fn a_setup_violation_is_reported_naming_the_check_the_nets_the_separation_and_the_limit() {
    let mut circuit = compliant_circuit();
    // t=0: settle the initial Zero/Zero state.
    circuit.accept_and_run_digital(&[], 0.0).expect("settles");
    circuit
        .consume_timing_check_violations(CheckPosture::Collect, SolverDomain::Transient, 0.0, &mut Vec::new())
        .expect("no violation at the initial settle");

    // t=10ns: d changes.
    circuit.devices[0] =
        Box::new(ScriptedDriver { net: D, script: vec![(10e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10e-9).expect("settles");
    let mut sink = Vec::new();
    circuit
        .consume_timing_check_violations(CheckPosture::Collect, SolverDomain::Transient, 10e-9, &mut sink)
        .expect("d's own change never triggers Setup (only the reference edge does)");
    assert!(sink.is_empty(), "Setup triggers on the reference (clk) edge, not the data (d) edge");

    // t=10.5ns: clk rises only 0.5ns after d changed — violates the 2ns
    // setup limit.
    circuit.devices[1] =
        Box::new(ScriptedDriver { net: CLK, script: vec![(10.5e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10.5e-9).expect("settles");
    let mut sink = Vec::new();
    circuit
        .consume_timing_check_violations(CheckPosture::Collect, SolverDomain::Transient, 10.5e-9, &mut sink)
        .expect("Collect posture never aborts");
    assert_eq!(sink.len(), 1, "exactly one violation reported");
    let finding = &sink[0];
    assert_eq!(finding.severity, ValidationSeverity::Error);
    assert!(finding.label.contains("Setup"), "names the check kind: {}", finding.label);
    // D = DigitalNet(1), CLK = DigitalNet(0); no `set_label` calls here,
    // so they read back as the anonymous `d1`/`d0` form — both must be
    // named in the message (the data net and the reference net).
    assert!(finding.message.contains("d1"), "names the data net: {}", finding.message);
    assert!(finding.message.contains("d0"), "names the reference net: {}", finding.message);
    assert!((finding.value - 0.5e-9).abs() < 1e-15, "reports the observed separation (0.5ns): {}", finding.value);
    // The limit is embedded verbatim (no arithmetic performed on it), so
    // its `{:e}` rendering is deterministic — unlike the computed
    // separation, whose own value is already pinned above by `finding.value`.
    assert!(finding.message.contains(&format!("{:e}", 2e-9_f64)), "message states the limit: {}", finding.message);
}

#[test]
fn a_compliant_run_reports_nothing() {
    let mut circuit = compliant_circuit();
    circuit.accept_and_run_digital(&[], 0.0).expect("settles");

    circuit.devices[0] = Box::new(ScriptedDriver { net: D, script: vec![(0.0, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 0.0).expect("settles");

    // clk rises 5ns after d changed — comfortably past the 2ns limit.
    circuit.devices[1] = Box::new(ScriptedDriver { net: CLK, script: vec![(5e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 5e-9).expect("settles");

    let mut sink = Vec::new();
    circuit
        .consume_timing_check_violations(CheckPosture::Collect, SolverDomain::Transient, 5e-9, &mut sink)
        .expect("no violation");
    assert!(sink.is_empty(), "a compliant separation reports nothing: {sink:?}");
}

#[test]
fn checks_off_evaluates_nothing() {
    let mut circuit = compliant_circuit();
    circuit.accept_and_run_digital(&[], 0.0).expect("settles");
    circuit.devices[0] = Box::new(ScriptedDriver { net: D, script: vec![(10e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10e-9).expect("settles");
    circuit.devices[1] = Box::new(ScriptedDriver { net: CLK, script: vec![(10.5e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10.5e-9).expect("settles");

    // The exact same violating scenario as the first test, but under
    // `Off` — a behavior test (the sink stays empty), not code inspection.
    let mut sink = Vec::new();
    circuit
        .consume_timing_check_violations(CheckPosture::Off, SolverDomain::Transient, 10.5e-9, &mut sink)
        .expect("Off never errors");
    assert!(sink.is_empty(), "checks=off must evaluate nothing, even for a genuinely violating scenario");
}

#[test]
fn strict_posture_fails_loud_on_the_violation() {
    let mut circuit = compliant_circuit();
    circuit.accept_and_run_digital(&[], 0.0).expect("settles");
    circuit.devices[0] = Box::new(ScriptedDriver { net: D, script: vec![(10e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10e-9).expect("settles");
    circuit.devices[1] = Box::new(ScriptedDriver { net: CLK, script: vec![(10.5e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10.5e-9).expect("settles");

    let mut sink = Vec::new();
    let err = circuit
        .consume_timing_check_violations(CheckPosture::Strict, SolverDomain::Transient, 10.5e-9, &mut sink)
        .expect_err("strict posture aborts on the first Error finding");
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("setup") || msg.contains("timing check"), "{msg}");
}

/// Enforcement never fires inside a rejected step: a checkpoint/rollback
/// (the exact mechanism `analyses::transient`'s LTE-reject path uses)
/// erases the speculative `last_change` entries a would-be-violating
/// attempt recorded — re-checking at that same point afterward reports
/// nothing, because the reference net's "just changed at this instant"
/// fact no longer holds.
#[test]
fn enforcement_never_fires_inside_a_rejected_step() {
    let mut circuit = compliant_circuit();
    circuit.accept_and_run_digital(&[], 0.0).expect("settles");
    circuit.devices[0] = Box::new(ScriptedDriver { net: D, script: vec![(10e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10e-9).expect("settles");

    // Checkpoint before the would-be-violating attempt (mirrors the
    // transient driver's own predict/attempt cycle).
    circuit.digital_state.checkpoint();
    circuit.devices[1] = Box::new(ScriptedDriver { net: CLK, script: vec![(10.5e-9, LogicValue::One)], next: 0 });
    circuit.accept_and_run_digital(&[], 10.5e-9).expect("settles");

    // Reject: roll back instead of committing.
    circuit.digital_state.rollback();

    let mut sink = Vec::new();
    circuit
        .consume_timing_check_violations(CheckPosture::Collect, SolverDomain::Transient, 10.5e-9, &mut sink)
        .expect("no violation after rollback");
    assert!(sink.is_empty(), "a rolled-back (rejected) attempt's edge must not be seen as having 'just happened'");
}
