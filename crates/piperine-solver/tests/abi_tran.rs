//! The transient driver on devices (SAI-11, SAI-01): an RC step reads
//! `1 - e^-1` with the literal and tolerance of `helper_devices.rs`; a
//! continuous pin's `init` starts Newton and is held through a transient's
//! operating point only; a rejected stage leaves the term histories as they
//! were; a stop off the tick grid is reached exactly once, by a step that integrates; the
//! step whose target is the stop ends at the stop bit for bit.

use std::sync::{Arc, OnceLock};

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use abi_mixed::{dac, timer};
use abi_devices::{Counting, add_capacitor, add_resistor, add_vsrc, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, ambient, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::abi::InitialValue;
use piperine_solver::prelude::{CircuitBuilder, Context, TransientAnalysisOptions};

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}

fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: "abi_tran.rs", op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
}

// ---------------------------------------------------------------------------
// ChargedC: the capacitor of `@initial { V(p, n) <- 3.0 }`
// ---------------------------------------------------------------------------

/// pins: c=0 (PARAM F64, 1e-6); p=1 (PORT, init 3.0); n=2 (PORT)
/// terms: T0 DDT{p, n}; fn 0: reads [p, n, c] writes [T0]
struct ChargedC;
static CHARGED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CHARGED_MODULE: OnceLock<Module> = OnceLock::new();
const C_DEFAULT: [u8; 8] = 1.0e-6_f64.to_le_bytes();
const INITIAL_VOLTAGE: f64 = 3.0;
const V_INIT: [u8; 8] = INITIAL_VOLTAGE.to_le_bytes();

fn charged_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &C_DEFAULT, ..PinSpec::discrete("c", role::PARAM, &F64) });
    let p = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &V_INIT, ..PinSpec::continuous("p", role::PORT) });
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("q", op::DDT, p, n));
    s.function(FunctionSpec {
        name: "q",
        origin: "abi_tran.rs",
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![
            piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
            piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
        ],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for ChargedC {
    fn prototype() -> &'static Shape<'static> {
        CHARGED_SHAPE.get_or_init(charged_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ChargedC, Arc::clone(CHARGED_SHAPE.get_or_init(charged_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let c = cx.f64(2);
        cx.set(0, c * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[c, -c]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn charged_c() -> &'static Module {
    CHARGED_MODULE.get_or_init(|| module::<ChargedC>("ChargedC"))
}

// ---------------------------------------------------------------------------
// FlakyC: the same capacitor, whose function faults late in time
// ---------------------------------------------------------------------------

/// `ChargedC` plus a read of `TIME`: the function returns a `FAULT` once the
/// time passes `FLAKY_AFTER`, so a step's second stage fails after its first
/// converged and pushed a row of every history.
/// fn 0: reads [p, n, c, TIME] writes [T0]
struct FlakyC;
static FLAKY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAKY_MODULE: OnceLock<Module> = OnceLock::new();
const FLAKY_AFTER: f64 = 8.0e-6;

fn flaky_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &C_DEFAULT, ..PinSpec::discrete("c", role::PARAM, &F64) });
    let p = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &V_INIT, ..PinSpec::continuous("p", role::PORT) });
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("q", op::DDT, p, n));
    let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
    s.function(FunctionSpec {
        name: "q",
        origin: "abi_tran.rs",
        reads: vec![pin(p), pin(n), pin(0), time],
        writes: vec![term(t0)],
        partials: vec![
            piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
            piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
        ],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for FlakyC {
    fn prototype() -> &'static Shape<'static> {
        FLAKY_SHAPE.get_or_init(flaky_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlakyC, Arc::clone(FLAKY_SHAPE.get_or_init(flaky_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.f64(3) > FLAKY_AFTER {
            return Err(Fault::new(7, "late"));
        }
        let c = cx.f64(2);
        cx.set(0, c * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[c, -c]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn flaky_c() -> &'static Module {
    FLAKY_MODULE.get_or_init(|| module::<FlakyC>("FlakyC"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn rc_step_reads_one_minus_exp_on_devices() {
    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c;
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", r, &top, &out);
    add_capacitor(&mut b, "c1", c, &out, &gnd);
    let mut circuit = b.build().expect("build");

    let mut solver = circuit
        .transient(TransientAnalysisOptions::new(tau, tau / 100.0), Context::default())
        .expect("tran");
    solver.apply_initial_conditions(vec![InitialValue { reference: out.clone(), value: 0.0 }]);
    let result = solver.solve().expect("solve");
    let last = result.last().expect("steps");

    let v = last.get_node(&node_id(&out)).expect("v(out)");
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((last.time() - tau).abs() < 1.0e-12, "stopped at t = {}", last.time());
    assert!((v - analytic).abs() < 1.0e-3, "v(rc) = {v} vs 1 - 1/e = {analytic}");
}

/// The RC of `rc_step_reads_one_minus_exp_on_devices` with the corner of the `ac_rc`/`tran_rc`
/// twins: `tau = 1 kohm * 1.5915494309189535e-9 F`, 1591549430.92 ticks of the default 1 fs grid.
fn rc_to(stop: f64) -> (f64, Vec<(f64, f64)>) {
    let options = TransientAnalysisOptions::new(stop, stop / 400.0);
    rc_with(options)
}

/// `rc_to` under the given options.
fn rc_with(options: TransientAnalysisOptions) -> (f64, Vec<(f64, f64)>) {
    let (r, c) = (1.0e3, 1.5915494309189535e-9);
    let tau = r * c;
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", r, &top, &out);
    add_capacitor(&mut b, "c1", c, &out, &gnd);
    let mut circuit = b.build().expect("build");
    let mut solver = circuit.transient(options, Context::default()).expect("tran");
    solver.apply_initial_conditions(vec![InitialValue { reference: out.clone(), value: 0.0 }]);
    let result = solver.solve().expect("solve");
    let id = node_id(&out);
    (tau, result.iter().map(|s| (s.time(), s.get_node(&id).expect("v(out)"))).collect())
}

/// Times strictly increase: no record repeats a time (a zero-length step records one).
fn assert_strictly_increasing(steps: &[(f64, f64)]) {
    for pair in steps.windows(2) {
        assert!(pair[1].0 > pair[0].0, "records at {:e} then {:e}: {steps:?}", pair[0].0, pair[1].0);
    }
}

#[test]
fn a_stop_off_the_tick_grid_is_reached_once_by_an_integrating_step() {
    let tau: f64 = 1.0e3 * 1.5915494309189535e-9;
    let fraction = (tau / 1.0e-15).fract();
    assert!(fraction > 0.5, "the stop must round up on the 1 fs grid: {fraction}");
    let (_, steps) = rc_to(tau);
    assert_strictly_increasing(&steps);
    let (t, v) = *steps.last().expect("steps");
    assert!((t - tau).abs() < 1.0e-12 * tau, "stopped at t = {t:e}");
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((v - analytic).abs() < 1.0e-3, "v(out) at the stop = {v} vs 1 - 1/e = {analytic}");
    let at_stop = steps.iter().filter(|(s, _)| (s - tau).abs() < 1.0e-12 * tau).count();
    assert_eq!(at_stop, 1, "one record at the stop: {:?}", &steps[steps.len().saturating_sub(3)..]);
}

#[test]
fn a_stop_on_the_tick_grid_ends_on_its_own_step() {
    let stop = 1.6e-6;
    let (tau, steps) = rc_to(stop);
    assert_strictly_increasing(&steps);
    let (t, v) = *steps.last().expect("steps");
    assert!((t - stop).abs() < 1.0e-12 * stop, "stopped at t = {t:e}");
    let analytic = 1.0 - (-stop / tau).exp();
    assert!((v - analytic).abs() < 1.0e-3, "v(out) at the stop = {v} vs {analytic}");
}

/// The step whose target is the stop ends at the stop bit for bit, on the tick grid or off it,
/// and records it once: a non-landing step ends at the controller's proposal only short of the stop.
#[test]
fn the_stop_is_reached_bit_exactly_and_recorded_once() {
    let off_grid: f64 = 1.0e3 * 1.5915494309189535e-9;
    // The `tran_rc` twin's run: 10 us with the step capped at 10 ns.
    let mut capped = TransientAnalysisOptions::new(1.0e-5, 1.0e-8);
    capped.dt_max = 1.0e-8;
    let mut off_grid_capped = TransientAnalysisOptions::new(off_grid * 6.0, 1.0e-8);
    off_grid_capped.dt_max = 1.0e-8;
    let runs = [
        TransientAnalysisOptions::new(off_grid, off_grid / 400.0),
        TransientAnalysisOptions::new(1.6e-6, 1.6e-6 / 400.0),
        capped,
        off_grid_capped,
    ];
    for options in runs {
        let stop = options.stop_time;
        let (_, steps) = rc_with(options);
        assert_strictly_increasing(&steps);
        let (t, _) = *steps.last().expect("steps");
        assert_eq!(t.to_bits(), stop.to_bits(), "stop {stop:e}: the last record is at t = {t:e}");
        let at_stop = steps.iter().filter(|(s, _)| *s == stop).count();
        assert_eq!(at_stop, 1, "stop {stop:e}: one record at the stop");
    }
}

/// A capacitor of `module` (seeded to `INITIAL_VOLTAGE`) over a resistor to
/// ground: `v(out)` decays from its `init`.
fn discharge_of(
    module: &'static Module,
) -> (piperine_solver::prelude::CircuitInstance, piperine_solver::prelude::AnalogReference) {
    let mut b = CircuitBuilder::new("discharge");
    let gnd = b.ground();
    let out = b.node("out");
    add_resistor(&mut b, "r1", 1.0e3, &out, &gnd);
    b.device(module, "c1", &[("c", ParamValue::Real(1.0e-6))], &[Port::Node(out.clone()), Port::Node(gnd)]);
    (b.build().expect("build"), out)
}

fn discharge() -> (piperine_solver::prelude::CircuitInstance, piperine_solver::prelude::AnalogReference) {
    discharge_of(charged_c())
}

#[test]
fn a_continuous_init_starts_newton_and_is_held_through_the_operating_point() {
    let tau = 1.0e3 * 1.0e-6;
    let (mut circuit, out) = discharge();

    // A plain operating point has no source: the `init` is not a hold there.
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    assert!(result.get_node(&node_id(&out)).expect("v(out)").abs() < 1.0e-12);

    let result = circuit
        .transient(TransientAnalysisOptions::new(tau, tau / 100.0), Context::default())
        .expect("tran")
        .solve()
        .expect("solve");
    // The transient's operating point starts at, and is held to, the seed ...
    let first = result.get(0).expect("the operating point").get_node(&node_id(&out)).expect("v(out)");
    assert!((first - INITIAL_VOLTAGE).abs() < 1.0e-9, "v(out) at t = 0 is {first}");
    // ... and the hold is released afterwards: the capacitor discharges.
    let last = result.last().expect("steps").get_node(&node_id(&out)).expect("v(out)");
    let analytic = INITIAL_VOLTAGE * (-1.0_f64).exp();
    assert!((last - analytic).abs() < 3.0e-3, "v(out) = {last} vs {analytic}");
}

#[test]
fn a_rejected_stage_restores_the_term_history() {
    let tau = 1.0e3 * 1.0e-6;
    let dt = tau / 100.0;

    // The histories after the operating point only: the step budget is spent before any step.
    let (mut control, _) = discharge_of(flaky_c());
    {
        let options = TransientAnalysisOptions::new(tau, dt).with_max_steps(0);
        let mut solver = control.transient(options, Context::default()).expect("tran");
        assert!(solver.solve().is_err(), "no step may run");
    }
    let at_start = control.bound().expect("bound").analog.snapshot();

    // The first step's first stage converges and pushes a row of every history; its second stage
    // (t = dt, past the time the device faults at) fails, and at `dt_min = dt` the run ends there.
    // The rejected attempt must leave nothing of that first stage behind.
    let (mut failing, _) = discharge_of(flaky_c());
    {
        let options = TransientAnalysisOptions::new(tau, dt).with_dt_min(dt);
        let mut solver = failing.transient(options, Context::default()).expect("tran");
        let err = solver.solve().err().expect("the second stage faults").to_string();
        assert!(err.contains("late"), "{err}");
    }
    assert_eq!(failing.bound().expect("bound").analog.snapshot(), at_start);

    // Accepted steps do move the histories, so the comparison above has teeth.
    let (mut running, _) = discharge();
    running.transient(TransientAnalysisOptions::new(tau, dt), Context::default()).expect("tran").solve().expect("solve");
    let (mut still, _) = discharge();
    {
        let options = TransientAnalysisOptions::new(tau, dt).with_max_steps(0);
        let _ = still.transient(options, Context::default()).expect("tran").solve();
    }
    assert_ne!(running.bound().expect("bound").analog.snapshot(), still.bound().expect("bound").analog.snapshot());
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("ChargedC", CHARGED_MODULE.get_or_init(|| module::<ChargedC>("ChargedC")));
    // TIME (read 3) before FLAKY_AFTER, where the fixture does not fault.
    partials::verify_every_function_with("FlakyC", FLAKY_MODULE.get_or_init(|| module::<FlakyC>("FlakyC")), &[(0, 3, 0.0)]);
}

#[test]
fn the_transient_driver_iterates_with_the_plans_strategy() {
    // Every Newton solve of a step goes through the strategy of the plan the driver was given:
    // one damp_update per iteration, so the count equals the run's iterations.
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (r, c) = (1.0e3, 1.0e-6);
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    add_resistor(&mut b, "r1", r, &top, &out);
    add_capacitor(&mut b, "c1", c, &out, &gnd);
    let mut circuit = b.build().expect("build");
    let plan = piperine_analog_solver::convergence::ConvergencePlan::default().with_newton(Box::new(Counting(Arc::clone(&calls))));
    let mut solver = circuit
        .transient(TransientAnalysisOptions::new(r * c, r * c / 100.0), Context::default())
        .expect("tran")
        ;
    solver.set_plan(plan);
    solver.apply_initial_conditions(vec![InitialValue { reference: out.clone(), value: 0.0 }]);
    let result = solver.solve().expect("solve");
    let seen = calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(result.len() > 10, "the run took steps");
    assert!(seen >= 2 * (result.len() - 1), "two Newton solves per accepted step went through the strategy: {seen} calls");
}

/// A timer on `code` toggling every `half`, a DAC of 1 V on it driving `o`, and `o` charging 1 nF
/// through 1 kohm.
fn jump_into_rc(b: &mut CircuitBuilder, name: &str, half: f64) {
    let gnd = b.ground();
    let o = b.node(&format!("{name}_o"));
    let c = b.node(&format!("{name}_c"));
    let code = || Port::Net(format!("{name}_code"));
    b.device(timer(), &format!("clk_{name}"), &[("half", ParamValue::Real(half))], &[code()]);
    b.device(dac(), &format!("dac_{name}"), &[("vref", ParamValue::Real(1.0))], &[code(), Port::Node(o.clone())]);
    add_resistor(b, &format!("r_{name}"), 1.0e3, &o, &c);
    add_capacitor(b, &format!("c_{name}"), 1.0e-9, &c, &gnd);
}

/// The step after each edge in `edges` is at most a tenth of the step that landed on the edge and
/// of the gap to the next edge (HYG-11 AC 1); returns how many edges were checked.
fn assert_jump_steps_are_capped(times: &[f64], edges: &[f64]) -> usize {
    let mut checked = 0;
    for (n, edge) in edges.iter().enumerate() {
        let Some(k) = times.iter().position(|t| (t - edge).abs() < 1.0e-15) else { continue };
        if k == 0 || k + 1 >= times.len() {
            continue;
        }
        let (landing, after) = (times[k] - times[k - 1], times[k + 1] - times[k]);
        let gap = edges.get(n + 1).map_or(f64::INFINITY, |next| next - edge);
        let bound = 0.1 * landing.min(gap);
        assert!(after <= bound * (1.0 + 1.0e-9) + 1.0e-15, "after the edge at {edge:e}: step {after:e} exceeds a tenth of the landing step {landing:e} and of the gap {gap:e}");
        assert!(after >= 1.0e-15, "the step is floored at dt_min, not zero: {after:e}");
        checked += 1;
    }
    checked
}

#[test]
fn the_step_after_a_jump_is_capped_at_a_tenth_of_the_last() {
    // One timer edges every microsecond; a controller that has grown its step (the RC is at rest
    // between edges, the ceiling is 100 ns) would otherwise take a full step into the jump.
    let mut b = CircuitBuilder::new("jumps");
    jump_into_rc(&mut b, "a", 1.0e-6);
    let mut circuit = b.build().expect("build");
    let options = TransientAnalysisOptions::new(4.5e-6, 1.0e-7);
    let result = circuit.transient(options, Context::default()).expect("tran").solve().expect("solve");
    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    let edges = [1.0e-6, 2.0e-6, 3.0e-6, 4.0e-6];
    assert_eq!(assert_jump_steps_are_capped(&times, &edges), 4, "every edge was a landing: {times:?}");
}

#[test]
fn two_jumps_in_one_restart_step_restart_from_the_later() {
    // Two timers whose edges are 0.25 us apart: the step after the later edge is capped from the
    // step that landed on it, not from the earlier jump's.
    let mut b = CircuitBuilder::new("two jumps");
    jump_into_rc(&mut b, "a", 1.0e-6);
    jump_into_rc(&mut b, "b", 1.25e-6);
    let mut circuit = b.build().expect("build");
    let options = TransientAnalysisOptions::new(3.0e-6, 1.0e-7);
    let result = circuit.transient(options, Context::default()).expect("tran").solve().expect("solve");
    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    let edges = [1.0e-6, 1.25e-6, 2.0e-6, 2.5e-6];
    assert_eq!(assert_jump_steps_are_capped(&times, &edges), 4, "every edge was a landing: {times:?}");
}
