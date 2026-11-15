//! Limiting convergence-gate tests (ABI-10) on the `Device` trait: an active
//! `LIMIT` input vetoes Newton convergence — the solver does not declare
//! convergence at a clamped non-solution — and a device that limits and then
//! settles reaches the operating point the unlimited device reaches.
//!
//! The guess is no longer steered by the solver (the held iterate is the
//! device's own `LIMITED` value), so the old claim that a reported
//! `limited_value` overwrites the Newton guess is gone with its hook.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::result::Result;
use std::sync::{Arc, OnceLock};

use abi_devices::{add_diode, add_resistor, add_vsrc, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, input, op, part, pin_flag, role,
};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver};

const ORIGIN: &str = "limiting_report.rs";
const THERMAL_VOLTAGE: f64 = 0.02585;

fn cell_of(kind: u32, index: u32, part: u32) -> Cell {
    Cell { kind, index, part }
}

fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}

/// A conductor with a `LIMIT` input that clamps the iterate `off` above the
/// proposed value, always: the limit is active at every iteration.
/// pins: off=0 (PARAM F64); a=1 (PORT)
/// inputs: I0 LIMIT "x"; terms: T0 FLOW{a, GROUND} "i" = 1e-3·x̂
/// fn 0 "limit": reads [a, off] writes [I0.PROPOSED = v, I0.LIMITED = v + off] partial (0<-0)=1
/// fn 1 "i": reads [I0] writes [T0] partial (0<-0)=1e-3
struct AlwaysLimited;
static ALWAYS_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static ALWAYS: OnceLock<Module> = OnceLock::new();

fn always_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let off = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &0.3_f64.to_le_bytes(), ..PinSpec::discrete("off", role::PARAM, &F64) });
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let i0 = s.input(InputSpec { name: "x", origin: ORIGIN, kind: input::LIMIT });
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: a, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "limit",
        origin: ORIGIN,
        reads: vec![cell_of(cell::PIN, a, part::VALUE), cell_of(cell::PIN, off, part::VALUE)],
        writes: vec![cell_of(cell::INPUT, i0, part::PROPOSED), cell_of(cell::INPUT, i0, part::LIMITED)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![cell_of(cell::INPUT, i0, part::VALUE)],
        writes: vec![cell_of(cell::TERM, t0, part::VALUE)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for AlwaysLimited {
    fn prototype() -> &'static Shape<'static> {
        ALWAYS_SHAPE.get_or_init(always_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AlwaysLimited, Arc::clone(ALWAYS_SHAPE.get_or_init(always_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let v = cx.f64(0);
            cx.set(0, v);
            cx.set(1, v + cx.f64(1));
            cx.jac().copy_from_slice(&[1.0]);
        } else {
            cx.set(0, 1.0e-3 * cx.f64(0));
            cx.jac()[0] = 1.0e-3;
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// ngspice's junction limiter.
fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
    if vnew > vcrit && (vnew - vold).abs() > vt + vt {
        if vold > 0.0 {
            let arg = 1.0 + (vnew - vold) / vt;
            if arg > 0.0 { vold + vt * arg.ln() } else { vcrit }
        } else {
            vt * (vnew / vt).ln()
        }
    } else {
        vnew
    }
}

/// The diode of `abi_devices` with `pnjlim` on its junction voltage.
/// pins: is_sat=0 (PARAM F64); p=1, n=2 (PORT)
/// inputs: I0 LIMIT "vd"; terms: T0 FLOW{p, n} "i" = is·(exp(vd/vt) − 1)
/// fn 0 "limit": reads [p, n, is, I0.LAST, I0.AGE, STATIC] writes [I0.PROPOSED, I0.LIMITED]
///               partials (0<-0)=1 (0<-1)=-1
/// fn 1 "i": reads [I0, is] writes [T0] partials (0<-0)=gd (0<-1)
struct LimitedDiode;
static LD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LD: OnceLock<Module> = OnceLock::new();
const IS_DEFAULT: [u8; 8] = 1.0e-14_f64.to_le_bytes();

fn ld_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let is = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &IS_DEFAULT, ..PinSpec::discrete("is_sat", role::PARAM, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let i0 = s.input(InputSpec { name: "vd", origin: ORIGIN, kind: input::LIMIT });
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "limit",
        origin: ORIGIN,
        reads: vec![
            cell_of(cell::PIN, p, part::VALUE),
            cell_of(cell::PIN, n, part::VALUE),
            cell_of(cell::PIN, is, part::VALUE),
            cell_of(cell::INPUT, i0, part::LAST),
            cell_of(cell::INPUT, i0, part::AGE),
            cell_of(cell::AMBIENT, ambient::STATIC, part::VALUE),
        ],
        writes: vec![cell_of(cell::INPUT, i0, part::PROPOSED), cell_of(cell::INPUT, i0, part::LIMITED)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![cell_of(cell::INPUT, i0, part::VALUE), cell_of(cell::PIN, is, part::VALUE)],
        writes: vec![cell_of(cell::TERM, t0, part::VALUE)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for LimitedDiode {
    fn prototype() -> &'static Shape<'static> {
        LD_SHAPE.get_or_init(ld_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LimitedDiode, Arc::clone(LD_SHAPE.get_or_init(ld_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let vt = THERMAL_VOLTAGE;
            let vcrit = vt * (vt / (std::f64::consts::SQRT_2 * cx.f64(2))).ln();
            let v = cx.f64(0) - cx.f64(1);
            let (age, is_static) = (cx.u8(4), cx.bool(5));
            let old = if age == 0 || (is_static && age == 1) { vcrit } else { cx.f64(3) };
            cx.set(0, v);
            cx.set(1, pnjlim(v, old, vt, vcrit));
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let (vd, is) = (cx.f64(0), cx.f64(1));
            let e = (vd / THERMAL_VOLTAGE).exp();
            cx.set(0, is * (e - 1.0));
            cx.jac().copy_from_slice(&[is * e / THERMAL_VOLTAGE, e - 1.0]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// Spec ABI-10: an always-active limiter vetoes Newton convergence — the
/// solver does NOT declare convergence at a clamped non-solution. A voltage
/// source forcing a node to 1 V while a limiter keeps its iterate 0.3 V away
/// cannot settle, so the solve fails to converge (the limiter keeps vetoing).
/// This proves the gate reads the limit's activity, not a stale boolean.
#[test]
fn active_limiter_vetoes_newton_convergence() {
    let mut b = CircuitBuilder::new("veto");
    let gnd = b.ground();
    let a = b.node("a");
    add_vsrc(&mut b, "v1", 1.0, &a, &gnd);
    b.device(ALWAYS.get_or_init(|| module::<AlwaysLimited>("AlwaysLimited")), "alwolim", &[], &[Port::Node(a)]);
    let circuit = b.build().expect("build");
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve();

    // The limiter vetoes convergence permanently → the DC plan fails.
    assert!(res.is_err(), "an always-active limiter must veto convergence");
}

/// A limiter that limits the early iterates and then releases reaches the
/// operating point the unlimited diode reaches: the limit steers the path,
/// never the fixed point.
#[test]
fn a_limited_diode_reaches_the_unlimited_operating_point() {
    let solve = |limited: bool| {
        let mut b = CircuitBuilder::new("diode");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        add_vsrc(&mut b, "v1", 5.0, &top, &gnd);
        add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
        if limited {
            b.device(
                LD.get_or_init(|| module::<LimitedDiode>("LimitedDiode")),
                "d1",
                &[("is_sat", ParamValue::Real(1.0e-14))],
                &[Port::Node(mid.clone()), Port::Node(gnd.clone())],
            );
        } else {
            add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
        }
        let circuit = b.build().expect("build");
        let mut solver = Solver::new(circuit).with_context(Context::default()).build();
        solver.dc().unwrap().solve().unwrap().get_node(&node_id(&mid)).unwrap()
    };
    let (limited, plain) = (solve(true), solve(false));
    assert!(plain > 0.5 && plain < 0.9, "the diode conducts: {plain}");
    // Both stop inside Newton's own criterion (vntol + reltol·|v| ≈ 7e-4 V).
    assert!((limited - plain).abs() < 1e-4, "limited {limited} vs unlimited {plain}");
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    use partials::Fixture;
    // The fixture of the veto: its limit is active at every point.
    partials::verify_every_function_except(
        "AlwaysLimited",
        ALWAYS.get_or_init(|| module::<AlwaysLimited>("AlwaysLimited")),
        &[(0, Fixture::AlwaysLimited)],
    );
    partials::verify_every_function("LimitedDiode", LD.get_or_init(|| module::<LimitedDiode>("LimitedDiode")));
}
