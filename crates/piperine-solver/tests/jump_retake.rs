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
use abi_mixed::{comparator, delay, reactions, settle_time, settle_times};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, cell, global, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{Observation, ObservationKind, Sink};

const ORIGIN: &str = "jump_retake.rs";
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();
/// The instant the code is set at, in the jump scenario.
const JUMP_AT: f64 = 1.0e-6;





/// Cells, terms and partials of the hand-built shapes of this target.
struct Shapes;

impl Shapes {
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
}

/// A source of `vref * code + m1 + m2 + m3` volts: the three modes add a volt each.
/// pins: vref=0 (PARAM F64, 1); code=1, m1=2, m2=3, m3=4 (PORT BIT); out=5 (PORT); f=6 (PRIVATE)
/// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
struct Gate;
static GATE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GATE: OnceLock<Module> = OnceLock::new();

impl Gate {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let vref = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("vref", role::PARAM, &F64) });
        let code = shape.pin(PinSpec::discrete("code", role::PORT, &BIT));
        let modes: Vec<u32> = ["m1", "m2", "m3"].into_iter().map(|name| shape.pin(PinSpec::discrete(name, role::PORT, &BIT))).collect();
        let out = shape.pin(PinSpec::continuous("out", role::PORT));
        let branch = shape.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = shape.term(Shapes::flow("i", out, GROUND));
        let t1 = shape.term(Shapes::flow("eq", branch, GROUND));
        shape.function(FunctionSpec {
            name: "gate",
            origin: ORIGIN,
            reads: vec![Shapes::pin(code), Shapes::pin(modes[0]), Shapes::pin(modes[1]), Shapes::pin(modes[2]), Shapes::pin(out), Shapes::pin(branch), Shapes::pin(vref)],
            writes: vec![Shapes::term(t0), Shapes::term(t1)],
            partials: vec![Shapes::partial(0, 5), Shapes::partial(1, 4), Shapes::partial(1, 6)],
            ..FunctionSpec::default()
        });
        shape.freeze()
    }

    fn module() -> &'static Module {
        GATE.get_or_init(|| module::<Gate>("Gate"))
    }
}


impl Device for Gate {
    fn prototype() -> &'static Shape<'static> {
        GATE_SHAPE.get_or_init(Gate::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gate, Arc::clone(GATE_SHAPE.get_or_init(Gate::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let bit = |index: usize| cx.bytes(index)[0] as f64;
        let (code, modes) = (bit(0), bit(1) + bit(2) + bit(3));
        let (out, branch, vref) = (cx.f64(4), cx.f64(5), cx.f64(6));
        cx.set(0, branch);
        cx.set(1, out - (code * vref + modes));
        cx.jac().copy_from_slice(&[1.0, 1.0, -code]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}


/// Sets `out` HIGH when `go` fires.
/// pins: go=0 (PORT event); out=1 (PORT BIT, init LOW)   reaction 0: on [go] writes [S0 {out}]
struct Setter;
static SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTER: OnceLock<Module> = OnceLock::new();

impl Setter {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let go = shape.pin(PinSpec::event("go", role::PORT));
        let out = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        shape.reaction(ReactionSpec {
            name: "set",
            origin: ORIGIN,
            on: vec![go],
            writes: vec![Statement { pin: out, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        shape.freeze()
    }

    fn module() -> &'static Module {
        SETTER.get_or_init(|| module::<Setter>("Setter"))
    }
}


impl Device for Setter {
    fn prototype() -> &'static Shape<'static> {
        SETTER_SHAPE.get_or_init(Setter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Setter, Arc::clone(SETTER_SHAPE.get_or_init(Setter::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}


/// Sets `out` HIGH in the settle phase of every time point: a settle commit (§15 S1), read by the
/// step that arrives at the next time point.
/// pins: settle=0 (GLOBAL SETTLE event); out=1 (PORT BIT, init LOW)   reaction 0: on [settle] writes [S0 {out}]
struct SettleSetter;
static SETTLE_SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_SETTER: OnceLock<Module> = OnceLock::new();

impl SettleSetter {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let settle = shape.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        let out = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        shape.reaction(ReactionSpec {
            name: "set",
            origin: ORIGIN,
            on: vec![settle],
            writes: vec![Statement { pin: out, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        shape.freeze()
    }

    fn module() -> &'static Module {
        SETTLE_SETTER.get_or_init(|| module::<SettleSetter>("SettleSetter"))
    }
}


impl Device for SettleSetter {
    fn prototype() -> &'static Shape<'static> {
        SETTLE_SETTER_SHAPE.get_or_init(SettleSetter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleSetter, Arc::clone(SETTLE_SETTER_SHAPE.get_or_init(SettleSetter::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}


/// Copies `src` to `dst` in the settle phase of every time point, as a settle commit: it writes
/// the final discrete state of the instant.
/// pins: settle=0 (GLOBAL SETTLE event); src=1 (PORT BIT); dst=2 (PORT BIT, init LOW)
/// reaction 0: on [settle] reads [src] writes [S0 {dst}]
struct SettleMirror;
static SETTLE_MIRROR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_MIRROR: OnceLock<Module> = OnceLock::new();

impl SettleMirror {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let settle = shape.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        let src = shape.pin(PinSpec::discrete("src", role::PORT, &BIT));
        let dst = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("dst", role::PORT, &BIT) });
        shape.reaction(ReactionSpec {
            name: "copy",
            origin: ORIGIN,
            on: vec![settle],
            reads: vec![Shapes::pin(src)],
            writes: vec![Statement { pin: dst, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        shape.freeze()
    }

    fn module() -> &'static Module {
        SETTLE_MIRROR.get_or_init(|| module::<SettleMirror>("SettleMirror"))
    }
}


impl Device for SettleMirror {
    fn prototype() -> &'static Shape<'static> {
        SETTLE_MIRROR_SHAPE.get_or_init(SettleMirror::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleMirror, Arc::clone(SETTLE_MIRROR_SHAPE.get_or_init(SettleMirror::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let value = [cx.bytes(0)[0], cx.bytes(0)[1]];
        cx.commit(0, &value, 0.0)?;
        Ok(Flow::Continue)
    }
}



/// The ports of the circuits of this target.
struct Nets;

impl Nets {
    /// The port of the net called `name`.
    fn port(name: &str) -> Port {
        Port::Net(name.to_string())
    }
}

/// What the comparators watch: the gate's own output, or a ramp of 1 V per millisecond.
#[derive(Clone, Copy)]
enum Watched {
    Output,
    Ramp,
}

/// What sets the gate's code.
#[derive(Clone, Copy)]
enum Trigger {
    /// A one-shot `delay` fires `at` seconds after the start and a setter sets the code on it.
    At(f64),
    /// A device on `settle` sets the code in the settle phase of every time point (a settle commit).
    Settle,
}


/// Every function of this target's own devices passes the partials verifier (the guard of
/// `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Gate", Gate::module());
    partials::verify_every_function("Setter", Setter::module());
    partials::verify_every_function("SettleSetter", SettleSetter::module());
    partials::verify_every_function("SettleMirror", SettleMirror::module());
}

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, observation: &Observation) {
        self.0.borrow_mut().push(observation.clone());
    }
}

/// A circuit of the gate, its comparators and its trigger, and the transient run of it.
struct Scenario;

impl Scenario {
    /// The gate's output node and the circuit: the code is set by `trigger`, and one comparator per
    /// level sets the mode carrier `m<k>` when its watched node crosses it. With `mirrored`, a
    /// device copies the first mode carrier to the third in the settle phase (a settle commit that
    /// reads the final state of the instant), and one `settle_time` device stamps every settle phase.
    fn circuit(levels: &[f64], watched: Watched, trigger: Trigger, mirrored: bool) -> (CircuitBuilder, AnalogReference) {
        let mut builder = CircuitBuilder::new("jump retake");
        let gnd = builder.ground();
        let out = builder.node("o");
        match trigger {
            Trigger::At(code_at) => {
                builder.device(delay(), "kick", &[("at", ParamValue::Real(code_at))], &[Nets::port("go")]);
                builder.device(Setter::module(), "set", &[], &[Nets::port("go"), Nets::port("code")]);
            }
            Trigger::Settle => {
                builder.device(SettleSetter::module(), "set", &[], &[Nets::port("code")]);
            }
        }
        if mirrored {
            builder.device(SettleMirror::module(), "mirror", &[], &[Nets::port("m2"), Nets::port("m3")]);
            builder.device(settle_time(), "stamp", &[], &[]);
        }
        builder.device(Gate::module(), "gate", &[], &[Nets::port("code"), Nets::port("m1"), Nets::port("m2"), Nets::port("m3"), Port::Node(out.clone())]);
        let ramp = builder.node("a");
        builder.device(vramp(), "ramp", &[], &[Port::Node(ramp.clone()), Port::Node(gnd.clone())]);
        let watch = match watched {
            Watched::Output => out.clone(),
            Watched::Ramp => ramp,
        };
        for (index, level) in levels.iter().enumerate() {
            let reference = builder.node(&format!("r{index}"));
            add_vsrc(&mut builder, &format!("vr{index}"), *level, &reference, &gnd);
            let mode = format!("m{}", index + 1);
            builder.device(comparator(), &format!("cmp{index}"), &[], &[Port::Node(watch.clone()), Port::Node(reference), Nets::port(&mode)]);
        }
        (builder, out)
    }

    /// The accepted `(time, value of o)` of a transient to `stop`, and the times the first
    /// comparator's `up` crossing fired at.
    fn run(levels: &[f64], watched: Watched, trigger: Trigger, mirrored: bool, stop: f64, context: Context) -> Result<(Vec<(f64, f64)>, Vec<f64>), String> {
        let (builder, out) = Scenario::circuit(levels, watched, trigger, mirrored);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut solver = Solver::new(builder.build().expect("build"))
            .with_context(context)
            .with_tran_opts(TransientAnalysisOptions::new(stop, stop / 50.0))
            .with_sink(Box::new(Collect(Rc::clone(&seen))))
            .build();
        let result = solver.tran().expect("tran").solve().map_err(|e| format!("{e}"))?;
        let steps = result.iter().map(|step| (step.time(), step.get_node(&node_id(&out)).expect("o"))).collect();
        let crossings = seen
            .borrow()
            .iter()
            .filter(|observation| observation.kind == ObservationKind::Crossing && observation.path == "cmp0.up")
            .map(|observation| observation.time)
            .collect();
        Ok((steps, crossings))
    }
}


#[test]
fn a_crossing_made_by_a_jump_retakes_the_step() {
    // AO-13: the code jumps at 1 us; the first step after it reads 1 V, which crosses the 0.5 V
    // comparator; its reaction sets m1; that step is retaken, so no accepted solution after the
    // jump holds the stale 1 V and the step the crossing fired in holds 2 V.
    let (steps, crossings) = Scenario::run(&[0.5], Watched::Output, Trigger::At(JUMP_AT), false, 3.0e-6, Context::default()).expect("tran");
    assert_eq!(crossings.len(), 1, "the crossing fires once: {crossings:?}");
    let after: Vec<&(f64, f64)> = steps.iter().filter(|(time, _)| *time > JUMP_AT + 1.0e-12).collect();
    let first = after.first().expect("a step after the jump");
    assert_eq!(first.0, crossings[0], "the crossing fired in the step that read the jump");
    for (time, value) in &after {
        assert!((value - 2.0).abs() < 1e-9, "o = {value} at {time:e}: a step after the jump holds the stale 1 V");
    }
    assert_eq!(steps.iter().filter(|(time, _)| *time == first.0).count(), 1, "the retaken solution replaces the first");
    assert_eq!(reactions("comparator"), 1, "the reaction that ran is not run again");
}

#[test]
fn a_step_that_reads_no_jump_never_retakes() {
    // AO-13 control: the code is set at the start, so the ramp's crossing of 0.5 V at 0.5 ms is
    // made by a step that reads no jump: its reaction's change waits for the next step (o is
    // 1 V in the crossing's step, 2 V after).
    let (steps, crossings) = Scenario::run(&[0.5], Watched::Ramp, Trigger::At(0.0), false, 1.0e-3, Context::default()).expect("tran");
    assert_eq!(crossings.len(), 1, "{crossings:?}");
    let at = steps.iter().position(|(time, _)| *time == crossings[0]).expect("a step at the crossing");
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
    let error = Scenario::run(&levels, Watched::Output, Trigger::At(JUMP_AT), false, 3.0e-6, context).expect_err("two rounds are not enough");
    assert!(error.contains("E0504"), "{error}");
    assert!(error.contains("more than 2 times"), "{error}");
    assert!(error.contains("t = 1"), "the instant is named: {error}");

    let (steps, _) = Scenario::run(&levels, Watched::Output, Trigger::At(JUMP_AT), false, 3.0e-6, Context::default()).expect("the default bound settles");
    let last = steps.last().expect("steps");
    assert!((last.1 - 4.0).abs() < 1e-9, "o = {} V", last.1);
}

#[test]
fn a_crossing_made_by_a_step_that_reads_a_settle_commit_retakes_it() {
    // §15 S2, S3 and §15.2 rule 4: the settle phase of the first time point commits the code; the
    // step to the next time point reads it as a jump (0 to 1 V, crossing the 0.5 V comparator), its
    // reaction sets m1, and that step is retaken: no accepted solution holds the stale 1 V.
    let (steps, crossings) = Scenario::run(&[0.5], Watched::Output, Trigger::Settle, false, 3.0e-6, Context::default()).expect("tran");
    assert_eq!(crossings.len(), 1, "the crossing fires once: {crossings:?}");
    let at = steps.iter().position(|(time, _)| *time == crossings[0]).expect("a step at the crossing");
    for (time, value) in &steps[at..] {
        assert!((value - 2.0).abs() < 1e-9, "o = {value} at {time:e}: the step that read the settle commit holds the stale 1 V");
    }
    assert_eq!(steps.iter().filter(|(time, _)| *time == crossings[0]).count(), 1, "the retaken solution replaces the first");
    assert_eq!(reactions("comparator"), 1, "the reaction that ran is not run again");
}

#[test]
fn a_second_round_retake_runs_settle_once_and_delivers_its_settle_commits() {
    // §15 step 4 ("once") and S2: at the jump the first retaken solution (2 V) crosses the 1.5 V
    // comparator, so the instant runs a second round. Its settle phase runs once, after the last
    // round, and its settle commit (m3 <- m2, the final state) reaches the step that follows: o = 4 V
    // there (code, m1, m2 and m3), not 3 V from the state the first round left.
    let (steps, _) = Scenario::run(&[0.5, 1.5], Watched::Output, Trigger::At(JUMP_AT), true, 3.0e-6, Context::default()).expect("tran");
    let times = settle_times();
    for pair in times.windows(2) {
        assert!(pair[1] > pair[0], "a settle phase ran twice at t = {:e}: {times:?}", pair[0]);
    }
    let jump = steps.iter().position(|(time, value)| *time > JUMP_AT && (value - 3.0).abs() < 1e-9).expect("the retaken step holds 3 V");
    let following = &steps[jump + 1];
    assert!((following.1 - 4.0).abs() < 1e-9, "o = {} V at {:e}: the settle commit of the last round did not reach the analog side", following.1, following.0);
}

#[test]
fn the_first_step_after_an_operating_point_that_holds_a_settle_commit_is_first_order() {
    // §15.2 rule 6, §15 S3: the operating point's settle commit sets the code, which the first step
    // reads as a jump (0 to 1 V) across a 1 nF capacitor. A first-order step ends with the secant
    // C * 1 V / h as its derivative, so the source gives -C / h at that step (the BDF2 stage would
    // end with a derivative of the opposite sign or another size).
    let capacitance = 1.0e-9;
    let (mut builder, out) = Scenario::circuit(&[], Watched::Output, Trigger::Settle, false);
    let ground = builder.ground();
    abi_devices::add_capacitor(&mut builder, "c1", capacitance, &out, &ground);
    let mut solver = Solver::new(builder.build().expect("build")).with_context(Context::default()).with_tran_opts(TransientAnalysisOptions::new(1.0e-6, 2.0e-8)).build();
    let result = solver.tran().expect("tran").solve().expect("solve");
    let steps: Vec<_> = result.iter().collect();
    let first = &steps[1];
    assert!((first.get_node(&node_id(&out)).expect("o") - 1.0).abs() < 1e-9, "the first step reads the committed code");
    let current = first.get_branch(piperine_solver::abi::BranchIdentifier::new("gate", "f")).expect("the gate's branch");
    let secant = -capacitance * 1.0 / first.time();
    assert!((current - secant).abs() < 1e-6 * secant.abs(), "the first step's derivative is the secant: {current:e} against {secant:e}");
}
