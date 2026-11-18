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
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
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

#[test]
fn its_device_passes_the_partials_verifier() {
    partials::verify_every_function("CubicCapacitor", cubic_capacitor());
}
