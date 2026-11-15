//! The analog device library of `tests/helpers/devices.rs` on the `Device`
//! trait: the same equations and constants, each keeping the Verilog-A text
//! it quotes. Every function declares its partials against its continuous
//! pins and against the params the sensitivity tests differentiate. Circuits
//! are assembled with `CircuitBuilder::device`; a voltage source is the
//! potential-contribution form of abi.md A1 (a private branch pin `f`).

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{F64, REAL};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, op, part, pin_flag, role,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, AnalogVariable, NodeIdentifier, ParamValue, Port};
use piperine_solver::prelude::CircuitBuilder;

/// The solver constant every junction module of the corpus uses.
const THERMAL_VOLTAGE: f64 = 0.02585;
/// The literal the Verilog-A sources write for pi.
const PI_LITERAL: f64 = 3.14159265358979;

const ORIGIN: &str = "abi_devices.rs";

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}

fn amb(constant: u32) -> Cell {
    Cell { kind: cell::AMBIENT, index: constant, part: part::VALUE }
}

fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}

fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: ORIGIN, op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
}

/// A `PARAM` pin holding one `F64` with its default.
fn real_param(name: &'static str, default: &'static [u8; 8]) -> PinSpec<'static> {
    PinSpec { flags: pin_flag::HAS_INIT, init: default, ..PinSpec::discrete(name, role::PARAM, &F64) }
}

fn shape_of(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
    slot.get_or_init(build)
}

fn module_of<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
    slot.get_or_init(|| module::<D>(name))
}

// ---------------------------------------------------------------------------
// Circuit assembly helpers
// ---------------------------------------------------------------------------

/// The `NodeIdentifier` behind a node reference (results are keyed by it).
pub fn node_id(reference: &AnalogReference) -> NodeIdentifier {
    match reference.variable().as_ref() {
        AnalogVariable::Node(id) => id.clone(),
        other => panic!("not a node reference: {other:?}"),
    }
}

fn nodes(p: &AnalogReference, n: &AnalogReference) -> [Port; 2] {
    [Port::Node(p.clone()), Port::Node(n.clone())]
}

pub fn add_resistor(b: &mut CircuitBuilder, path: &str, r: f64, p: &AnalogReference, n: &AnalogReference) {
    b.device(resistor(), path, &[("r", ParamValue::Real(r))], &nodes(p, n));
}

pub fn add_capacitor(b: &mut CircuitBuilder, path: &str, c: f64, p: &AnalogReference, n: &AnalogReference) {
    b.device(capacitor(), path, &[("c", ParamValue::Real(c))], &nodes(p, n));
}

pub fn add_diode(b: &mut CircuitBuilder, path: &str, is_sat: f64, p: &AnalogReference, n: &AnalogReference) {
    b.device(diode(), path, &[("is_sat", ParamValue::Real(is_sat))], &nodes(p, n));
}

pub fn add_vsrc(b: &mut CircuitBuilder, path: &str, dc: f64, p: &AnalogReference, n: &AnalogReference) {
    b.device(vsrc(), path, &[("dc", ParamValue::Real(dc))], &nodes(p, n));
}

// ---------------------------------------------------------------------------
// R
// ---------------------------------------------------------------------------

/// ```verilog
/// module R (inout electrical p, inout electrical n);
///     parameter real r = 1.0e3;
///     analog I(p, n) <+ V(p, n) / r;
/// endmodule
/// ```
/// pins: r=0 (PARAM F64, default 1e3); p=1, n=2 (PORT)
/// terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, r] writes [T0] partials (0<-0) (0<-1) (0<-2)
///
/// The value is written as `g·V_p − g·V_n`, the same products the assembly's
/// Norton right side `−(v − Σ g_j·x_j)` subtracts, so that side is exactly 0
/// as the conductance-only stamp of the `Element` double has none. Written
/// `(V_p − V_n)/r`, it carries a last-bit residue every solve, which the
/// step controller turns into diverging step times (the differential oracle).
pub struct R;
static R_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static R_MODULE: OnceLock<Module> = OnceLock::new();
const R_DEFAULT: [u8; 8] = 1.0e3_f64.to_le_bytes();

fn r_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("r", &R_DEFAULT));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for R {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&R_SHAPE, r_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((R, Arc::clone(shape_of(&R_SHAPE, r_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, r) = (cx.f64(0), cx.f64(1), cx.f64(2));
        let g = 1.0 / r;
        cx.set(0, g * vp - g * vn);
        // `dF/dr = -(V_p - V_n) / r^2`: the sensitivity partial.
        cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r)]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn resistor() -> &'static Module {
    module_of::<R>(&R_MODULE, "R")
}

// ---------------------------------------------------------------------------
// ROpt
// ---------------------------------------------------------------------------

/// ```verilog
/// module ROpt (inout electrical p, inout electrical n);
///     parameter real r = 1.0e3;
///     parameter real w = 1.0e-3;
///     real g;
///     analog begin
///         g = 1.0e-3;
///         if ($param_given("w")) g = w;
///         I(p, n) <+ V(p, n) * g;
///     end
/// endmodule
/// ```
/// `w` is presence-queried: a `REAL` field whose known byte is 0 was not given.
/// Changing whether it is given is a rebuild, so the pin is `STRUCTURAL`.
/// pins: r=0 (PARAM F64); w=1 (PARAM REAL, STRUCTURAL); p=2, n=3 (PORT)
/// terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, w] writes [T0] partials (0<-0) (0<-1); none against a param,
/// so a sensitivity to `w` or `r` takes the finite-difference fallback.
pub struct ROpt;
static ROPT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static ROPT_MODULE: OnceLock<Module> = OnceLock::new();

fn ropt_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("r", &R_DEFAULT));
    let w = s.pin(PinSpec { flags: pin_flag::STRUCTURAL, ..PinSpec::discrete("w", role::PARAM, &REAL) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(w)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for ROpt {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&ROPT_SHAPE, ropt_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ROpt, Arc::clone(shape_of(&ROPT_SHAPE, ropt_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let given = cx.bytes(2)[8] != 0;
        let g = if given { cx.f64(2) } else { 1.0e-3 };
        cx.set(0, (cx.f64(0) - cx.f64(1)) * g);
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn resistor_opt() -> &'static Module {
    module_of::<ROpt>(&ROPT_MODULE, "ROpt")
}

// ---------------------------------------------------------------------------
// C
// ---------------------------------------------------------------------------

/// ```verilog
/// module C (inout electrical p, inout electrical n);
///     parameter real c = 1.0e-6;
///     analog I(p, n) <+ c * ddt(V(p, n));
/// endmodule
/// ```
/// The solver owns the TR-BDF2 companion; the device declares the charge.
/// pins: c=0 (PARAM F64, default 1e-6); p=1, n=2 (PORT)
/// terms: T0 DDT{p, n} "q"
/// fn 0 "q": reads [p, n, c] writes [T0] partials (0<-0) (0<-1) (0<-2)
pub struct C;
static C_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static C_MODULE: OnceLock<Module> = OnceLock::new();
const C_DEFAULT: [u8; 8] = 1.0e-6_f64.to_le_bytes();

fn c_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("c", &C_DEFAULT));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("q", op::DDT, p, n));
    s.function(FunctionSpec {
        name: "q",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for C {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&C_SHAPE, c_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((C, Arc::clone(shape_of(&C_SHAPE, c_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, c * v);
        cx.jac().copy_from_slice(&[c, -c, v]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn capacitor() -> &'static Module {
    module_of::<C>(&C_MODULE, "C")
}

// ---------------------------------------------------------------------------
// D
// ---------------------------------------------------------------------------

/// ```verilog
/// module D (inout electrical p, inout electrical n);
///     parameter real is_sat = 1.0e-14;
///     analog I(p, n) <+ is_sat * (exp(V(p, n) / 0.02585) - 1.0);
/// endmodule
/// ```
/// pins: is_sat=0 (PARAM F64, default 1e-14); p=1, n=2 (PORT)
/// terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, is_sat] writes [T0] partials (0<-0) (0<-1) (0<-2)
pub struct D;
static D_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static D_MODULE: OnceLock<Module> = OnceLock::new();
const D_DEFAULT: [u8; 8] = 1.0e-14_f64.to_le_bytes();

fn d_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("is_sat", &D_DEFAULT));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(0)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for D {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&D_SHAPE, d_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((D, Arc::clone(shape_of(&D_SHAPE, d_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vd, is_sat) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        let ex = (vd / THERMAL_VOLTAGE).exp();
        let gd = is_sat / THERMAL_VOLTAGE * ex;
        cx.set(0, is_sat * (ex - 1.0));
        // `dF/d is_sat = exp(vd / vt) - 1`.
        cx.jac().copy_from_slice(&[gd, -gd, ex - 1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn diode() -> &'static Module {
    module_of::<D>(&D_MODULE, "D")
}

// ---------------------------------------------------------------------------
// Voltage sources: Vsrc, Vsine, Vramp
// ---------------------------------------------------------------------------
//
// A potential contribution is a private continuous pin `f` (the branch
// current): a FLOW term {p, n} of value `f` and a FLOW term {f, GROUND} of
// value `V(p) - V(n) - value` (abi.md A1).

/// ```verilog
/// module Vsrc (inout electrical p, inout electrical n);
///     parameter real dc = 10.0;
///     analog V(p, n) <+ dc;
/// endmodule
/// ```
/// pins: dc=0 (PARAM F64, default 10); p=1, n=2 (PORT); f=3 (PRIVATE continuous)
/// terms: T0 FLOW{p, n} "i", T1 FLOW{f, GROUND} "eq"
/// fn 0: reads [p, n, f, dc, SOURCE_SCALE] writes [T0, T1]
///       partials (0<-f) (1<-p) (1<-n) (1<-dc)
pub struct Vsrc;
static VSRC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VSRC_MODULE: OnceLock<Module> = OnceLock::new();
const VSRC_DEFAULT: [u8; 8] = 10.0_f64.to_le_bytes();

fn vsrc_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("dc", &VSRC_DEFAULT));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    s.function(FunctionSpec {
        name: "v",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(f), pin(0), amb(ambient::SOURCE_SCALE)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 0), partial(1, 1), partial(1, 3)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Vsrc {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&VSRC_SHAPE, vsrc_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Vsrc, Arc::clone(shape_of(&VSRC_SHAPE, vsrc_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, dc, scale) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4));
        cx.set(0, f);
        cx.set(1, vp - vn - dc * scale);
        // The branch row is `V_p - V_n - dc*scale`, so `dF/d dc = -scale`.
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -scale]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn vsrc() -> &'static Module {
    module_of::<Vsrc>(&VSRC_MODULE, "Vsrc")
}

/// ```verilog
/// module Vsine (inout electrical p, inout electrical n);
///     parameter real amp = 5.0;
///     parameter real freq = 1.0e3;
///     analog V(p, n) <+ amp * sin(2.0 * 3.14159265358979 * freq * $abstime);
/// endmodule
/// ```
/// pins: amp=0 (5), freq=1 (1e3) (PARAM F64); p=2, n=3 (PORT); f=4 (PRIVATE)
/// fn 0: reads [p, n, f, amp, freq, TIME] writes [T0, T1]
///       partials (0<-f) (1<-p) (1<-n) (1<-amp) (1<-freq)
pub struct Vsine;
static VSINE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VSINE_MODULE: OnceLock<Module> = OnceLock::new();
const VSINE_AMP: [u8; 8] = 5.0_f64.to_le_bytes();
const VSINE_FREQ: [u8; 8] = 1.0e3_f64.to_le_bytes();

fn vsine_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(real_param("amp", &VSINE_AMP));
    s.pin(real_param("freq", &VSINE_FREQ));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    s.function(FunctionSpec {
        name: "v",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(f), pin(0), pin(1), amb(ambient::TIME)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 0), partial(1, 1), partial(1, 3), partial(1, 4)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Vsine {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&VSINE_SHAPE, vsine_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Vsine, Arc::clone(shape_of(&VSINE_SHAPE, vsine_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, amp, freq, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4), cx.f64(5));
        let arg = 2.0 * PI_LITERAL * freq * t;
        cx.set(0, f);
        cx.set(1, vp - vn - amp * arg.sin());
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -arg.sin(), -amp * arg.cos() * 2.0 * PI_LITERAL * t]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn vsine() -> &'static Module {
    module_of::<Vsine>(&VSINE_MODULE, "Vsine")
}

/// ```verilog
/// module Vramp (inout electrical p, inout electrical n);
///     analog V(p, n) <+ 1.0e3 * $abstime;
/// endmodule
/// ```
/// pins: p=0, n=1 (PORT); f=2 (PRIVATE)
/// fn 0: reads [p, n, f, TIME] writes [T0, T1] partials (0<-f) (1<-p) (1<-n)
pub struct Vramp;
static VRAMP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VRAMP_MODULE: OnceLock<Module> = OnceLock::new();

fn vramp_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    s.function(FunctionSpec {
        name: "v",
        origin: ORIGIN,
        reads: vec![pin(p), pin(n), pin(f), amb(ambient::TIME)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 0), partial(1, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Vramp {
    fn prototype() -> &'static Shape<'static> {
        shape_of(&VRAMP_SHAPE, vramp_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Vramp, Arc::clone(shape_of(&VRAMP_SHAPE, vramp_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn - 1.0e3 * t);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn vramp() -> &'static Module {
    module_of::<Vramp>(&VRAMP_MODULE, "Vramp")
}
