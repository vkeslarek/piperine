//! The transient driver on devices (SAI-11, SAI-01): an RC step reads
//! `1 - e^-1` with the literal and tolerance of `helper_devices.rs`; a
//! continuous pin's `init` starts Newton and is held through a transient's
//! operating point only; a rejected stage leaves the term histories as they
//! were.

use std::sync::{Arc, OnceLock};

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{add_capacitor, add_resistor, add_vsrc, node_id};
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
