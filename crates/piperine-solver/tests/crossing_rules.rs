//! Crossings across operating points and the run errors of crossings and step caps
//! (feature `solver-crossings`, SCX-02, SCX-03; docs/spec Part IV §13 rule 5, Part V §16.1
//! rule 2, §17.2 rule 5, §17.3): a `CROSS` watch compares successive solutions, so it fires in
//! the round of an operating point that moves it above zero, and at a later operating point of
//! the same circuit; an invalid `ttol` or `etol` is `E0510`, an invalid step cap `E0509`.

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

use abi_devices::{add_vsrc, vsine};
use abi_mixed::{capped, comparator, reactions};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, GROUND, Module, Partial, Shape, Statement, cell, global, op, part, pin_flag, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

const ORIGIN: &str = "crossing_rules.rs";

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}

// ─── Devices ────────────────────────────────────────────────────────────────

/// A source of 0 V that a reaction on `START` raises to 1 V: the operating point's first
/// round solves 0 V, the instant after it commits `hi`, and the next round solves 1 V.
/// pins: p=0 (PORT continuous); f=1 (PRIVATE continuous); hi=2 (PRIVATE BIT, init LOW);
///       start=3 (GLOBAL START)
/// terms: T0 FLOW{p, GROUND} "i" = f, T1 FLOW{f, GROUND} "eq" = v(p) − (hi ? 1 : 0)
/// fn 0: reads [p, f, hi] writes [T0, T1] partials (0<-f) (1<-p)
/// reaction 0: on [start] writes [S0 {hi}]; hi <- HIGH, no delay
struct Riser;
static RISER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RISER: OnceLock<Module> = OnceLock::new();

fn riser_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let hi = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("hi", role::PRIVATE, &BIT) });
    let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
    let flow = |name, plus| TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus: GROUND, abstol: piperine_abi::raw::DEFAULT, max: piperine_abi::raw::DEFAULT };
    let t0 = s.term(flow("i", p));
    let t1 = s.term(flow("eq", f));
    let term = |index| Cell { kind: cell::TERM, index, part: part::VALUE };
    s.function(FunctionSpec {
        name: "v",
        origin: ORIGIN,
        reads: vec![pin(p), pin(f), pin(hi)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 1), partial(1, 0)],
        ..FunctionSpec::default()
    });
    let st = Statement { pin: hi, offset: 0, len: 2, flags: 0, min_after: 0.0 };
    s.reaction(ReactionSpec { name: "rise", origin: ORIGIN, on: vec![start], writes: vec![st], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for Riser {
    fn prototype() -> &'static Shape<'static> {
        RISER_SHAPE.get_or_init(riser_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Riser, Arc::clone(RISER_SHAPE.get_or_init(riser_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = if cx.bytes(2) == HIGH { 1.0 } else { 0.0 };
        cx.set(0, cx.f64(1));
        cx.set(1, cx.f64(0) - level);
        cx.jac().copy_from_slice(&[1.0, 1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

fn riser() -> &'static Module {
    RISER.get_or_init(|| module::<Riser>("Riser"))
}

/// A comparator whose `ttol` and `etol` are its own params (abi.md §3 `Watch { ttol, etol }`).
/// pins: ttol=0, etol=1 (PARAM F64, 1 ps); inp=2, inn=3 (PORT continuous); up=4 (PRIVATE event)
/// watches: W0 CROSS "rise" -> up, tolerances from pins 0 and 1
/// fn 0: reads [inp, inn] writes [W0] partials (0<-0) (0<-1)
struct TolComparator;
static TOL_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TOL: OnceLock<Module> = OnceLock::new();
const ONE_PS: [u8; 8] = 1.0e-12f64.to_le_bytes();

fn tol_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let param = |name, init| PinSpec { flags: pin_flag::HAS_INIT, init, ..PinSpec::discrete(name, role::PARAM, &F64) };
    let ttol = s.pin(param("ttol", &ONE_PS));
    let etol = s.pin(param("etol", &ONE_PS));
    let inp = s.pin(PinSpec::continuous("inp", role::PORT));
    let inn = s.pin(PinSpec::continuous("inn", role::PORT));
    let up = s.pin(PinSpec::event("up", role::PRIVATE));
    let w0 = s.watch(WatchSpec { name: "rise", origin: ORIGIN, kind: watch::CROSS, layout: &F64, event: up, ttol, etol });
    s.function(FunctionSpec {
        name: "e",
        origin: ORIGIN,
        reads: vec![pin(inp), pin(inn)],
        writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for TolComparator {
    fn prototype() -> &'static Shape<'static> {
        TOL_SHAPE.get_or_init(tol_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TolComparator, Arc::clone(TOL_SHAPE.get_or_init(tol_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, cx.f64(0) - cx.f64(1));
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn tol_comparator() -> &'static Module {
    TOL.get_or_init(|| module::<TolComparator>("TolComparator"))
}

/// Every function of this target's own devices passes the partials verifier (the guard of
/// `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Riser", riser());
    partials::verify_every_function("TolComparator", tol_comparator());
}

// ─── SCX-02 ─────────────────────────────────────────────────────────────────

#[test]
fn a_crossing_between_operating_point_rounds_fires_in_the_later_round() {
    // SCX-02 AC 6 (§13 rule 5: two rounds of an operating point, "fires in the round that
    // follows"): round 1 solves the riser at 0 V, below the comparator's 0.5 V; the START
    // reaction raises it; round 2 solves 1 V. The comparator fires once, in the operating
    // point, and `out` is HIGH after it.
    let mut b = CircuitBuilder::new("rounds");
    let gnd = b.ground();
    let a = b.node("a");
    let r = b.node("r");
    b.device(riser(), "src", &[], &[Port::Node(a.clone())]);
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(a), Port::Node(r), net("out")]);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut solver = Solver::new(b.build().expect("build")).with_sink(Box::new(Collect(Rc::clone(&seen)))).build();
    solver.dc().expect("dc").solve().expect("the operating point");
    let fired: Vec<Observation> =
        seen.borrow().iter().filter(|o| o.kind == ObservationKind::Crossing && o.path == "cmp.up").cloned().collect();
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert_eq!(fired[0].analysis, AnalysisKind::OperatingPoint);
    assert_eq!(reactions("comparator"), 1);
    let bound = solver.circuit().bound().unwrap();
    assert_eq!(bound.digital.read(bound.names.nets["net:out"]), HIGH);
}

#[test]
fn a_crossing_between_two_operating_points_fires_at_the_later() {
    // SCX-02 AC 7 (§16.1 rule 2: "cross compares the last solution of one point with the first
    // of the next"): a source at 0.3 V against 0.5 V, then set to 0.7 V for the next operating
    // point of the same circuit. Nothing fires at the first, the comparator fires at the second.
    let mut b = CircuitBuilder::new("points");
    let gnd = b.ground();
    let a = b.node("a");
    let r = b.node("r");
    add_vsrc(&mut b, "va", 0.3, &a, &gnd);
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(a), Port::Node(r), net("out")]);
    let mut circuit = b.build().expect("build");
    circuit.dc(Context::default()).expect("dc").solve().expect("point 1");
    let out = |c: &piperine_solver::prelude::CircuitInstance| {
        let bound = c.bound().unwrap();
        bound.digital.read(bound.names.nets["net:out"]).to_vec()
    };
    assert_eq!(reactions("comparator"), 0, "0.3 V: nothing fires");
    assert_eq!(out(&circuit), LOW);
    circuit.set("va", "dc", ParamValue::Real(0.7)).expect("set");
    circuit.dc(Context::default()).expect("dc").solve().expect("point 2");
    assert_eq!(reactions("comparator"), 1, "0.3 V then 0.7 V: it fires at the later point");
    assert_eq!(out(&circuit), HIGH);
}

// ─── SCX-03 ─────────────────────────────────────────────────────────────────

/// A transient of the 1 V 1 kHz sine with `add` placed on it; the error text.
fn failing(add: impl FnOnce(&mut CircuitBuilder, piperine_solver::abi::AnalogReference)) -> String {
    let mut b = CircuitBuilder::new("failing");
    let gnd = b.ground();
    let top = b.node("top");
    b.device(vsine(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
        Port::Node(top.clone()),
        Port::Node(gnd),
    ]);
    add(&mut b, top);
    let mut solver =
        Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(1.0e-4, 1.0e-6)).build();
    match solver.tran().expect("tran").solve() {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

#[test]
fn an_invalid_ttol_or_etol_fails_the_analysis_with_e0510() {
    // SCX-03 AC 8 (§17.3 resolution): a ttol or etol that is not finite and positive
    for which in ["ttol", "etol"] {
        for bad in [0.0, -1.0e-12, f64::NAN, f64::INFINITY] {
            let err = failing(|b, top| {
                let gnd = b.ground();
                let r = b.node("r");
                add_vsrc(b, "vr", 0.5, &r, &gnd);
                b.device(tol_comparator(), "cmp", &[(which, ParamValue::Real(bad))], &[Port::Node(top), Port::Node(r)]);
            });
            assert!(
                err.contains("E0510") && err.contains("`cmp`") && err.contains("`rise`") && err.contains(which),
                "{which} = {bad}: {err:?}"
            );
        }
    }
    let ok = failing(|b, top| {
        let gnd = b.ground();
        let r = b.node("r");
        add_vsrc(b, "vr", 0.5, &r, &gnd);
        b.device(tol_comparator(), "cmp", &[], &[Port::Node(top), Port::Node(r)]);
    });
    assert_eq!(ok, "", "1 ps and 1 ps are valid");
}

#[test]
fn an_invalid_step_cap_fails_the_analysis_with_e0509() {
    // SCX-03 AC 9 (§17.2 rule 5 and its resolution): a step cap not finite and positive, and not
    // `+∞`: `+∞` is a `bound_step` not executed at this solution and caps nothing (abi.md, the
    // `CAP` row; analog-structure AS-01 AC 5)
    for bad in [0.0, -1.0e-9, f64::NAN, f64::NEG_INFINITY] {
        let err = failing(|b, top| {
            b.device(capped(), "cc", &[("cap", ParamValue::Real(bad))], &[Port::Node(top)]);
        });
        assert!(err.contains("E0509") && err.contains("`cc`") && err.contains("watch `cap`"), "cap {bad}: {err:?}");
    }
    let ok = failing(|b, top| {
        b.device(capped(), "cc", &[("cap", ParamValue::Real(1.0e-6))], &[Port::Node(top)]);
    });
    assert_eq!(ok, "", "1 µs is valid");
    let infinite = failing(|b, top| {
        b.device(capped(), "cc", &[("cap", ParamValue::Real(f64::INFINITY))], &[Port::Node(top)]);
    });
    assert_eq!(infinite, "", "+∞ caps nothing");
}
