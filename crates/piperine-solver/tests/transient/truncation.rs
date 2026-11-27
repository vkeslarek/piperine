//! Scope: the truncation test and the step floor on the `ddt` states (HYG-07, HYG-09; spec Part V
//! §17.2 Resolution "Where rule 1 is measured"). A sine source drives a nonlinear capacitor
//! directly, so its node voltage is the source's and only the charge says where the step is
//! too large; `chgtol` is read; every expected current is the analytic `dq/dt`.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::partials::PartialsCheck;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, ambient, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_analog_solver::netlist::BranchIdentifier;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, TransientAnalysisOptions};

/// A capacitor whose charge is `q = c0·v + c1·v³` (`v = V(p, n)`).
/// pins: c0=0, c1=1 (PARAM F64); p=2, n=3 (PORT)      terms: T0 DDT{p, n} "q"
/// fn 0 "q": reads [p, n, c0, c1]  writes [T0]  partials (0←0) (0←1)
struct CubicCapacitor;
static CUBIC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CUBIC_MODULE: OnceLock<Module> = OnceLock::new();
const ONE_NANO: [u8; 8] = 1.0e-9_f64.to_le_bytes();

impl Device for CubicCapacitor {
    fn prototype() -> &'static Shape<'static> {
        CUBIC_SHAPE.get_or_init(TruncationCases::cubic_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CubicCapacitor, Arc::clone(CUBIC_SHAPE.get_or_init(TruncationCases::cubic_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c0, c1) = (cx.f64(0) - cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, c0 * v + c1 * v * v * v);
        let g = c0 + 3.0 * c1 * v * v;
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

const FREQUENCY: f64 = 1.0e5;

#[test]
fn a_source_driven_nonlinear_capacitor_meets_the_current_tolerance() {
    // HYG-07 AC 2 (amended 2026-11-17, ngspice CKTterr's criterion per Q1(a)): the source holds
    // the node at `sin(ωt)`, so the node basis sees only the sine's curvature; the charge basis
    // sees the cubic. At every accepted step the source current (minus the capacitor's, KCL at
    // the node) differs from the analytic `dq/dt = (c0 + 3·c1·v²)·dv/dt`, `v = sin(ωt)`, by at
    // most `trtol·tol` with `tol = max(abstol + reltol·|i|, reltol·max(|q|, chgtol)/h)`, ngspice's
    // own tolerance. Measured when written: the new basis peaks at 0.27 of that bound, the
    // node-potential basis at 2.28 (it fails).
    let tol = Context::default().tolerances;
    let (c0, c1) = (1.0e-10, 1.0e-9);
    let steps = TruncationCases::run(c0, c1, Context::default());
    assert!(steps.len() > 100, "{} steps", steps.len());
    let omega = 2.0 * PI * FREQUENCY;
    let charge = |t: f64| {
        let v = (omega * t).sin();
        c0 * v + c1 * v * v * v
    };
    for pair in steps.windows(2) {
        let ((t_before, _), (t, branch)) = (pair[0], pair[1]);
        let h = t - t_before;
        let v = (omega * t).sin();
        let analytic = (c0 + 3.0 * c1 * v * v) * omega * (omega * t).cos();
        let per_step = (tol.abstol + tol.reltol * analytic.abs()).max(tol.reltol * charge(t).abs().max(charge(t_before).abs()).max(tol.chgtol) / h);
        let bound = tol.trtol * per_step;
        let error = (-branch - analytic).abs();
        assert!(error <= bound, "t = {t:e}: source current {} against dq/dt {analytic} (error {error:e}, bound {bound:e})", -branch);
    }
}

#[test]
fn a_larger_chgtol_takes_fewer_steps() {
    // HYG-07 AC 3: every charge here is under the default `chgtol` (1e-14 C: c0·1 V = 1e-16 C),
    // so the charge floor `reltol·chgtol` sets the bound; raising `chgtol` by 10⁶ loosens it
    // 10⁶-fold and the run takes fewer steps.
    let default = Context::default();
    let mut loose = Context::default();
    loose.tolerances.chgtol = default.tolerances.chgtol * 1e6;
    let tight_steps = TruncationCases::run(1.0e-16, 1.0e-16, default).len();
    let loose_steps = TruncationCases::run(1.0e-16, 1.0e-16, loose).len();
    assert!(loose_steps < tight_steps, "{loose_steps} steps with chgtol raised, {tight_steps} with the default");
}

/// A voltage source that goes from 0 to 1 V at the time `at`, computed from `TIME` (nothing
/// announces the corner to the solver): a hard step when `width` is 0, else `½(1 + tanh((t − at)/width))`.
/// Pins: at=0, width=1 (PARAM F64); p=2, n=3 (PORT); f=4 (PRIVATE).
/// fn 0: reads [p, n, f, at, width, TIME] writes [T0, T1] partials (0←f) (1←p) (1←n)
struct StepSource;
static STEP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STEP_MODULE: OnceLock<Module> = OnceLock::new();
const STEP_AT: [u8; 8] = 1.03e-6_f64.to_le_bytes();

impl Device for StepSource {
    fn prototype() -> &'static Shape<'static> {
        STEP_SHAPE.get_or_init(TruncationCases::step_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((StepSource, Arc::clone(STEP_SHAPE.get_or_init(TruncationCases::step_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, at, width, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4), cx.f64(5));
        let wanted = if width > 0.0 { 0.5 * (1.0 + ((t - at) / width).tanh()) } else if t >= at { 1.0 } else { 0.0 };
        cx.set(0, f);
        cx.set(1, vp - vn - wanted);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn an_unannounced_corner_is_accepted_at_the_floor_and_counted() {
    // HYG-09 AC 1 (amended 2026-11-18, ngspice's floor rule): a 1 V step at 1.03 us that nothing
    // announces, across 1 nF, leaves a step at the 1 ns floor whose charge still jumps by 1 nC.
    // Nothing smaller is tried and ngspice completes this circuit, so the run completes: the step
    // at the floor is accepted, counted in `floor_accepts`, and the run reaches its stop.
    let result = TruncationCases::run_corner(1.03e-6, 0.0, 1.0e-9, 2.0e-6).expect("the corner is accepted at the floor, not failed");
    assert!(result.stats.floor_accepts >= 1, "the step at the floor was counted: {:?}", result.stats);
    assert_eq!(result.last().expect("a last step").time(), 2.0e-6, "the run reaches its stop");
}

#[test]
fn the_default_floor_takes_the_same_corner_too() {
    // The corner of the verifier's evidence: the default `dt_min` (1e-15) and a hard step.
    let result = TruncationCases::run_corner(1.03e-6, 0.0, 1.0e-15, 2.0e-6).expect("the corner completes with the default floor");
    assert!(result.stats.floor_accepts >= 1, "{:?}", result.stats);
    assert_eq!(result.last().expect("a last step").time(), 2.0e-6);
}

#[test]
fn a_proposal_that_lands_on_the_floor_is_accepted_not_failed() {
    // HYG-09 AC 2 (amended): a 5 ns `tanh` corner with a 1 ns floor. The controller's proposal
    // after an accepted step lands on the floor before the corner; that attempt (no larger
    // attempt failed before it) is taken, not failed: the run completes.
    let result = TruncationCases::run_corner(1.03e-6, 5.0e-9, 1.0e-9, 2.0e-6).expect("the tanh corner completes");
    assert_eq!(result.last().expect("a last step").time(), 2.0e-6);
}

#[test]
fn a_corner_at_the_stop_time_is_accepted_at_the_floor() {
    // Edge case 6 (amended): the corner is at the stop time, so the step at the floor is the one
    // that ends there; it is accepted and the run ends at that time.
    let stop = 1.03e-6;
    let result = TruncationCases::run_corner(stop, 0.0, 1.0e-9, stop).expect("the step at the floor is accepted");
    assert_eq!(result.last().expect("a last step").time(), stop);
    assert!(result.stats.floor_accepts >= 1, "{:?}", result.stats);
}

/// A conductance of 1 mS that faults the first time it is evaluated after `FAULT_AFTER`: one
/// Newton failure, at whatever step reaches it first. `FAULTED` says it happened.
/// pins: p=0, n=1 (PORT); fn 0: reads [p, n, TIME] writes [T0] partials (0<-0) (0<-1)
struct FaultOnce;
static FAULT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FAULT_MODULE: OnceLock<Module> = OnceLock::new();
const FAULT_AFTER: f64 = 5.0e-7;

thread_local! {
    static FAULTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Device for FaultOnce {
    fn prototype() -> &'static Shape<'static> {
        FAULT_SHAPE.get_or_init(TruncationCases::fault_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FaultOnce, Arc::clone(FAULT_SHAPE.get_or_init(TruncationCases::fault_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.f64(2) > FAULT_AFTER && !FAULTED.with(|faulted| faulted.replace(true)) {
            return Err(Fault::new(7, "once"));
        }
        let g = 1.0e-3;
        cx.set(0, g * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn a_failed_step_that_falls_to_the_floor_is_retaken_there_once() {
    // HYG-09 AC 2 (amended, ngspice's `olddelta` rule): the attempt of 100 ns fails Newton once;
    // an eighth of it is under the 50 ns floor, so the step is retaken at the floor, counted
    // once, and the retry (which converges) lets the run complete.
    let result = TruncationCases::run_once_faulting(5.0e-8).expect("the retry at the floor converges");
    assert_eq!(result.stats.dt_min_floor_hits, 1, "one retake at the floor: {:?}", result.stats);
    assert!(result.stats.steps_rejected >= 1, "{:?}", result.stats);
    assert_eq!(result.last().expect("a last step").time(), 1.0e-6);
}

#[test]
fn a_failure_at_an_attempt_already_at_the_floor_is_not_retried() {
    // The controller's proposal reaches the floor with no larger attempt before it (the floor is
    // the 100 ns step itself): the one Newton failure ends the run, as ngspice's "Timestep too
    // small" does (`olddelta` is not above `delmin`). A retry would have converged, so the failure
    // is the proof that none was made. The failure keeps its own error (a device fault, which has
    // no run code); a Newton exhaustion at the floor is `E0508` (failure_rules.rs).
    let error = TruncationCases::run_once_faulting(1.0e-7).map(|_| ()).expect_err("a failure at the floor ends the run");
    let text = error.to_string();
    assert!(text.contains("once") && text.contains("at t = 6"), "{text}");
}

#[test]
fn its_corner_source_passes_the_partials_verifier() {
    // `StepSource`'s partials, away from its corner (TIME held at 0).
    PartialsCheck::verify_every_function_with("StepSource", TruncationCases::step_source(), &[(0, 5, 0.0)]);
}

#[test]
fn its_device_passes_the_partials_verifier() {
    PartialsCheck::verify_every_function("CubicCapacitor", TruncationCases::cubic_capacitor());
}

#[test]
fn its_once_faulting_device_passes_the_partials_verifier() {
    // `FaultOnce`'s partials, before it faults (TIME, read 2, held at 0).
    PartialsCheck::verify_every_function_with("FaultOnce", FAULT_MODULE.get_or_init(|| module::<FaultOnce>("FaultOnce")), &[(0, 2, 0.0)]);
}

/// The helpers of the truncation tests.
pub struct TruncationCases;

impl TruncationCases {
    /// The pin helper `pin`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The term helper `term`.
    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The real param helper `real_param`.
    fn real_param(name: &'static str, default: &'static [u8; 8]) -> PinSpec<'static> {
        PinSpec { flags: pin_flag::HAS_INIT, init: default, ..PinSpec::discrete(name, role::PARAM, &F64) }
    }

    /// The cubic shape helper `cubic_shape`.
    fn cubic_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(TruncationCases::real_param("c0", &ONE_NANO));
        s.pin(TruncationCases::real_param("c1", &ONE_NANO));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "q", origin: "truncation.rs", op: op::DDT, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "q",
            origin: "truncation.rs",
            reads: vec![TruncationCases::pin(p), TruncationCases::pin(n), TruncationCases::pin(0), TruncationCases::pin(1)],
            writes: vec![TruncationCases::term(t0)],
            partials: vec![
                piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
                piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The cubic capacitor helper `cubic_capacitor`.
    fn cubic_capacitor() -> &'static Module {
        CUBIC_MODULE.get_or_init(|| module::<CubicCapacitor>("CubicCapacitor"))
    }

    /// A 1 V sine of `FREQUENCY` across the cubic capacitor `c0`, `c1`, run for one period with the
    /// step free to grow (`dt_max` = the period) under `context`: every accepted step as
    /// `(time, source branch current)`.
    fn run(c0: f64, c1: f64, context: Context) -> Vec<(f64, f64)> {
        let mut b = CircuitBuilder::new("Top");
        let gnd = b.ground();
        let top = b.node("top");
        let nodes = [Port::Node(top.clone()), Port::Node(gnd)];
        b.device(circuit::SineSource::module(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(FREQUENCY))], &nodes);
        b.device(TruncationCases::cubic_capacitor(), "c1", &[("c0", ParamValue::Real(c0)), ("c1", ParamValue::Real(c1))], &nodes);
        let period = 1.0 / FREQUENCY;
        let mut options = TransientAnalysisOptions::new(period, period / 1000.0);
        options.dt_max = period;
        let mut solver = piperine_solver::prelude::Solver::new(b.build().expect("build")).with_context(context).with_tran_opts(options).build();
        let result = solver.tran().expect("transient").solve().expect("the transient solves");
        let source = BranchIdentifier::new("v1", "f");
        result.iter().map(|step| (step.time(), step.get_branch(source.clone()).expect("the source current"))).collect()
    }

    /// The step shape helper `step_shape`.
    fn step_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(TruncationCases::real_param("at", &STEP_AT));
        s.pin(TruncationCases::real_param("width", &[0; 8]));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let flow = |name, plus, minus| TermSpec { name, origin: "truncation.rs", op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT };
        let t0 = s.term(flow("i", p, n));
        let t1 = s.term(flow("eq", f, GROUND));
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        s.function(FunctionSpec {
            name: "v",
            origin: "truncation.rs",
            reads: vec![TruncationCases::pin(p), TruncationCases::pin(n), TruncationCases::pin(f), TruncationCases::pin(0), TruncationCases::pin(1), time],
            writes: vec![TruncationCases::term(t0), TruncationCases::term(t1)],
            partials: vec![
                piperine_abi::raw::Partial { write: 0, to: 0, read: 2, at: 0 },
                piperine_abi::raw::Partial { write: 1, to: 0, read: 0, at: 0 },
                piperine_abi::raw::Partial { write: 1, to: 0, read: 1, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The step source helper `step_source`.
    fn step_source() -> &'static Module {
        STEP_MODULE.get_or_init(|| module::<StepSource>("StepSource"))
    }

    /// The source across a 1 nF capacitor, run to `stop` with the step cap at 100 ns.
    fn run_corner(at: f64, width: f64, dt_min: f64, stop: f64) -> Result<piperine_solver::prelude::TransientAnalysisResult, piperine_solver::abi::Error> {
        let mut b = CircuitBuilder::new("Top");
        let gnd = b.ground();
        let top = b.node("top");
        let nodes = [Port::Node(top), Port::Node(gnd)];
        b.device(TruncationCases::step_source(), "v1", &[("at", ParamValue::Real(at)), ("width", ParamValue::Real(width))], &nodes);
        b.device(circuit::Capacitor::module(), "c1", &[("c", ParamValue::Real(1.0e-9))], &nodes);
        let mut options = TransientAnalysisOptions::new(stop, 1.0e-7);
        options.dt_min = dt_min;
        options.dt_max = 1.0e-7;
        let mut solver = piperine_solver::prelude::Solver::new(b.build().expect("build")).with_tran_opts(options).build();
        solver.tran().and_then(|mut t| t.solve())
    }

    /// The fault shape helper `fault_shape`.
    fn fault_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: "truncation.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        s.function(FunctionSpec {
            name: "i",
            origin: "truncation.rs",
            reads: vec![TruncationCases::pin(p), TruncationCases::pin(n), time],
            writes: vec![TruncationCases::term(t0)],
            partials: vec![piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 }, piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A 1 kHz sine across 1 nF and the once-faulting conductance, run to 1 us at steps of 100 ns
    /// (the cap and the first step) with the floor at `dt_min`.
    fn run_once_faulting(dt_min: f64) -> Result<piperine_solver::prelude::TransientAnalysisResult, piperine_solver::abi::Error> {
        FAULTED.with(|faulted| faulted.set(false));
        let mut b = CircuitBuilder::new("Top");
        let gnd = b.ground();
        let top = b.node("top");
        let nodes = [Port::Node(top), Port::Node(gnd)];
        b.device(circuit::SineSource::module(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(1.0e3))], &nodes);
        b.device(circuit::Capacitor::module(), "c1", &[("c", ParamValue::Real(1.0e-9))], &nodes);
        b.device(FAULT_MODULE.get_or_init(|| module::<FaultOnce>("FaultOnce")), "g1", &[], &nodes);
        let mut options = TransientAnalysisOptions::new(1.0e-6, 1.0e-7);
        options.dt_min = dt_min;
        options.dt_max = 1.0e-7;
        let mut solver = piperine_solver::prelude::Solver::new(b.build().expect("build")).with_tran_opts(options).build();
        let outcome = solver.tran().and_then(|mut t| t.solve());
        assert!(FAULTED.with(|faulted| faulted.get()), "the Newton failure happened");
        outcome
    }
}
