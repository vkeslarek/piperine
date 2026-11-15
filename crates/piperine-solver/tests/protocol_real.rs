//! The transient loop of a circuit of devices through the protocol over the
//! real sub-solvers (SAI-02, SAI-03, SAI-04; abi.md §5): a clock nothing
//! analog reads adds no step; a boundary change is solved on the left value
//! at its landing and delivered after the instant, restarting the next step;
//! a rejected step leaves no trace and replays its instants; a design with no
//! continuous pin runs its instants with no solve; a commit due after the stop
//! time stays pending; `SETTLE` fires at the stop time.

// The targets use the libraries selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::Cell as StdCell;
use std::sync::{Arc, OnceLock};

use abi_devices::{add_capacitor, add_resistor, add_vsrc, node_id, vsine};
use abi_mixed::{count_of, counter, dac, reactions, settle_time, settle_times, timer};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::HIGH;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_digital_solver::abi::tick::Tick;
use piperine_solver::abi::{AnalogReference, Bound, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions, TransientAnalysisResult};

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

fn bytes_of(bound: &Bound, name: &str) -> Vec<u8> {
    bound.digital.read(bound.names.nets[&format!("net:{name}")]).to_vec()
}

fn run(circuit: &mut CircuitInstance, stop: f64, dt: f64) -> TransientAnalysisResult {
    circuit
        .transient(TransientAnalysisOptions::new(stop, dt), Context::default())
        .expect("tran")
        .solve()
        .expect("solve")
}

/// A 1 V source charging 1 µF through 1 kΩ (τ = 1 ms).
fn rc(b: &mut CircuitBuilder) -> AnalogReference {
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    add_vsrc(b, "v1", 1.0, &top, &gnd);
    add_resistor(b, "r1", 1.0e3, &top, &out);
    add_capacitor(b, "c1", 1.0e-6, &out, &gnd);
    out
}

#[test]
fn a_clock_nothing_analog_reads_adds_no_step() {
    let plain = {
        let mut b = CircuitBuilder::new("rc");
        rc(&mut b);
        run(&mut b.build().expect("build"), 1.0e-3, 1.0e-5)
    };
    let clocked = {
        let mut b = CircuitBuilder::new("rc + clock");
        rc(&mut b);
        b.device(timer(), "clk", &[("half", ParamValue::Real(1.0e-5))], &[net("clk")]);
        b.device(counter(), "cnt", &[], &[net("clk"), net("n")]);
        let mut circuit = b.build().expect("build");
        let result = run(&mut circuit, 1.0e-3, 1.0e-5);
        // The clock did run: 50 rising edges in 1 ms of 10 µs half periods.
        assert_eq!(count_of(&bytes_of(circuit.bound().unwrap(), "n")), 50);
        result
    };
    assert_eq!(clocked.stats.steps_accepted, plain.stats.steps_accepted);
    assert_eq!(clocked.len(), plain.len());
    for (a, b) in clocked.iter().zip(plain.iter()) {
        assert_eq!(a.time(), b.time(), "the clock moved an accepted time");
    }
}

#[test]
fn a_boundary_change_is_read_by_the_arriving_step_and_restarts_the_next() {
    // The timer drives the DAC's code HIGH at 1 µs; the DAC drives `o`, which
    // charges `c` through 1 kΩ into 1 nF. The user's order (2026-11-15): the step
    // arriving at 1 µs is solved on the value before the instant (code LOW), the
    // instant runs, and the change reaches the analog side for the next step.
    let edge = 1.0e-6;
    let mut b = CircuitBuilder::new("timer -> dac");
    let gnd = b.ground();
    let o = b.node("o");
    let c = b.node("c");
    b.device(timer(), "clk", &[("half", ParamValue::Real(edge))], &[net("code")]);
    b.device(dac(), "dac", &[("vref", ParamValue::Real(1.0))], &[net("code"), Port::Node(o.clone())]);
    add_resistor(&mut b, "r1", 1.0e3, &o, &c);
    add_capacitor(&mut b, "c1", 1.0e-9, &c, &gnd);
    let mut circuit = b.build().expect("build");
    let result = run(&mut circuit, 1.5e-6, 1.0e-8);

    // The last accepted time within a tick of the edge is the landing (the
    // controller's own grid may put a step a few ulps short of it before).
    let at = result.iter().rposition(|s| (s.time() - edge).abs() < 1.0e-15).expect("a step lands on the edge");
    let landing = result.get(at).unwrap();
    let next = result.get(at + 1).expect("a step after the edge");
    assert_eq!(landing.get_node(&node_id(&o)).unwrap(), 0.0, "the landing is solved on the left value");
    assert!((next.get_node(&node_id(&o)).unwrap() - 1.0).abs() < 1.0e-12, "the next step reads the new code");
    // The jump lies inside the step after the landing: that step is exempt from
    // the truncation test, so nothing is rejected and no step falls to the floor.
    assert_eq!(result.stats.steps_rejected, 0, "{:?}", result.stats);
    assert_eq!(result.stats.dt_min_floor_hits, 0);
    assert_eq!(bytes_of(circuit.bound().unwrap(), "code"), HIGH.to_vec());
}

// ---------------------------------------------------------------------------
// A conductance whose function faults once, the first time it is evaluated
// inside `FAULT_WINDOW`: one Newton failure at a step that is then retried.
// ---------------------------------------------------------------------------

thread_local! {
    static FAULTED: StdCell<bool> = const { StdCell::new(false) };
}
const FAULT_WINDOW: (f64, f64) = (3.0e-4, 4.0e-4);

/// pins: p=0, n=1 (PORT); fn 0: reads [p, n, TIME] writes [T0] partials (0<-0) (0<-1); g = 1 mS
struct FlakyG;
static FLAKY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAKY_MODULE: OnceLock<Module> = OnceLock::new();

fn flaky_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: "protocol_real.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
    s.function(FunctionSpec {
        name: "i",
        origin: "protocol_real.rs",
        reads: vec![pin(p), pin(n), Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for FlakyG {
    fn prototype() -> &'static Shape<'static> {
        FLAKY_SHAPE.get_or_init(flaky_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlakyG, Arc::clone(FLAKY_SHAPE.get_or_init(flaky_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let t = cx.f64(2);
        if t > FAULT_WINDOW.0 && t < FAULT_WINDOW.1 && !FAULTED.with(|f| f.replace(true)) {
            return Err(Fault::new(7, "once"));
        }
        let g = 1.0e-3;
        cx.set(0, g * cx.f64(0) - g * cx.f64(1));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A 1 V, 1 kHz sine through 1 kΩ into 1 µF beside the flaky 1 mS conductance
/// at `out`, with a 10 µs clock counted in `n`. `fault` arms the conductance.
fn flaky_circuit(fault: bool) -> (CircuitInstance, AnalogReference) {
    FAULTED.with(|f| f.set(!fault));
    let mut b = CircuitBuilder::new("rejections");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    b.device(vsine(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
        Port::Node(top.clone()),
        Port::Node(gnd.clone()),
    ]);
    add_resistor(&mut b, "r1", 1.0e3, &top, &out);
    add_capacitor(&mut b, "c1", 1.0e-6, &out, &gnd);
    b.device(FLAKY_MODULE.get_or_init(|| module::<FlakyG>("FlakyG")), "g1", &[], &[Port::Node(out.clone()), Port::Node(gnd)]);
    b.device(timer(), "clk", &[("half", ParamValue::Real(1.0e-5))], &[net("clk")]);
    b.device(counter(), "cnt", &[], &[net("clk"), net("n")]);
    (b.build().expect("build"), out)
}

#[test]
fn a_rejected_step_leaves_no_trace() {
    // A first step of a whole sine period is rejected by the truncation test; the
    // conductance faults once between 0.3 and 0.4 ms, a Newton failure (the first
    // attempt evaluates at 0.59 and 1 ms, outside it). A 10 µs clock runs ahead of
    // every attempt; what a rejection undoes is replayed.
    let (mut circuit, out) = flaky_circuit(true);
    let result = run(&mut circuit, 1.0e-3, 1.0e-3);
    assert!(FAULTED.with(|f| f.get()), "the Newton failure happened");
    // The store holds every edge once, although the replays ran reactions again.
    assert_eq!(count_of(&bytes_of(circuit.bound().unwrap(), "n")), 50, "the discrete store left no trace");
    assert!(reactions("counter") > 50, "the undone instants were replayed ({})", reactions("counter"));
    // The term histories were restored: the run reads what a run with no fault reads.
    let (mut clean, _) = flaky_circuit(false);
    let reference = run(&mut clean, 1.0e-3, 1.0e-3);
    let (v, v_ref) = (result.last().unwrap().get_node(&node_id(&out)).unwrap(), reference.last().unwrap().get_node(&node_id(&out)).unwrap());
    assert!((v - v_ref).abs() < 1.0e-4, "v(out) = {v} vs {v_ref} without the fault");
    assert!(reference.stats.steps_rejected >= 1, "the truncation test rejects: {:?}", reference.stats);
    assert!(result.stats.steps_rejected > reference.stats.steps_rejected, "the Newton failure rejects too: {:?}", result.stats);
}

#[test]
fn a_design_without_analog_runs_every_instant_with_no_solve() {
    let mut b = CircuitBuilder::new("clock only");
    b.device(timer(), "clk", &[("half", ParamValue::Real(1.0e-6))], &[net("clk")]);
    b.device(counter(), "cnt", &[], &[net("clk"), net("n")]);
    let mut circuit = b.build().expect("build");
    let result = run(&mut circuit, 1.0e-5, 1.0e-7);
    // Rising edges at 1, 3, 5, 7, 9 µs; the edge at 10 µs (falling) is the stop time.
    assert_eq!(count_of(&bytes_of(circuit.bound().unwrap(), "n")), 5);
    assert_eq!(result.stats.newton_iterations, 0, "no solve");
    assert_eq!(result.stats.steps_accepted, 0);
}

#[test]
fn a_commit_due_after_the_stop_time_stays_pending() {
    let mut b = CircuitBuilder::new("rc + slow clock");
    rc(&mut b);
    b.device(timer(), "clk", &[("half", ParamValue::Real(3.0e-4))], &[net("clk")]);
    let mut circuit = b.build().expect("build");
    run(&mut circuit, 5.0e-4, 1.0e-5);
    let bound = circuit.bound().unwrap();
    assert_eq!(bytes_of(bound, "clk"), HIGH.to_vec(), "the edge at 0.3 ms applied");
    let spt = bound.digital.plan().resolution.seconds_per_tick;
    assert_eq!(bound.digital.next_due(), Some(Tick((6.0e-4 / spt).round() as i64)), "the edge at 0.6 ms waits");
}

#[test]
fn settle_fires_at_the_stop_time() {
    let mut b = CircuitBuilder::new("rc + settle");
    rc(&mut b);
    b.device(settle_time(), "st", &[], &[]);
    let mut circuit = b.build().expect("build");
    let stop = 1.0e-3;
    let result = run(&mut circuit, stop, 1.0e-5);
    let last = *settle_times().last().expect("SETTLE fired");
    assert!((last - stop).abs() < 1.0e-15, "the last SETTLE is at the stop time: {last}");
    assert!((result.last().unwrap().time() - stop).abs() < 1.0e-15);
}
