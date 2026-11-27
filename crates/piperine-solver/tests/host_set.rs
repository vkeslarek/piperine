//! Host writes to device params (SAI-08; abi.md §10 "Host actions", P5, P6):
//! a false `check` refuses a `set`; a `RESHAPE` is refused inside a transient
//! and re-creates and re-binds the instance between analyses; a scheduled
//! write is a landing followed by a step of 1e-3 of the last; a `STRUCTURAL`
//! param cannot change during a run.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::sync::{Arc, OnceLock};

use abi_devices::{add_capacitor, add_resistor, add_vsrc, node_id, resistor_opt};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port, Value};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions};

const ORIGIN: &str = "host_set.rs";

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}
fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}
fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}
fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
}
const KILOHM: [u8; 8] = 1.0e3_f64.to_le_bytes();

// ---------------------------------------------------------------------------
// CheckedR: a resistor whose `r` has `where r > 0`
// ---------------------------------------------------------------------------

/// pins: r=0 (PARAM F64, default 1 kΩ, check -> ok); p=1, n=2 (PORT); ok=3 (PRIVATE BIT)
/// terms: T0 FLOW{p, n}
/// fn 0 "where": reads [r] writes [ok]; ok = r > 0
/// fn 1 "i": reads [p, n, r] writes [T0] partials (0<-0) (0<-1); g = 1/r
struct CheckedR;
static CHECKED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CHECKED: OnceLock<Module> = OnceLock::new();

fn checked_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec {
        flags: pin_flag::HAS_INIT | pin_flag::HAS_CHECK,
        init: &KILOHM,
        check: 3,
        ..PinSpec::discrete("r", role::PARAM, &F64)
    });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let ok = s.pin(PinSpec::discrete("ok", role::PRIVATE, &BIT));
    let t0 = s.term(flow("i", p, n));
    s.function(FunctionSpec { name: "where", origin: ORIGIN, reads: vec![pin(0)], writes: vec![pin(ok)], ..FunctionSpec::default() });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for CheckedR {
    fn prototype() -> &'static Shape<'static> {
        CHECKED_SHAPE.get_or_init(checked_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CheckedR, Arc::clone(CHECKED_SHAPE.get_or_init(checked_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let ok = if cx.f64(0) > 0.0 { HIGH } else { LOW };
            cx.out_bytes(0).copy_from_slice(&ok);
            return Ok(());
        }
        let g = 1.0 / cx.f64(2);
        cx.set(0, g * cx.f64(0) - g * cx.f64(1));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ---------------------------------------------------------------------------
// Collapsible: a resistor with an internal node above a threshold
// ---------------------------------------------------------------------------

const THRESHOLD: f64 = 100.0;

/// At `r >= THRESHOLD`: two halves `r/2` in series through a private node `m`;
/// below it the node collapses: one resistor `r`. Each shape's setup function
/// asks for the other shape when `r` crosses the threshold (`RESHAPE`).
///
/// split pins: r=0 (PARAM F64, default 1 kΩ); p=1, n=2 (PORT); big=3 (PRIVATE BIT); m=4 (PRIVATE)
///   terms T0 FLOW{p, m}, T1 FLOW{m, n}; fn 0 "setup": reads [r] writes [big];
///   fn 1 "i": reads [p, m, n, r] writes [T0, T1] partials (0<-0) (0<-1) (1<-1) (1<-2)
/// whole pins: r=0; p=1, n=2; big=3; terms T0 FLOW{p, n}; fn 0 "setup"; fn 1 "i": reads [p, n, r]
struct Collapsible {
    split: bool,
}
static SPLIT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static WHOLE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COLLAPSIBLE: OnceLock<Module> = OnceLock::new();

fn collapsible_shape(split: bool) -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &KILOHM, ..PinSpec::discrete("r", role::PARAM, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let big = s.pin(PinSpec::discrete("big", role::PRIVATE, &BIT));
    s.function(FunctionSpec { name: "setup", origin: ORIGIN, reads: vec![pin(0)], writes: vec![pin(big)], ..FunctionSpec::default() });
    if split {
        let m = s.pin(PinSpec::continuous("m", role::PRIVATE));
        let t0 = s.term(flow("a", p, m));
        let t1 = s.term(flow("b", m, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![pin(p), pin(m), pin(n), pin(0)],
            writes: vec![term(t0), term(t1)],
            partials: vec![partial(0, 0), partial(0, 1), partial(1, 1), partial(1, 2)],
            ..FunctionSpec::default()
        });
    } else {
        let t0 = s.term(flow("a", p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![pin(p), pin(n), pin(0)],
            writes: vec![term(t0)],
            partials: vec![partial(0, 0), partial(0, 1)],
            ..FunctionSpec::default()
        });
    }
    s.freeze()
}

fn split_shape() -> &'static ShapeRef {
    SPLIT_SHAPE.get_or_init(|| collapsible_shape(true))
}
fn whole_shape() -> &'static ShapeRef {
    WHOLE_SHAPE.get_or_init(|| collapsible_shape(false))
}

impl Device for Collapsible {
    fn prototype() -> &'static Shape<'static> {
        split_shape().shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let split = values.f64(0).unwrap_or(1.0e3) >= THRESHOLD;
        let shape = if split { split_shape() } else { whole_shape() };
        Ok((Collapsible { split }, Arc::clone(shape)))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            if (cx.f64(0) >= THRESHOLD) != self.split {
                return Err(Fault::reshape("r crossed the threshold"));
            }
            cx.out_bytes(0).copy_from_slice(&HIGH);
            return Ok(());
        }
        if self.split {
            let g = 2.0 / cx.f64(3);
            cx.set(0, g * cx.f64(0) - g * cx.f64(1));
            cx.set(1, g * cx.f64(1) - g * cx.f64(2));
            cx.jac().copy_from_slice(&[g, -g, g, -g]);
        } else {
            let g = 1.0 / cx.f64(2);
            cx.set(0, g * cx.f64(0) - g * cx.f64(1));
            cx.jac().copy_from_slice(&[g, -g]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A 1 V source over `r1` (1 kΩ) into `x`, a device of `module`, to ground.
fn divider(module: &'static Module) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
    b.device(module, "x", &[], &[Port::Node(mid.clone()), Port::Node(gnd)]);
    (b.build().expect("build"), mid)
}

fn v_mid(circuit: &mut CircuitInstance, mid: &AnalogReference) -> f64 {
    circuit.dc(Context::default()).expect("dc").solve().expect("dc").get_node(&node_id(mid)).expect("v(mid)")
}

#[test]
fn a_false_check_refuses_a_set() {
    let (mut circuit, mid) = divider(CHECKED.get_or_init(|| module::<CheckedR>("CheckedR")));
    assert!((v_mid(&mut circuit, &mid) - 0.5).abs() < 1e-12);

    let err = circuit.set("x", "r", ParamValue::Real(-5.0)).expect_err("r > 0 is false").to_string();
    assert!(err.contains("x.ok"), "the check is named: {err}");
    assert!((v_mid(&mut circuit, &mid) - 0.5).abs() < 1e-12, "the next solve uses the old value");

    circuit.set("x", "r", ParamValue::Real(3.0e3)).expect("r > 0 holds");
    assert!((v_mid(&mut circuit, &mid) - 0.75).abs() < 1e-12, "an accepted write is read");
}

#[test]
fn reshape_is_refused_in_a_transient_and_rebinds_between_analyses() {
    let (mut circuit, mid) = divider(COLLAPSIBLE.get_or_init(|| module::<Collapsible>("Collapsible")));
    circuit.set("r1", "r", ParamValue::Real(3.0e3)).expect("a plain write");
    let unknowns = circuit.netlist().all_references().len();
    assert!((v_mid(&mut circuit, &mid) - 0.25).abs() < 1e-12);

    // Inside a transient the write is refused, and nothing changed.
    {
        let mut solver =
            circuit.transient(TransientAnalysisOptions::new(1.0e-3, 1.0e-5), Context::default()).expect("tran");
        solver.schedule_set(5.0e-4, "x", "r", Value::Real(50.0));
        let err = solver.solve().expect_err("RESHAPE inside a transient").to_string();
        assert!(err.contains("another shape"), "{err}");
    }
    assert_eq!(circuit.netlist().all_references().len(), unknowns);
    assert!((v_mid(&mut circuit, &mid) - 0.25).abs() < 1e-12);

    // Between analyses the instance is created and bound again: its private
    // node is gone, and every param net keeps its value (r1 stays 3 kΩ).
    circuit.set("x", "r", ParamValue::Real(50.0)).expect("re-created and re-bound");
    assert_eq!(circuit.netlist().all_references().len(), unknowns - 1, "the internal node collapsed");
    let expected = 50.0 / (3.0e3 + 50.0);
    assert!((v_mid(&mut circuit, &mid) - expected).abs() < 1e-12, "v(mid) = {}", v_mid(&mut circuit, &mid));
}

#[test]
fn a_scheduled_host_write_lands_and_restarts_small() {
    let at = 5.0e-4;
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", 1.0e3, &top, &out);
    add_capacitor(&mut b, "c1", 1.0e-6, &out, &gnd);
    let mut circuit = b.build().expect("build");
    let options = TransientAnalysisOptions::new(1.0e-3, 1.0e-5);
    let dt_min = options.dt_min;
    let mut solver = circuit.transient(options, Context::default()).expect("tran");
    solver.schedule_set(at, "v1", "dc", Value::Real(2.0));
    let result = solver.solve().expect("solve");

    let k = result.iter().rposition(|s| (s.time() - at).abs() < 1.0e-15).expect("the write's time is an accepted time");
    let t = |i: usize| result.get(i).unwrap().time();
    let v = |i: usize| result.get(i).unwrap().get_node(&node_id(&top)).unwrap();
    assert_eq!(v(k), 1.0, "the landing is solved before the write");
    assert_eq!(v(k + 1), 2.0, "the next step reads it");
    let (last, next) = (t(k) - t(k - 1), t(k + 1) - t(k));
    let small = (0.1 * last).max(dt_min);
    assert!((next - small).abs() <= 1e-9 * small, "the next step is 0.1 of the last: {next:e} vs {small:e}");
}

#[test]
fn a_structural_set_during_a_run_is_refused() {
    let mut b = CircuitBuilder::new("structural");
    let gnd = b.ground();
    let top = b.node("top");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    b.device(resistor_opt(), "x", &[], &[Port::Node(top), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");
    let w_net = circuit.bound().names.params[&("x".to_string(), "w".to_string())];
    {
        let mut solver =
            circuit.transient(TransientAnalysisOptions::new(1.0e-3, 1.0e-5), Context::default()).expect("tran");
        solver.schedule_set(5.0e-4, "x", "w", Value::Real(2.0e-3));
        let err = solver.solve().expect_err("a STRUCTURAL pin cannot change during a run").to_string();
        assert!(err.contains("STRUCTURAL"), "{err}");
    }
    let bound = circuit.bound();
    assert_eq!(bound.digital.read(w_net), 1.0e-3f64.to_le_bytes(), "nothing changed: `w` still holds its default");
    assert!(bound.host_writes.is_empty());
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("CheckedR", CHECKED.get_or_init(|| module::<CheckedR>("CheckedR")));
    // r (read 0 of "setup") on the side of the threshold the default instance was made for.
    partials::verify_every_function_with(
        "Collapsible",
        COLLAPSIBLE.get_or_init(|| module::<Collapsible>("Collapsible")),
        &[(0, 0, 1.0e3)],
    );
}
