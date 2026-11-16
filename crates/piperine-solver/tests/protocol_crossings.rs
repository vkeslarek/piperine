//! Crossings, samples and step caps on the transient of a circuit of devices
//! (SAI-11, SAI-02; abi.md C6, R2, §4.3): a `CROSS` watch that fired is a
//! commit to its event pin at microstep 0 of the landing; a reaction reads the
//! accepted solution; the smallest of several `CAP` watches bounds the next
//! step, whatever their order. A crossing is located inside its step
//! (`solver-crossings`, SCX-01; docs/spec §17.3): within `ttol` plus one tick
//! or `etol`, on a tick, the earliest first, not after a jump.

// The targets use the libraries selectively.
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

use abi_devices::{add_capacitor, add_resistor, add_vsrc, node_id, vramp, vsine};
use abi_mixed::{capped, clock_src, comparator, delay, reactions, sampler};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, Module, Partial, Shape, Statement, cell, part, pin_flag, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, WatchSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

const ORIGIN: &str = "protocol_crossings.rs";

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

struct Collect(Rc<RefCell<Vec<Observation>>>);

impl Sink for Collect {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

/// A 1 V, 1 kHz sine at `top`, loaded by 1 kΩ and 1 µF.
fn sine(b: &mut CircuitBuilder) -> AnalogReference {
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    b.device(vsine(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &[
        Port::Node(top.clone()),
        Port::Node(gnd.clone()),
    ]);
    add_resistor(b, "r1", 1.0e3, &top, &out);
    add_capacitor(b, "c1", 1.0e-6, &out, &gnd);
    top
}

#[test]
fn a_fired_crossing_is_a_commit_at_the_landing() {
    let mut b = CircuitBuilder::new("crossing");
    let top = sine(&mut b);
    let gnd = b.ground();
    let r = b.node("r");
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(top.clone()), Port::Node(r), net("out")]);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut solver = Solver::new(b.build().expect("build"))
        .with_tran_opts(TransientAnalysisOptions::new(2.0e-4, 1.0e-5))
        .with_sink(Box::new(Collect(Rc::clone(&seen))))
        .build();
    let result = solver.tran().expect("tran").solve().expect("solve");
    let seen = seen.borrow();

    let crossings: Vec<(usize, &Observation)> =
        seen.iter().enumerate().filter(|(_, o)| o.kind == ObservationKind::Crossing && o.path == "cmp.up").collect();
    assert_eq!(crossings.len(), 1, "the sine crosses 0.5 V upward once before 0.2 ms");
    let (ci, crossing) = crossings[0];
    assert_eq!(crossing.analysis, AnalysisKind::Transient);
    // SCX-01 AC 2: the crossing is located (it used to fire at the first accepted
    // solution after the sign change, wherever that fell). It lands on a tick, an
    // accepted step is at exactly that time, e ≤ 0 at the step before and e > 0 there,
    // within etol (1 µV) or ttol (1 ps) plus one tick of asin(0.5)/(2π·1 kHz).
    let ticks = crossing.time / TICK;
    assert_eq!(ticks, ticks.round(), "the landing is a tick: {:e}", crossing.time);
    let k = result.iter().position(|s| s.time() == crossing.time).expect("an accepted step at exactly the landing");
    let v = |i: usize| result.get(i).unwrap().get_node(&node_id(&top)).unwrap();
    assert!(v(k - 1) <= 0.5 && v(k) > 0.5, "the sign change lies in the last step: {} {}", v(k - 1), v(k));
    let late = crossing.time - sine_truth();
    assert!(v(k) - 0.5 < ETOL || late.abs() <= TTOL + TICK, "located: e = {:e}, late {late:e}", v(k) - 0.5);
    // `out` changes one microstep later, in the same instant.
    let (oi, out) = seen
        .iter()
        .enumerate()
        .find(|(_, o)| o.path == "out" && o.time > 0.0 && matches!(o.kind, ObservationKind::Change { .. }))
        .expect("out changed");
    assert_eq!(out.time, crossing.time);
    assert!(oi > ci, "after the firing");
    assert_eq!(reactions("comparator"), 1);
}

#[test]
fn a_reaction_reads_the_accepted_solution() {
    let at = 2.0e-4;
    let mut b = CircuitBuilder::new("sampler");
    let top = sine(&mut b);
    b.device(delay(), "kick", &[("at", ParamValue::Real(at))], &[net("go")]);
    b.device(sampler(), "smp", &[], &[Port::Node(top.clone()), net("go"), net("s")]);
    let mut solver = Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(5.0e-4, 1.0e-5)).build();
    let result = solver.tran().expect("tran").solve().expect("solve");

    let landing = result.iter().find(|s| (s.time() - at).abs() < 1.0e-15).expect("the probing instant is a landing");
    let bound = solver.circuit().bound().unwrap();
    let s = bound.digital.read(bound.names.nets["net:s"]).to_vec();
    assert_eq!(s[8], 1, "the sampler ran");
    let sample = f64::from_le_bytes(s[..8].try_into().unwrap());
    assert_eq!(sample, landing.get_node(&node_id(&top)).unwrap(), "the accepted solution at its instant");
    assert!(sample > 0.5, "sin(2π·0.2) = 0.95: {sample}");
}

#[test]
fn a_cap_watch_bounds_the_next_step() {
    // Two CAP watches, 4 ns and 1 ns, placed in both orders: the smallest bounds
    // every step, and the controller reaches it (a 10 ns dt asks for more).
    for caps in [[4.0e-9, 1.0e-9], [1.0e-9, 4.0e-9]] {
        let mut b = CircuitBuilder::new("capped sine");
        let top = sine(&mut b);
        for (i, cap) in caps.into_iter().enumerate() {
            b.device(capped(), &format!("cap{i}"), &[("cap", ParamValue::Real(cap))], &[Port::Node(top.clone())]);
        }
        let mut solver = Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8)).build();
        let result = solver.tran().expect("tran").solve().expect("solve");
        assert!(result.len() > 200, "at least 200 steps of 1 ns: {}", result.len());
        let mut widest = 0.0_f64;
        for pair in result.iter().collect::<Vec<_>>().windows(2) {
            let dt = pair[1].time() - pair[0].time();
            assert!(dt <= 1.0e-9 * (1.0 + 1.0e-9), "caps {caps:?}: a step of {dt:e} s past the 1 ns cap at t = {:e}", pair[0].time());
            widest = widest.max(dt);
        }
        assert!(widest > 0.99e-9, "caps {caps:?}: the steps reach the smallest cap, widest {widest:e} s");
    }
}

// ─── Located crossings (feature `solver-crossings`, SCX-01) ─────────────────

/// One tick of the session's default resolution, 1 fs.
const TICK: f64 = 1.0e-15;
/// The default `ttol` (`TransientAnalysisOptions::ttol`) and potential `abstol` (`vntol`).
const TTOL: f64 = 1.0e-12;
const ETOL: f64 = 1.0e-6;

/// A comparator whose `ttol` and `etol` are its own params (abi.md §3 `Watch { ttol, etol }`).
/// pins: ttol=0, etol=1 (PARAM F64); inp=2, inn=3 (PORT continuous); out=4 (PORT BIT, init LOW);
///       up=5 (PRIVATE event)
/// watches: W0 CROSS "rise" -> up, tolerances from pins 0 and 1
/// fn 0: reads [inp, inn] writes [W0] partials (0<-0) (0<-1);  reaction 0: on [up] out <- HIGH
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
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    let up = s.pin(PinSpec::event("up", role::PRIVATE));
    let w0 = s.watch(WatchSpec { name: "rise", origin: ORIGIN, kind: watch::CROSS, layout: &F64, event: up, ttol, etol });
    let cell = |kind, index| Cell { kind, index, part: part::VALUE };
    s.function(FunctionSpec {
        name: "e",
        origin: ORIGIN,
        reads: vec![cell(cell::PIN, inp), cell(cell::PIN, inn)],
        writes: vec![cell(cell::WATCH, w0)],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    let st = Statement { pin: out, offset: 0, len: 2, flags: 0, min_after: 0.0 };
    s.reaction(ReactionSpec { name: "up", origin: ORIGIN, on: vec![up], writes: vec![st], ..ReactionSpec::default() });
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
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
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
    partials::verify_every_function("TolComparator", tol_comparator());
    partials::verify_every_function("Saturating", saturating());
}

/// A transient of `b` to `stop` from the initial step `dt`, with `options` adjusted by `adjust`;
/// returns the result and every observation.
fn run(
    b: CircuitBuilder,
    stop: f64,
    dt: f64,
    adjust: impl FnOnce(&mut TransientAnalysisOptions),
    context: Context,
) -> (piperine_solver::prelude::TransientAnalysisResult, Vec<Observation>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut options = TransientAnalysisOptions::new(stop, dt);
    adjust(&mut options);
    let mut solver = Solver::new(b.build().expect("build"))
        .with_context(context)
        .with_tran_opts(options)
        .with_sink(Box::new(Collect(Rc::clone(&seen))))
        .build();
    let result = solver.tran().expect("tran").solve().expect("solve");
    let seen = seen.borrow().clone();
    (result, seen)
}

/// The times of the transient `Crossing` observations of `path`.
fn crossings(seen: &[Observation], path: &str) -> Vec<f64> {
    seen.iter()
        .filter(|o| o.kind == ObservationKind::Crossing && o.path == path && o.analysis == AnalysisKind::Transient)
        .map(|o| o.time)
        .collect()
}

/// `v(node)` at the accepted step at exactly `t`.
fn at(result: &piperine_solver::prelude::TransientAnalysisResult, node: &AnalogReference, t: f64) -> f64 {
    let step = result.iter().find(|s| s.time() == t).unwrap_or_else(|| panic!("no accepted step at {t:e} s"));
    step.get_node(&node_id(node)).expect("the node")
}

/// The sine of `sine()` against `vref` through a default comparator `cmp`.
fn sine_against(vref: f64) -> (CircuitBuilder, AnalogReference) {
    let mut b = CircuitBuilder::new("crossing");
    let top = sine(&mut b);
    let gnd = b.ground();
    let r = b.node("r");
    add_vsrc(&mut b, "vr", vref, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(top.clone()), Port::Node(r), net("out")]);
    (b, top)
}

/// The true upward crossing of 0.5 V by the 1 V, 1 kHz sine: asin(0.5) / (2π · 1 kHz).
fn sine_truth() -> f64 {
    0.5f64.asin() / (2.0 * std::f64::consts::PI * 1.0e3)
}

#[test]
fn a_ramp_lands_within_ttol_plus_one_tick() {
    // SCX-01 AC 1: a 1 kV/s ramp crosses 0.5 V at 0.5 ms exactly. `etol` is 1e-300 V, out of play,
    // so the landing must exceed the crossing by at most ttol (1 ps) plus one tick, and lie on
    // the side where e > 0, whatever step the run starts from
    let truth = 0.5e-3;
    for dt in [10.0e-6, 7.0e-6, 3.0e-6] {
        let mut b = CircuitBuilder::new("ramp");
        let gnd = b.ground();
        let top = b.node("top");
        let r = b.node("r");
        b.device(vramp(), "v1", &[], &[Port::Node(top.clone()), Port::Node(gnd.clone())]);
        add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
        let params = [("ttol", ParamValue::Real(TTOL)), ("etol", ParamValue::Real(1.0e-300))];
        b.device(tol_comparator(), "cmp", &params, &[Port::Node(top.clone()), Port::Node(r), net("out")]);
        let (result, seen) = run(b, 1.0e-3, dt, |_| {}, Context::default());
        let fired = crossings(&seen, "cmp.up");
        assert_eq!(fired.len(), 1, "dt {dt:e}: one crossing: {fired:?}");
        let late = fired[0] - truth;
        assert!(late <= TTOL + TICK, "dt {dt:e}: landed {late:e} s after the crossing");
        assert!(late > -TICK, "dt {dt:e}: landed {late:e} s before the crossing");
        assert!(at(&result, &top, fired[0]) > 0.5, "dt {dt:e}: e > 0 where it fires");
    }
}

#[test]
fn a_sine_crossing_is_located() {
    // SCX-01 AC 1, the T45 repro: the 1 V 1 kHz sine against 0.5 V fires 0.97 µs late today, at
    // the first accepted point after the sign change. Located, 0 < e < etol (1 µV) or the landing
    // is within ttol (1 ps) plus one tick of asin(0.5) / (2π·1 kHz)
    let (b, top) = sine_against(0.5);
    let (result, seen) = run(b, 2.0e-4, 1.0e-6, |o| o.dt_max = 1.0e-6, Context::default());
    let fired = crossings(&seen, "cmp.up");
    assert_eq!(fired.len(), 1, "{fired:?}");
    let e = at(&result, &top, fired[0]) - 0.5;
    let late = fired[0] - sine_truth();
    assert!(e > 0.0, "e > 0 where it fires: {e:e}");
    assert!(e < ETOL || late.abs() <= TTOL + TICK, "late by {late:e} s with e = {e:e} V");
}

#[test]
fn an_rc_charge_trips_at_its_threshold() {
    // SCX-01 AC 1, the T45 RC repro: a clock steps `clk` from 0 to 1 V at period/4 = 1 µs; 1 kΩ
    // into 1 nF (τ = 1 µs) charges `x` as 1 − exp(−(t − 1 µs)/τ), which crosses 0.625 V at
    // 1 µs + τ·ln(1/0.375). Today it trips at 0.6352 V. Located, 0 < v − 0.625 < 1 µV, or the
    // landing is within 1 ps plus one tick of the analytic time.
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let clk = b.node("clk");
    let x = b.node("x");
    let r = b.node("r");
    b.device(clock_src(), "ck", &[("period", ParamValue::Real(4.0e-6))], &[Port::Node(clk.clone()), Port::Node(gnd.clone())]);
    add_resistor(&mut b, "r1", 1.0e3, &clk, &x);
    add_capacitor(&mut b, "c1", 1.0e-9, &x, &gnd);
    add_vsrc(&mut b, "vr", 0.625, &r, &gnd);
    b.device(comparator(), "cmp", &[], &[Port::Node(x.clone()), Port::Node(r), net("out")]);
    let (result, seen) = run(b, 2.5e-6, 1.0e-8, |o| o.dt_max = 5.0e-8, Context::default());
    let fired = crossings(&seen, "cmp.up");
    assert_eq!(fired.len(), 1, "{fired:?}");
    let truth = 1.0e-6 + 1.0e-6 * (1.0f64 / 0.375).ln();
    let e = at(&result, &x, fired[0]) - 0.625;
    let late = fired[0] - truth;
    assert!(e > 0.0, "e > 0 where it trips: {e:e}");
    assert!(e < ETOL || late.abs() <= TTOL + TICK, "late by {late:e} s with v − 0.625 = {e:e} V");
    // the integrator's own error only: the landing is the analytic time to within 1 ns
    assert!(late.abs() < 1.0e-9, "{late:e} s from the analytic crossing");
}

#[test]
fn the_earliest_of_several_crossings_lands_first() {
    // SCX-01 AC 3: on a 1 kV/s ramp, thresholds 0.3 V and 0.7 V cross at 0.3 ms and 0.7 ms,
    // inside one step of a 1 ms initial step and no cap. The solver lands on the earlier first,
    // then the later, each within ttol plus one tick (etol out of play)
    let mut b = CircuitBuilder::new("ramp");
    let gnd = b.ground();
    let top = b.node("top");
    b.device(vramp(), "v1", &[], &[Port::Node(top.clone()), Port::Node(gnd.clone())]);
    for (name, vth) in [("lo", 0.3), ("hi", 0.7)] {
        let r = b.node(&format!("r_{name}"));
        add_vsrc(&mut b, &format!("v_{name}"), vth, &r, &gnd);
        let params = [("ttol", ParamValue::Real(TTOL)), ("etol", ParamValue::Real(1.0e-300))];
        b.device(tol_comparator(), &format!("c_{name}"), &params, &[Port::Node(top.clone()), Port::Node(r), net(name)]);
    }
    let (_, seen) = run(b, 1.0e-3, 1.0e-3, |o| o.dt_max = 1.0e-3, Context::default());
    let lo = crossings(&seen, "c_lo.up");
    let hi = crossings(&seen, "c_hi.up");
    assert_eq!((lo.len(), hi.len()), (1, 1), "{lo:?} {hi:?}");
    for (fired, truth) in [(lo[0], 0.3e-3), (hi[0], 0.7e-3)] {
        assert!(fired - truth <= TTOL + TICK && fired - truth > -TICK, "{fired:e} vs {truth:e}");
    }
    let order: Vec<&str> = seen
        .iter()
        .filter(|o| o.kind == ObservationKind::Crossing && o.analysis == AnalysisKind::Transient)
        .map(|o| o.path.as_str())
        .collect();
    assert_eq!(order, ["c_lo.up", "c_hi.up"], "the earlier lands first");
}

#[test]
fn crossings_on_one_tick_fire_together() {
    // SCX-01 AC 3: two comparators on one sine and one 0.5 V reference cross at the same
    // instant: they land on one tick and fire together there
    let (mut b, top) = sine_against(0.5);
    let r2 = b.node("r2");
    let gnd = b.ground();
    add_vsrc(&mut b, "vr2", 0.5, &r2, &gnd);
    b.device(comparator(), "cmp2", &[], &[Port::Node(top), Port::Node(r2), net("out2")]);
    let (_, seen) = run(b, 2.0e-4, 1.0e-6, |o| o.dt_max = 1.0e-6, Context::default());
    let (a, b2) = (crossings(&seen, "cmp.up"), crossings(&seen, "cmp2.up"));
    assert_eq!((a.len(), b2.len()), (1, 1), "{a:?} {b2:?}");
    assert_eq!(a[0], b2[0], "one tick");
    let ticks = a[0] / TICK;
    assert_eq!(ticks, ticks.round(), "a tick: {:e}", a[0]);
}

#[test]
fn the_session_ttol_applies_to_a_watch_without_a_ttol_pin() {
    // SCX-01 AC 4: the comparator has no ttol pin. With TransientAnalysisOptions::ttol at 1 s
    // every crossing is "within ttol" at the first accepted point after the sign change, so it
    // fires there, where e exceeds 1 µV; at the default 1 ps it is located (C6)
    let (b, top) = sine_against(0.5);
    let (result, seen) = run(b, 2.0e-4, 1.0e-5, |o| o.ttol = 1.0, Context::default());
    let fired = crossings(&seen, "cmp.up");
    assert_eq!(fired.len(), 1);
    let k = result.iter().position(|s| s.time() == fired[0]).expect("an accepted step");
    let v = |i: usize| result.get(i).unwrap().get_node(&node_id(&top)).unwrap();
    assert!(v(k - 1) <= 0.5 && v(k) > 0.5 + ETOL, "unlocated: {} then {}", v(k - 1), v(k));
    assert!(fired[0] - sine_truth() > TTOL + TICK, "later than a located landing");
}

#[test]
fn a_watch_without_an_etol_pin_takes_the_potential_abstol() {
    // SCX-01 AC 4: the comparator has no etol pin, so its etol is the session's vntol. The
    // circuit is linear (the solve is exact whatever vntol). At vntol = 0.6 V every rising e
    // of this sine (under 0.5 V) is under etol: it fires at the first accepted point after the
    // sign change, e above 1 µV. At the default vntol it fires under 1 µV.
    let mut loose = Context::default();
    loose.tolerances.vntol = 0.6;
    let (b, top) = sine_against(0.5);
    let (result, seen) = run(b, 2.0e-4, 1.0e-5, |_| {}, loose);
    let fired = crossings(&seen, "cmp.up");
    assert_eq!(fired.len(), 1);
    let e = at(&result, &top, fired[0]) - 0.5;
    assert!(e > ETOL, "etol 0.6 V: not located, e = {e:e}");

    let (b, top) = sine_against(0.5);
    let (result, seen) = run(b, 2.0e-4, 1.0e-5, |_| {}, Context::default());
    let fired = crossings(&seen, "cmp.up");
    let e = at(&result, &top, fired[0]) - 0.5;
    let late = fired[0] - sine_truth();
    assert!(e > 0.0 && (e < ETOL || late.abs() <= TTOL + TICK), "etol 1 µV: e = {e:e}, late {late:e}");
}

#[test]
fn a_crossing_in_the_step_after_a_jump_is_not_located() {
    // SCX-01 AC 5 (§17.3 rule 8): the clock steps from 0 to 1 V at period/4 = 0.25 ms, a jump.
    // A comparator on it against 0.5 V sees the step that reads the jump go from −0.5 to 0.5:
    // that crossing is not located, so the step is not retaken: it fires at the end of the step
    // the controller proposed, the same time and the same steps as a run with ttol = 1 s, where
    // every crossing is accepted where it is first seen.
    let period = 1.0e-3;
    let edge_run = |ttol: f64| {
        let mut b = CircuitBuilder::new("edge");
        let gnd = b.ground();
        let clk = b.node("clk");
        let r = b.node("r");
        b.device(clock_src(), "ck", &[("period", ParamValue::Real(period))], &[Port::Node(clk.clone()), Port::Node(gnd.clone())]);
        add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
        b.device(comparator(), "cmp", &[], &[Port::Node(clk), Port::Node(r), net("c")]);
        let (result, seen) = run(b, 0.4e-3, 1.0e-5, |o| o.ttol = ttol, Context::default());
        let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
        (crossings(&seen, "cmp.up"), times)
    };
    let (fired, times) = edge_run(TTOL);
    let (unlocated, unlocated_times) = edge_run(1.0);
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert_eq!(fired, unlocated, "no retake: it fires where an unlocated crossing fires");
    assert_eq!(times, unlocated_times, "the same accepted steps");
    let edge = period / 4.0;
    let after: Vec<f64> = times.iter().copied().filter(|t| *t > edge + 0.5 * TICK).collect();
    assert_eq!(fired[0], after[0], "it fires at the end of the step that reads the jump");
    assert!(fired[0] - edge > 1.0e-6, "a controller step after the edge, not a landing next to it: {:e}", fired[0] - edge);
}

/// A `CROSS` watch on a curved function of time: `e = 1 − exp(−t/tau) − vth`, concave, so the
/// secant of a bracket always lands past the crossing.
/// pins: tau=0, vth=1 (PARAM F64); up=2 (PRIVATE event)
/// watches: W0 CROSS "rise" -> up;  fn 0: reads [TIME, tau, vth] writes [W0]
struct Saturating;
static SAT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SAT: OnceLock<Module> = OnceLock::new();
const ONE_US: [u8; 8] = 1.0e-6f64.to_le_bytes();
const VTH: [u8; 8] = 0.999f64.to_le_bytes();

fn saturating_shape() -> ShapeRef {
    use piperine_abi::raw::{DEFAULT, ambient};
    let mut s = ShapeBuf::new();
    let param = |name, init| PinSpec { flags: pin_flag::HAS_INIT, init, ..PinSpec::discrete(name, role::PARAM, &F64) };
    let tau = s.pin(param("tau", &ONE_US));
    let vth = s.pin(param("vth", &VTH));
    let up = s.pin(PinSpec::event("up", role::PRIVATE));
    let w0 = s.watch(WatchSpec { name: "rise", origin: ORIGIN, kind: watch::CROSS, layout: &F64, event: up, ttol: DEFAULT, etol: DEFAULT });
    s.function(FunctionSpec {
        name: "e",
        origin: ORIGIN,
        reads: vec![
            Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE },
            Cell { kind: cell::PIN, index: tau, part: part::VALUE },
            Cell { kind: cell::PIN, index: vth, part: part::VALUE },
        ],
        writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Saturating {
    fn prototype() -> &'static Shape<'static> {
        SAT_SHAPE.get_or_init(saturating_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Saturating, Arc::clone(SAT_SHAPE.get_or_init(saturating_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (t, tau, vth) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, 1.0 - (-t / tau).exp() - vth);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn saturating() -> &'static Module {
    SAT.get_or_init(|| module::<Saturating>("Saturating"))
}

#[test]
fn a_curved_crossing_converges_from_both_sides() {
    // SCX-01 AC 1, the two-point secant (verification finding): e = 1 − exp(−t/1 µs) − 0.999
    // crosses at 1 µs·ln(1000) = 6.9078 µs and is concave, so the secant of a bracket that starts
    // at 0 always lands past the crossing and closes in from one side, a few parts in a thousand
    // per retake: with steps of 20 µs it would need far more than the protocol's 64 consecutive
    // retakes. The secant of the step's last two attempts past the crossing lands on either side
    // and converges in a handful. The crossing is located, with e < etol (1 µV) or within ttol
    // plus one tick.
    let mut b = CircuitBuilder::new("curved");
    let gnd = b.ground();
    let top = b.node("top");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", 1.0e3, &top, &gnd);
    b.device(saturating(), "sat", &[], &[]);
    let (_, seen) = run(b, 40.0e-6, 20.0e-6, |o| o.dt_max = 20.0e-6, Context::default());
    let fired = crossings(&seen, "sat.up");
    assert_eq!(fired.len(), 1, "{fired:?}");
    let truth = 1.0e-6 * 1000f64.ln();
    let e = 1.0 - (-fired[0] / 1.0e-6).exp() - 0.999;
    let late = fired[0] - truth;
    assert!(e > 0.0 && (e < ETOL || late <= TTOL + TICK), "e = {e:e}, late {late:e}");
}
