//! Live parameter mutation on a compiled circuit of devices: loud addressing
//! errors, idle sets applying to the next analysis run, and scheduled sets
//! landing mid-transient (LIVE-03/04/05/08), on the `Device` trait. A write
//! is a host commit through the parameter cone; the observable effect is the
//! operating point or the waveform, and a param's value reads from its net.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::sync::{Arc, OnceLock};

use abi_devices::{add_resistor, add_vsrc, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port, Value};
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, Context, DcAnalysisResult, TransientAnalysisOptions,
};

/// 10 V source over r1 (top→mid) and r2 (mid→gnd): v(mid) = 10·r2/(r1+r2).
fn divider(r1: f64, r2: f64) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 10.0, &top, &gnd);
    add_resistor(&mut b, "r1", r1, &top, &mid);
    add_resistor(&mut b, "r2", r2, &mid, &gnd);
    (b.build().expect("build"), mid)
}

fn v_mid(result: &DcAnalysisResult, mid: &AnalogReference) -> f64 {
    result.get_node(&node_id(mid)).expect("v(mid)")
}

/// The value a `PARAM` pin reads now.
fn param(circuit: &CircuitInstance, path: &str, name: &str) -> f64 {
    let bound = circuit.bound();
    let net = bound.names.params[&(path.to_string(), name.to_string())];
    f64::from_le_bytes(bound.digital.read(net)[..8].try_into().expect("an F64 param"))
}

// ── LIVE-04: unknown path fails loud with the path ──────────────────────────

#[test]
fn unknown_label_fails_loud_with_the_path() {
    let (mut circuit, _) = divider(1000.0, 1000.0);
    let err = circuit.set("nope", "r", ParamValue::Real(1.0)).expect_err("unknown label must fail");
    assert!(err.to_string().contains("nope"), "error names the path: {err}");
}

// ── LIVE-03: unknown param fails loud, listing the element's params ─────────

#[test]
fn unknown_param_fails_loud_and_lists_available_params() {
    let (mut circuit, _) = divider(1000.0, 1000.0);
    let err = circuit.set("r1", "bogus", ParamValue::Real(1.0)).expect_err("unknown param must fail");
    let msg = err.to_string();
    assert!(msg.contains("r1"), "error names the element: {msg}");
    assert!(msg.contains("bogus"), "error names the bad param: {msg}");
    assert!(msg.contains("available parameters"), "error lists candidates: {msg}");
    assert!(msg.contains("`r`") || msg.contains(": r"), "candidate list holds `r`: {msg}");
}

/// A device whose shape declares no param: one port and nothing else.
struct Mute;
static MUTE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static MUTE: OnceLock<Module> = OnceLock::new();

fn mute_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec::continuous("p", role::PORT));
    s.freeze()
}

impl Device for Mute {
    fn prototype() -> &'static Shape<'static> {
        MUTE_SHAPE.get_or_init(mute_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Mute, Arc::clone(MUTE_SHAPE.get_or_init(mute_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn unknown_param_on_paramless_element_says_so() {
    let mut b = CircuitBuilder::new("mute");
    let gnd = b.ground();
    b.device(MUTE.get_or_init(|| module::<Mute>("Mute")), "mute", &[], &[Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");
    let err = circuit.set("mute", "x", ParamValue::Real(1.0)).expect_err("element with no params must reject");
    assert!(err.to_string().contains("declares no writable parameters"), "empty-list case is explicit: {err}");
}

// ── LIVE-08: an idle set applies to the next analysis run ───────────────────

#[test]
fn idle_set_applies_to_the_next_analysis_run() {
    let (mut circuit, mid) = divider(1000.0, 1000.0);
    let first = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!((v_mid(&first, &mid) - 5.0).abs() < 1e-9, "10·1k/2k = 5 V");

    // No analysis running — the set simply lands on the element and the
    // next run picks it up.
    circuit.set("r2", "r", ParamValue::Real(3000.0)).expect("live set on idle circuit");

    let second = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!(
        (v_mid(&second, &mid) - 7.5).abs() < 1e-9,
        "10·3k/4k = 7.5 V after the set, got {}",
        v_mid(&second, &mid)
    );

    // A source-value set and the next run reflects it.
    circuit.set("v1", "dc", ParamValue::Real(20.0)).expect("source set");
    let third = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!((v_mid(&third, &mid) - 15.0).abs() < 1e-9, "20·3k/4k = 15 V, got {}", v_mid(&third, &mid));
}

// ── LIVE-06/09: scheduled sets land on breakpoints mid-transient ─────────────

#[test]
fn scheduled_op_strength_set_resolves_consistently_at_the_breakpoint() {
    let (mut circuit, mid) = divider(1000.0, 1000.0);
    let opts = TransientAnalysisOptions::new(10e-6, 0.5e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();

    // Source value 10 → 20 V at t = 5 µs. The landing is solved on the left
    // value (the user's order for a boundary change), then the write is
    // delivered: every point after it reflects the new source.
    tran.schedule_set(5e-6, "v1", "dc", Value::Real(20.0));
    let result = tran.solve().unwrap();

    let landing = result
        .iter()
        .find(|s| (s.time() - 5e-6).abs() < 1e-18)
        .expect("a step lands exactly on the set time");
    let v_at = landing.get_node(&node_id(&mid)).unwrap();
    assert!(
        (v_at - 5.0).abs() < 1e-9,
        "the landing is solved on the left value: v(mid) at 5 µs is {v_at}, want 5 (pre-set)"
    );

    for step in result.iter() {
        let v = step.get_node(&node_id(&mid)).unwrap();
        if step.time() < 5e-6 + 1e-18 {
            assert!((v - 5.0).abs() < 1e-9, "pre-set point at {}: {v}", step.time());
        } else {
            assert!((v - 10.0).abs() < 1e-9, "post-set point at {}: {v}", step.time());
        }
    }
}

#[test]
fn same_param_sets_at_one_time_apply_last_write_wins_with_one_breakpoint() {
    let (mut circuit, mid) = divider(1000.0, 1000.0);
    let opts = TransientAnalysisOptions::new(10e-6, 0.5e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();

    // Two writes to the same param at the same time: application order is
    // scheduling order, so the later call wins — one landing at 5 µs.
    tran.schedule_set(5e-6, "r2", "r", Value::Real(9000.0));
    tran.schedule_set(5e-6, "r2", "r", Value::Real(3000.0));
    let result = tran.solve().unwrap();

    let landings: Vec<f64> =
        result.iter().map(|s| s.time()).filter(|t| (t - 5e-6).abs() < 1e-18).collect();
    assert_eq!(landings.len(), 1, "exactly one recorded landing at the set time");

    // The value applies from the next accepted step.
    let last = result.last().unwrap();
    let v_end = last.get_node(&node_id(&mid)).unwrap();
    assert!(
        (v_end - 7.5).abs() < 1e-9,
        "last write wins: 10·3k/4k = 7.5 V, got {v_end} (9 V would mean first write won)"
    );

    drop(result);
    assert_eq!(param(&circuit, "r2", "r"), 3000.0);
}

#[test]
fn scheduled_set_at_or_before_zero_applies_to_the_whole_run() {
    let (mut circuit, mid) = divider(1000.0, 1000.0);
    let opts = TransientAnalysisOptions::new(2e-6, 0.5e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    tran.schedule_set(0.0, "r2", "r", Value::Real(3000.0));
    let result = tran.solve().unwrap();
    for step in result.iter() {
        let v = step.get_node(&node_id(&mid)).unwrap();
        assert!((v - 7.5).abs() < 1e-9, "t={}: {v}", step.time());
    }
}

#[test]
fn scheduled_set_with_bad_addressing_fails_the_run_loud() {
    let (mut circuit, _) = divider(1000.0, 1000.0);
    let opts = TransientAnalysisOptions::new(2e-6, 0.5e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    tran.schedule_set(1e-6, "r2", "bogus", Value::Real(1.0));
    let err = tran.solve().expect_err("bad scheduled set must fail the run");
    assert!(err.to_string().contains("bogus"), "{err}");
}

// ── T9 (host-library HOST-07..12): the labeled device's parameters ──────────

#[test]
fn device_introspect_reaches_the_labeled_devices_introspect_surface() {
    let (mut circuit, _) = divider(1000.0, 1000.0);
    // The parameter nets hold their values once an analysis has started.
    circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert_eq!(param(&circuit, "r1", "r"), 1000.0);
    assert_eq!(param(&circuit, "v1", "dc"), 10.0);
    let bound = circuit.bound();
    let r1_params: Vec<_> = bound.names.params.keys().filter(|(p, _)| p == "r1").collect();
    assert_eq!(r1_params.len(), 1);
    assert_eq!(r1_params[0].1, "r");
}

#[test]
fn device_introspect_returns_none_for_an_unknown_label() {
    let (circuit, _) = divider(1000.0, 1000.0);
    assert!(!circuit.bound().names.params.keys().any(|(p, _)| p == "nope"));
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Mute", MUTE.get_or_init(|| module::<Mute>("Mute")));
}
