//! The owner's retake of a step that read a jump (docs/spec §15.2 rule 4, AO-13): a crossing the
//! step made fires after the step, and when its reaction changes a carrier `analog` reads, the
//! step is taken again at the same time with the change and the retaken solution replaces the
//! first. The devices are hand-built: a `Gate` source whose value is a code plus one volt per
//! mode carrier, a `Setter` that sets the code on an event, and the comparators of the mixed
//! fixtures, whose crossings set the modes (the way a slew's own rate sets its mode).

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use abi_devices::{add_vsrc, node_id, vramp};
use abi_mixed::{comparator, delay, reactions};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{Observation, ObservationKind, Sink};

const ORIGIN: &str = "jump_retake.rs";
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();
/// The instant the code is set at, in the jump scenario.
const JUMP_AT: f64 = 1.0e-6;

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}

fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
}

fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}

/// A source of `vref * code + m1 + m2 + m3` volts: the three modes add a volt each.
/// pins: vref=0 (PARAM F64, 1); code=1, m1=2, m2=3, m3=4 (PORT BIT); out=5 (PORT); f=6 (PRIVATE)
/// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
struct Gate;
static GATE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GATE: OnceLock<Module> = OnceLock::new();

fn gate_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let vref = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("vref", role::PARAM, &F64) });
    let code = s.pin(PinSpec::discrete("code", role::PORT, &BIT));
    let modes: Vec<u32> = ["m1", "m2", "m3"].into_iter().map(|name| s.pin(PinSpec::discrete(name, role::PORT, &BIT))).collect();
    let out = s.pin(PinSpec::continuous("out", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", out, GROUND));
    let t1 = s.term(flow("eq", f, GROUND));
    s.function(FunctionSpec {
        name: "gate",
        origin: ORIGIN,
        reads: vec![pin(code), pin(modes[0]), pin(modes[1]), pin(modes[2]), pin(out), pin(f), pin(vref)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 5), partial(1, 4), partial(1, 6)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Gate {
    fn prototype() -> &'static Shape<'static> {
        GATE_SHAPE.get_or_init(gate_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gate, Arc::clone(GATE_SHAPE.get_or_init(gate_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let bit = |index: usize| cx.bytes(index)[0] as f64;
        let (code, modes) = (bit(0), bit(1) + bit(2) + bit(3));
        let (out, f, vref) = (cx.f64(4), cx.f64(5), cx.f64(6));
        cx.set(0, f);
        cx.set(1, out - (code * vref + modes));
        cx.jac().copy_from_slice(&[1.0, 1.0, -code]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn gate() -> &'static Module {
    GATE.get_or_init(|| module::<Gate>("Gate"))
}

/// Sets `out` HIGH when `go` fires.
/// pins: go=0 (PORT event); out=1 (PORT BIT, init LOW)   reaction 0: on [go] writes [S0 {out}]
struct Setter;
static SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTER: OnceLock<Module> = OnceLock::new();

fn setter_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    s.reaction(ReactionSpec {
        name: "set",
        origin: ORIGIN,
        on: vec![go],
        writes: vec![Statement { pin: out, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Setter {
    fn prototype() -> &'static Shape<'static> {
        SETTER_SHAPE.get_or_init(setter_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Setter, Arc::clone(SETTER_SHAPE.get_or_init(setter_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

fn setter() -> &'static Module {
    SETTER.get_or_init(|| module::<Setter>("Setter"))
}

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

/// What the comparators watch: the gate's own output, or a ramp of 1 V per millisecond.
#[derive(Clone, Copy)]
enum Watched {
    Output,
    Ramp,
}

/// The gate's output node and the circuit: the code is set `code_at` seconds after the start,
/// and one comparator per level sets the mode carrier `m<k>` when its watched node crosses it.
fn circuit(levels: &[f64], watched: Watched, code_at: f64) -> (CircuitBuilder, AnalogReference) {
    let mut b = CircuitBuilder::new("jump retake");
    let gnd = b.ground();
    let out = b.node("o");
    b.device(delay(), "kick", &[("at", ParamValue::Real(code_at))], &[net("go")]);
    b.device(setter(), "set", &[], &[net("go"), net("code")]);
    b.device(gate(), "gate", &[], &[net("code"), net("m1"), net("m2"), net("m3"), Port::Node(out.clone())]);
    let ramp = b.node("a");
    b.device(vramp(), "ramp", &[], &[Port::Node(ramp.clone()), Port::Node(gnd.clone())]);
    let watch = match watched {
        Watched::Output => out.clone(),
        Watched::Ramp => ramp,
    };
    for (k, level) in levels.iter().enumerate() {
        let reference = b.node(&format!("r{k}"));
        add_vsrc(&mut b, &format!("vr{k}"), *level, &reference, &gnd);
        let mode = format!("m{}", k + 1);
        b.device(comparator(), &format!("cmp{k}"), &[], &[Port::Node(watch.clone()), Port::Node(reference), net(&mode)]);
    }
    (b, out)
}

/// Every function of this target's own devices passes the partials verifier (the guard of
/// `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Gate", gate());
    partials::verify_every_function("Setter", setter());
}

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

/// The accepted `(time, value of o)` of a transient to `stop`, and the times the first
/// comparator's `up` crossing fired at.
fn run(levels: &[f64], watched: Watched, code_at: f64, stop: f64, context: Context) -> Result<(Vec<(f64, f64)>, Vec<f64>), String> {
    let (b, out) = circuit(levels, watched, code_at);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut solver = Solver::new(b.build().expect("build"))
        .with_context(context)
        .with_tran_opts(TransientAnalysisOptions::new(stop, stop / 50.0))
        .with_sink(Box::new(Collect(Rc::clone(&seen))))
        .build();
    let result = solver.tran().expect("tran").solve().map_err(|e| format!("{e}"))?;
    let steps = result.iter().map(|s| (s.time(), s.get_node(&node_id(&out)).expect("o"))).collect();
    let crossings = seen
        .borrow()
        .iter()
        .filter(|o| o.kind == ObservationKind::Crossing && o.path == "cmp0.up")
        .map(|o| o.time)
        .collect();
    Ok((steps, crossings))
}

#[test]
fn a_crossing_made_by_a_jump_retakes_the_step() {
    // AO-13: the code jumps at 1 us; the first step after it reads 1 V, which crosses the 0.5 V
    // comparator; its reaction sets m1; that step is retaken, so no accepted solution after the
    // jump holds the stale 1 V and the step the crossing fired in holds 2 V.
    let (steps, crossings) = run(&[0.5], Watched::Output, JUMP_AT, 3.0e-6, Context::default()).expect("tran");
    assert_eq!(crossings.len(), 1, "the crossing fires once: {crossings:?}");
    let after: Vec<&(f64, f64)> = steps.iter().filter(|(t, _)| *t > JUMP_AT + 1.0e-12).collect();
    let first = after.first().expect("a step after the jump");
    assert_eq!(first.0, crossings[0], "the crossing fired in the step that read the jump");
    for (time, value) in &after {
        assert!((value - 2.0).abs() < 1e-9, "o = {value} at {time:e}: a step after the jump holds the stale 1 V");
    }
    assert_eq!(steps.iter().filter(|(t, _)| *t == first.0).count(), 1, "the retaken solution replaces the first");
    assert_eq!(reactions("comparator"), 1, "the reaction that ran is not run again");
}

#[test]
fn a_step_that_reads_no_jump_never_retakes() {
    // AO-13 control: the code is set at the start, so the ramp's crossing of 0.5 V at 0.5 ms is
    // made by a step that reads no jump: its reaction's change waits for the next step (o is
    // 1 V in the crossing's step, 2 V after).
    let (steps, crossings) = run(&[0.5], Watched::Ramp, 0.0, 1.0e-3, Context::default()).expect("tran");
    assert_eq!(crossings.len(), 1, "{crossings:?}");
    let at = steps.iter().position(|(t, _)| *t == crossings[0]).expect("a step at the crossing");
    assert!((steps[at].1 - 1.0).abs() < 1e-9, "the crossing's step holds {} V, not 1 V", steps[at].1);
    assert!((steps[at + 1].1 - 2.0).abs() < 1e-9, "the next step reads m1: {} V", steps[at + 1].1);
}

#[test]
fn endless_retakes_fail_with_e0504() {
    // AO-13: each retaken solution makes the next comparator's crossing (levels 0.5, 1.5, 2.5 V
    // against 1, 2, 3, 4 V), so the step is retaken three times; a bound of 2 rounds fails as
    // `E0504` naming the instant, and the default bound lets it settle at 4 V (the control).
    let levels = [0.5, 1.5, 2.5];
    let context = Context { max_rounds: 2, ..Context::default() };
    let error = run(&levels, Watched::Output, JUMP_AT, 3.0e-6, context).expect_err("two rounds are not enough");
    assert!(error.contains("E0504"), "{error}");
    assert!(error.contains("more than 2 times"), "{error}");
    assert!(error.contains("t = 1"), "the instant is named: {error}");

    let (steps, _) = run(&levels, Watched::Output, JUMP_AT, 3.0e-6, Context::default()).expect("the default bound settles");
    let last = steps.last().expect("steps");
    assert!((last.1 - 4.0).abs() < 1e-9, "o = {} V", last.1);
}
