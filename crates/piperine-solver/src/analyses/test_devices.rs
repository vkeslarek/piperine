//! The two-terminal doubles the `tf`, `pz` and `sp` unit tests build their
//! circuits from, as `Device`s: a resistor, a capacitor, a DC-only bias
//! conductance, an ideal wire, a DC voltage source, an inductor and a device
//! whose AC stamp is quadratic in frequency. Each is added to a
//! `CircuitBuilder` by a plain function.

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_analog_solver::netlist::AnalogReference;

use crate::core::bind::{ParamValue, Port};
use crate::core::builder::CircuitBuilder;

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}

fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}

fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: "test_devices.rs", op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
}

fn param(name: &'static str) -> PinSpec<'static> {
    PinSpec::discrete(name, role::PARAM, &F64)
}


/// The shape and module of one device type behind a pair of statics.
fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
    slot.get_or_init(build)
}

fn module_of<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
    slot.get_or_init(|| module::<D>(name))
}

fn nodes(p: &AnalogReference, n: &AnalogReference) -> [Port; 2] {
    [Port::Node(p.clone()), Port::Node(n.clone())]
}

// ── R: `I(p, n) <+ V(p, n) / r` ─────────────────────────────────────────

struct R;
static R_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static R_MODULE: OnceLock<Module> = OnceLock::new();

fn r_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("r"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        origin: "test_devices.rs",
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for R {
    fn prototype() -> &'static Shape<'static> {
        shared(&R_SHAPE, r_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((R, Arc::clone(shared(&R_SHAPE, r_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, g) = (cx.f64(0), cx.f64(1), 1.0 / cx.f64(2));
        cx.set(0, g * vp - g * vn);
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A resistor between `p` and `n`.
pub(crate) fn resistor(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, r: f64) {
    b.device(module_of::<R>(&R_MODULE, "R"), path, &[("r", ParamValue::Real(r))], &nodes(p, n));
}

// ── C: `I(p, n) <+ c * ddt(V(p, n))` ────────────────────────────────────

struct C;
static C_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static C_MODULE: OnceLock<Module> = OnceLock::new();

fn c_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("c"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("q", op::DDT, p, n));
    s.function(FunctionSpec {
        name: "q",
        origin: "test_devices.rs",
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for C {
    fn prototype() -> &'static Shape<'static> {
        shared(&C_SHAPE, c_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((C, Arc::clone(shared(&C_SHAPE, c_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, c) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, c * vp - c * vn);
        cx.jac().copy_from_slice(&[c, -c]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A capacitor between `p` and `n`.
pub(crate) fn capacitor(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, c: f64) {
    b.device(module_of::<C>(&C_MODULE, "C"), path, &[("c", ParamValue::Real(c))], &nodes(p, n));
}

// ── Bias: a 1 nS conductance to ground that exists at DC only ───────────

struct Bias;
static BIAS_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static BIAS_MODULE: OnceLock<Module> = OnceLock::new();

fn bias_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, GROUND));
    let is_static = Cell { kind: cell::AMBIENT, index: ambient::STATIC, part: part::VALUE };
    s.function(FunctionSpec {
        name: "i",
        origin: "test_devices.rs",
        reads: vec![pin(p), is_static],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Bias {
    fn prototype() -> &'static Shape<'static> {
        shared(&BIAS_SHAPE, bias_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Bias, Arc::clone(shared(&BIAS_SHAPE, bias_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = if cx.bool(1) { 1.0e-9 } else { 0.0 };
        cx.set(0, g * cx.f64(0));
        cx.jac().copy_from_slice(&[g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A 1 GΩ DC-only bias conductance from `n` to ground: it gives a floating test node a
/// defined operating point and no small-signal admittance.
pub(crate) fn dc_bias(b: &mut CircuitBuilder, path: &str, n: &AnalogReference) {
    b.device(module_of::<Bias>(&BIAS_MODULE, "Bias"), path, &[], &[Port::Node(n.clone())]);
}

// ── Branch devices: wire, voltage source, inductor ──────────────────────
//
// Each owns a private branch current `f`: a FLOW term {p, n} of value `f` and
// a FLOW term {f, GROUND} of `V(p) - V(n) - value`; the inductor adds a DDT
// term {f, GROUND} of `-l * f`.

fn branch_shape(extra: Option<(&'static str, bool)>) -> ShapeRef {
    let mut s = ShapeBuf::new();
    if let Some((name, _)) = extra {
        s.pin(param(name));
    }
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    let mut writes = vec![term(t0), term(t1)];
    let mut partials = vec![partial(0, 2), partial(1, 0), partial(1, 1)];
    let mut reads = vec![pin(p), pin(n), pin(f)];
    if let Some((_, reactive)) = extra {
        reads.push(pin(0));
        if reactive {
            // The inductor: flux `-l * f`.
            let t2 = s.term(flow("flux", op::DDT, f, GROUND));
            writes.push(term(t2));
            partials.push(partial(2, 2));
            partials.push(partial(2, 3));
        } else {
            // The source: its value enters `eq` with partial -1.
            partials.push(partial(1, 3));
        }
    }
    s.function(FunctionSpec { name: "branch", origin: "test_devices.rs", reads, writes, partials, ..FunctionSpec::default() });
    s.freeze()
}

struct Wire;
static WIRE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static WIRE_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Wire {
    fn prototype() -> &'static Shape<'static> {
        shared(&WIRE_SHAPE, || branch_shape(None)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Wire, Arc::clone(shared(&WIRE_SHAPE, || branch_shape(None)))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, f);
        cx.set(1, vp - vn);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// An ideal wire between two distinct nodes.
pub(crate) fn wire(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference) {
    b.device(module_of::<Wire>(&WIRE_MODULE, "Wire"), path, &[], &nodes(p, n));
}

struct Vsrc;
static VSRC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VSRC_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Vsrc {
    fn prototype() -> &'static Shape<'static> {
        shared(&VSRC_SHAPE, || branch_shape(Some(("dc", false)))).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Vsrc, Arc::clone(shared(&VSRC_SHAPE, || branch_shape(Some(("dc", false)))))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, dc) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn - dc);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// An ideal DC voltage source between `p` and `n`.
pub(crate) fn vsrc(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, dc: f64) {
    b.device(module_of::<Vsrc>(&VSRC_MODULE, "Vsrc"), path, &[("dc", ParamValue::Real(dc))], &nodes(p, n));
}

struct L;
static L_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static L_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for L {
    fn prototype() -> &'static Shape<'static> {
        shared(&L_SHAPE, || branch_shape(Some(("l", true)))).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((L, Arc::clone(shared(&L_SHAPE, || branch_shape(Some(("l", true)))))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, l) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn);
        cx.set(2, -l * f);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -l, -f]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// An ideal inductor between `p` and `n`: a DC short whose AC branch row adds `-jωL`.
pub(crate) fn inductor(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, l: f64) {
    b.device(module_of::<L>(&L_MODULE, "L"), path, &[("l", ParamValue::Real(l))], &nodes(p, n));
}

// ── Quadratic: an AC stamp `jω²k` at (n1, n2) ───────────────────────────

struct Quadratic;
static QUAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static QUAD_MODULE: OnceLock<Module> = OnceLock::new();

fn quad_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("k"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let b = s.pin(PinSpec::continuous("b", role::PORT));
    let t0 = s.term(flow("q", op::DDT, a, GROUND));
    let freq = Cell { kind: cell::AMBIENT, index: ambient::FREQ, part: part::VALUE };
    s.function(FunctionSpec {
        name: "q",
        origin: "test_devices.rs",
        reads: vec![pin(b), pin(0), freq],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Quadratic {
    fn prototype() -> &'static Shape<'static> {
        shared(&QUAD_SHAPE, quad_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Quadratic, Arc::clone(shared(&QUAD_SHAPE, quad_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        // The DDT term's response is jω; the partial `ω·k` makes the stamp `jω²k`.
        let omega = 2.0 * std::f64::consts::PI * cx.f64(2);
        let g = omega * cx.f64(1);
        cx.set(0, g * cx.f64(0));
        cx.jac().copy_from_slice(&[g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A device whose AC stamp at (`n1`, `n2`) is `jω²k`: not affine in `jω`.
pub(crate) fn freq_nonlinear(b: &mut CircuitBuilder, path: &str, n1: &AnalogReference, n2: &AnalogReference, k: f64) {
    b.device(module_of::<Quadratic>(&QUAD_MODULE, "Quadratic"), path, &[("k", ParamValue::Real(k))], &nodes(n1, n2));
}
