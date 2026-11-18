//! Scope: the truncation test and the step floor on the `ddt` states (HYG-07, HYG-09; spec Part V
//! §17.2 Resolution "Where rule 1 is measured"). A sine source drives a nonlinear capacitor
//! directly, so its node voltage is the source's and only the charge says where the step is
//! too large; `chgtol` is read; every expected current is the analytic `dq/dt`.

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

use abi_devices::vsine;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, ambient, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_analog_solver::error::RunCode;
use piperine_analog_solver::netlist::BranchIdentifier;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, TransientAnalysisOptions};

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}

fn real_param(name: &'static str, default: &'static [u8; 8]) -> PinSpec<'static> {
    PinSpec { flags: pin_flag::HAS_INIT, init: default, ..PinSpec::discrete(name, role::PARAM, &F64) }
}

/// A capacitor whose charge is `q = c0·v + c1·v³` (`v = V(p, n)`).
/// pins: c0=0, c1=1 (PARAM F64); p=2, n=3 (PORT)      terms: T0 DDT{p, n} "q"
/// fn 0 "q": reads [p, n, c0, c1]  writes [T0]  partials (0←0) (0←1)
struct CubicCapacitor;
static CUBIC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CUBIC_MODULE: OnceLock<Module> = OnceLock::new();
const ONE_NANO: [u8; 8] = 1.0e-9_f64.to_le_bytes();

fn cubic_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("c0", &ONE_NANO));
    s.pin(real_param("c1", &ONE_NANO));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { name: "q", origin: "truncation.rs", op: op::DDT, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "q",
        origin: "truncation.rs",
        reads: vec![pin(p), pin(n), pin(0), pin(1)],
        writes: vec![term(t0)],
        partials: vec![
            piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
            piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
        ],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for CubicCapacitor {
    fn prototype() -> &'static Shape<'static> {
        CUBIC_SHAPE.get_or_init(cubic_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CubicCapacitor, Arc::clone(CUBIC_SHAPE.get_or_init(cubic_shape))))
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

fn cubic_capacitor() -> &'static Module {
    CUBIC_MODULE.get_or_init(|| module::<CubicCapacitor>("CubicCapacitor"))
}

const FREQUENCY: f64 = 1.0e5;

/// A 1 V sine of `FREQUENCY` across the cubic capacitor `c0`, `c1`, run for one period with the
/// step free to grow (`dt_max` = the period) under `context`: every accepted step as
/// `(time, source branch current)`.
fn run(c0: f64, c1: f64, context: Context) -> Vec<(f64, f64)> {
    let mut b = CircuitBuilder::new("Top");
    let gnd = b.ground();
    let top = b.node("top");
    let nodes = [Port::Node(top.clone()), Port::Node(gnd)];
    b.device(vsine(), "v1", &[("amp", ParamValue::Real(1.0)), ("freq", ParamValue::Real(FREQUENCY))], &nodes);
    b.device(cubic_capacitor(), "c1", &[("c0", ParamValue::Real(c0)), ("c1", ParamValue::Real(c1))], &nodes);
    let period = 1.0 / FREQUENCY;
    let mut options = TransientAnalysisOptions::new(period, period / 1000.0);
    options.dt_max = period;
    let mut solver = piperine_solver::prelude::Solver::new(b.build().expect("build")).with_context(context).with_tran_opts(options).build();
    let result = solver.tran().expect("transient").solve().expect("the transient solves");
    let source = BranchIdentifier::new("v1", "f");
    result.iter().map(|step| (step.time(), step.get_branch(source.clone()).expect("the source current"))).collect()
}

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
    let steps = run(c0, c1, Context::default());
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
    let tight_steps = run(1.0e-16, 1.0e-16, default).len();
    let loose_steps = run(1.0e-16, 1.0e-16, loose).len();
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

fn step_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("at", &STEP_AT));
    s.pin(real_param("width", &[0; 8]));
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
        reads: vec![pin(p), pin(n), pin(f), pin(0), pin(1), time],
        writes: vec![term(t0), term(t1)],
        partials: vec![
            piperine_abi::raw::Partial { write: 0, to: 0, read: 2, at: 0 },
            piperine_abi::raw::Partial { write: 1, to: 0, read: 0, at: 0 },
            piperine_abi::raw::Partial { write: 1, to: 0, read: 1, at: 0 },
        ],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for StepSource {
    fn prototype() -> &'static Shape<'static> {
        STEP_SHAPE.get_or_init(step_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((StepSource, Arc::clone(STEP_SHAPE.get_or_init(step_shape))))
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

fn step_source() -> &'static Module {
    STEP_MODULE.get_or_init(|| module::<StepSource>("StepSource"))
}

/// The source across a 1 nF capacitor, run to `stop` with the step cap at 100 ns.
fn run_corner(at: f64, width: f64, dt_min: f64, stop: f64) -> Result<piperine_solver::prelude::TransientAnalysisResult, piperine_solver::abi::Error> {
    let mut b = CircuitBuilder::new("Top");
    let gnd = b.ground();
    let top = b.node("top");
    let nodes = [Port::Node(top), Port::Node(gnd)];
    b.device(step_source(), "v1", &[("at", ParamValue::Real(at)), ("width", ParamValue::Real(width))], &nodes);
    b.device(abi_devices::capacitor(), "c1", &[("c", ParamValue::Real(1.0e-9))], &nodes);
    let mut options = TransientAnalysisOptions::new(stop, 1.0e-7);
    options.dt_min = dt_min;
    options.dt_max = 1.0e-7;
    let mut solver = piperine_solver::prelude::Solver::new(b.build().expect("build")).with_tran_opts(options).build();
    solver.tran().and_then(|mut t| t.solve())
}

#[test]
fn an_unannounced_corner_fails_e0508_at_the_floor() {
    // HYG-09 AC 1: a 1 V step at 1.03 us that nothing announces, across 1 nF, leaves a step at the
    // 1 ns floor whose charge still jumps by 1 nC: the run fails E0508, naming the time (the
    // step that contains 1.03 us), and accepts no step.
    let error = run_corner(1.03e-6, 0.0, 1.0e-9, 2.0e-6).map(|_| ()).expect_err("the step at the floor must fail, not be accepted");
    assert_eq!(error.code(), Some(RunCode::E0508), "{error}");
    let text = error.to_string();
    assert!(text.contains("E0508") && text.contains("at t ="), "{text}");
    let time = text.split("at t = ").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<f64>().ok()).expect("the time");
    assert!((1.03e-6..=1.03e-6 + 1.5e-9).contains(&time), "the failing step contains 1.03 us: {time:e} ({text})");
}

#[test]
fn the_floor_is_retried_once_before_failing() {
    // HYG-09 AC 2: the step that first falls to the 1 ns floor is retaken there once (counted in
    // `dt_min_floor_hits`) and only the failure of that retry ends the run: the error of the hard
    // corner is reported as failing after the retry at the floor, not at a first floor-sized step.
    let error = run_corner(1.03e-6, 0.0, 1.0e-9, 2.0e-6).map(|_| ()).expect_err("the retry at the floor fails");
    assert_eq!(error.code(), Some(RunCode::E0508), "{error}");
    assert!(error.to_string().contains("after the retry at the floor"), "{error}");
}

#[test]
fn the_floor_at_the_stop_time_names_that_time() {
    // Edge case 6: the corner is at the stop time, so the step that fails at the floor is the one
    // that ends there, and the error names that time.
    let stop = 1.03e-6;
    let error = run_corner(stop, 0.0, 1.0e-9, stop).map(|_| ()).expect_err("the step at the floor fails");
    assert_eq!(error.code(), Some(RunCode::E0508), "{error}");
    let text = error.to_string();
    let time = text.split("at t = ").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<f64>().ok()).expect("the time");
    assert!((time - stop).abs() < 1e-12, "the error names the stop time {stop:e}: {time:e} ({text})");
}

#[test]
fn its_corner_source_passes_the_partials_verifier() {
    // `StepSource`'s partials, away from its corner (TIME held at 0).
    partials::verify_every_function_with("StepSource", step_source(), &[(0, 5, 0.0)]);
}

#[test]
fn its_device_passes_the_partials_verifier() {
    partials::verify_every_function("CubicCapacitor", cubic_capacitor());
}
