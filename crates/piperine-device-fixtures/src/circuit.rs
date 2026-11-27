//! The analog device library of the solver's ABI tests on the `Device` trait: the same equations and
//! constants as the Verilog-A models, each keeping the Verilog-A text it quotes. Every function declares
//! its partials against its continuous pins and against the params the sensitivity tests differentiate.
//! A voltage source is the potential-contribution form of abi.md A1 (a private branch pin `f`).

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, op, part, pin_flag, role,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

/// The solver constant every junction module of the corpus uses.
/// The solver constant every junction module of the corpus uses.
const THERMAL_VOLTAGE: f64 = 0.02585;
/// The literal the Verilog-A sources write for pi.
const PI_LITERAL: f64 = 3.14159265358979;

/// The origin every term and function of this module reports: the worked examples of abi.md section 11.
const ORIGIN: &str = "abi.md:11";

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
/// as the conductance-only stamp of the earlier hand-written double has none. Written
/// `(V_p − V_n)/r`, it carries a last-bit residue every solve, which the
/// step controller turns into diverging step times (the differential oracle).
pub struct Resistor;
static R_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static R_MODULE: OnceLock<Module> = OnceLock::new();
const R_DEFAULT: [u8; 8] = 1.0e3_f64.to_le_bytes();

impl Resistor {
    /// The shape of `Resistor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("r", &R_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(0)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0), CircuitParts::partial(0, 1), CircuitParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Resistor`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<Resistor>(&R_MODULE, "R")
    }
}

impl Device for Resistor {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&R_SHAPE, Resistor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resistor, Arc::clone(CircuitParts::shape_of(&R_SHAPE, Resistor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, r) = (cx.f64(0), cx.f64(1), cx.f64(2));
        let g = 1.0 / r;
        cx.set(0, g * vp - g * vn);
        // `dF/dr = -(V_p - V_n) / r^2`: the sensitivity partial.
        cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r)]);
        Ok(())
    }
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
/// `w` defaults to 1 mS; a structural param, so changing it is a rebuild.
/// pins: r=0 (PARAM F64); w=1 (PARAM F64, STRUCTURAL, default 1 mS); p=2, n=3 (PORT)
/// terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, w] writes [T0] partials (0<-0) (0<-1); none against a param,
/// so a sensitivity to `w` or `r` takes the finite-difference fallback.
pub struct OptionalResistor;
/// The default of `ROpt`'s `w`: 1 mS, as little-endian bytes.
static W_DEFAULT: [u8; 8] = 1.0e-3f64.to_le_bytes();
static ROPT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static ROPT_MODULE: OnceLock<Module> = OnceLock::new();

impl OptionalResistor {
    /// The shape of `OptionalResistor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("r", &R_DEFAULT));
        let w = s.pin(PinSpec { flags: pin_flag::STRUCTURAL | pin_flag::HAS_INIT, init: &W_DEFAULT, ..PinSpec::discrete("w", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(w)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0), CircuitParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `OptionalResistor`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<OptionalResistor>(&ROPT_MODULE, "ROpt")
    }
}

impl Device for OptionalResistor {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&ROPT_SHAPE, OptionalResistor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((OptionalResistor, Arc::clone(CircuitParts::shape_of(&ROPT_SHAPE, OptionalResistor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = cx.f64(2);
        cx.set(0, (cx.f64(0) - cx.f64(1)) * g);
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
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
pub struct Capacitor;
static C_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static C_MODULE: OnceLock<Module> = OnceLock::new();
const C_DEFAULT: [u8; 8] = 1.0e-6_f64.to_le_bytes();

impl Capacitor {
    /// The shape of `Capacitor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("c", &C_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(CircuitParts::flow("q", op::DDT, p, n));
        s.function(FunctionSpec {
            name: "q",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(0)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0), CircuitParts::partial(0, 1), CircuitParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Capacitor`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<Capacitor>(&C_MODULE, "C")
    }
}

impl Device for Capacitor {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&C_SHAPE, Capacitor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capacitor, Arc::clone(CircuitParts::shape_of(&C_SHAPE, Capacitor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, c * v);
        cx.jac().copy_from_slice(&[c, -c, v]);
        Ok(())
    }
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
pub struct Diode;
static D_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static D_MODULE: OnceLock<Module> = OnceLock::new();
const D_DEFAULT: [u8; 8] = 1.0e-14_f64.to_le_bytes();

impl Diode {
    /// The shape of `Diode`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("is_sat", &D_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(0)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0), CircuitParts::partial(0, 1), CircuitParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Diode`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<Diode>(&D_MODULE, "D")
    }
}

impl Device for Diode {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&D_SHAPE, Diode::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Diode, Arc::clone(CircuitParts::shape_of(&D_SHAPE, Diode::shape))))
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
pub struct VoltageSource;
static VSRC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VSRC_MODULE: OnceLock<Module> = OnceLock::new();
const VSRC_DEFAULT: [u8; 8] = 10.0_f64.to_le_bytes();

impl VoltageSource {
    /// The shape of `VoltageSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("dc", &VSRC_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(CircuitParts::flow("eq", op::FLOW, f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(f), CircuitParts::pin(0), CircuitParts::ambient_cell(ambient::SOURCE_SCALE)],
            writes: vec![CircuitParts::term(t0), CircuitParts::term(t1)],
            partials: vec![CircuitParts::partial(0, 2), CircuitParts::partial(1, 0), CircuitParts::partial(1, 1), CircuitParts::partial(1, 3)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `VoltageSource`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<VoltageSource>(&VSRC_MODULE, "Vsrc")
    }
}

impl Device for VoltageSource {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&VSRC_SHAPE, VoltageSource::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((VoltageSource, Arc::clone(CircuitParts::shape_of(&VSRC_SHAPE, VoltageSource::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, dc, scale) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4));
        cx.set(0, f);
        cx.set(1, vp - vn - dc * scale);
        // The branch row is `V_p - V_n - dc*scale`, so `dF/d dc = -scale`.
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -scale]);
        Ok(())
    }
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
pub struct SineSource;
static VSINE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VSINE_MODULE: OnceLock<Module> = OnceLock::new();
const VSINE_AMP: [u8; 8] = 5.0_f64.to_le_bytes();
const VSINE_FREQ: [u8; 8] = 1.0e3_f64.to_le_bytes();

impl SineSource {
    /// The shape of `SineSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("amp", &VSINE_AMP));
        s.pin(CircuitParts::real_param("freq", &VSINE_FREQ));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(CircuitParts::flow("eq", op::FLOW, f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(f), CircuitParts::pin(0), CircuitParts::pin(1), CircuitParts::ambient_cell(ambient::TIME)],
            writes: vec![CircuitParts::term(t0), CircuitParts::term(t1)],
            partials: vec![CircuitParts::partial(0, 2), CircuitParts::partial(1, 0), CircuitParts::partial(1, 1), CircuitParts::partial(1, 3), CircuitParts::partial(1, 4)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `SineSource`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<SineSource>(&VSINE_MODULE, "Vsine")
    }
}

impl Device for SineSource {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&VSINE_SHAPE, SineSource::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SineSource, Arc::clone(CircuitParts::shape_of(&VSINE_SHAPE, SineSource::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, amp, freq, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4), cx.f64(5));
        let arg = 2.0 * PI_LITERAL * freq * t;
        cx.set(0, f);
        cx.set(1, vp - vn - amp * arg.sin());
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -arg.sin(), -amp * arg.cos() * 2.0 * PI_LITERAL * t]);
        Ok(())
    }
}

/// ```verilog
/// module Vramp (inout electrical p, inout electrical n);
///     analog V(p, n) <+ 1.0e3 * $abstime;
/// endmodule
/// ```
/// pins: p=0, n=1 (PORT); f=2 (PRIVATE)
/// fn 0: reads [p, n, f, TIME] writes [T0, T1] partials (0<-f) (1<-p) (1<-n)
pub struct RampSource;
static VRAMP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VRAMP_MODULE: OnceLock<Module> = OnceLock::new();

impl RampSource {
    /// The shape of `RampSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(CircuitParts::flow("eq", op::FLOW, f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(f), CircuitParts::ambient_cell(ambient::TIME)],
            writes: vec![CircuitParts::term(t0), CircuitParts::term(t1)],
            partials: vec![CircuitParts::partial(0, 2), CircuitParts::partial(1, 0), CircuitParts::partial(1, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `RampSource`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<RampSource>(&VRAMP_MODULE, "Vramp")
    }
}

impl Device for RampSource {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&VRAMP_SHAPE, RampSource::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((RampSource, Arc::clone(CircuitParts::shape_of(&VRAMP_SHAPE, RampSource::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn - 1.0e3 * t);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
}

/// An AC current source injecting `i` amps into `p` (small-signal stimulus).
/// pins: i=0 (PARAM F64, default 1); p=1 (PORT)
/// inputs: I0 STIM "stim"; terms: T0 FLOW{p, GROUND} "i"
/// fn 0 "stim": reads [i] writes [I0.RE, I0.IM]
/// fn 1 "load": reads [I0.VALUE] writes [T0] partials (0<-0) = -1
pub struct AcCurrentSource;
static IAC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static IAC_MODULE: OnceLock<Module> = OnceLock::new();
const IAC_DEFAULT: [u8; 8] = 1.0_f64.to_le_bytes();

impl AcCurrentSource {
    /// The shape of `AcCurrentSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("i", &IAC_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let i0 = s.input(piperine_abi::shape::InputSpec { name: "stim", origin: ORIGIN, kind: piperine_abi::raw::input::STIM });
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, GROUND));
        s.function(FunctionSpec {
            name: "stim",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(0)],
            writes: vec![CircuitParts::input_cell(i0, part::RE), CircuitParts::input_cell(i0, part::IM)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "load",
            origin: ORIGIN,
            reads: vec![CircuitParts::input_cell(i0, part::VALUE)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `AcCurrentSource`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<AcCurrentSource>(&IAC_MODULE, "Iac")
    }
}

impl Device for AcCurrentSource {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&IAC_SHAPE, AcCurrentSource::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AcCurrentSource, Arc::clone(CircuitParts::shape_of(&IAC_SHAPE, AcCurrentSource::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let i = cx.f64(0);
            cx.set(0, i);
            cx.set(1, 0.0_f64);
        } else {
            cx.set(0, -cx.f64(0));
            cx.jac().copy_from_slice(&[-1.0]);
        }
        Ok(())
    }
}

/// An inductor, `V(p, n) = l * ddt(I)`, on a private branch current `f`.
/// pins: l=0 (PARAM F64, default 1e-3); p=1, n=2 (PORT); f=3 (PRIVATE)
/// terms: T0 FLOW{p, n} "i", T1 FLOW{f, GROUND} "eq", T2 DDT{f, GROUND} "flux"
/// fn 0: reads [p, n, f, l] writes [T0, T1, T2]
///       partials (0<-f) (1<-p) (1<-n) (2<-f) (2<-l)
pub struct Inductor;
static L_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static L_MODULE: OnceLock<Module> = OnceLock::new();
const L_DEFAULT: [u8; 8] = 1.0e-3_f64.to_le_bytes();

impl Inductor {
    /// The shape of `Inductor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("l", &L_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(CircuitParts::flow("eq", op::FLOW, f, GROUND));
        let t2 = s.term(CircuitParts::flow("flux", op::DDT, f, GROUND));
        s.function(FunctionSpec {
            name: "l",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(f), CircuitParts::pin(0)],
            writes: vec![CircuitParts::term(t0), CircuitParts::term(t1), CircuitParts::term(t2)],
            partials: vec![CircuitParts::partial(0, 2), CircuitParts::partial(1, 0), CircuitParts::partial(1, 1), CircuitParts::partial(2, 2), CircuitParts::partial(2, 3)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Inductor`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<Inductor>(&L_MODULE, "L")
    }
}

impl Device for Inductor {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&L_SHAPE, Inductor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Inductor, Arc::clone(CircuitParts::shape_of(&L_SHAPE, Inductor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, l) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn);
        cx.set(2, -l * f);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -l, -f]);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Noise: a resistor with thermal noise, and a correlated pair
// ---------------------------------------------------------------------------

/// Boltzmann's constant, J/K.
pub const K_B: f64 = 1.380649e-23;

/// `R` with its thermal noise: a current density `4kT/r` through the branch.
/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT); temp=3 (GLOBAL TEMPERATURE)
/// inputs: I0 NOISE "thermal"; terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, r, I0.VALUE] writes [T0] partials (0<-p) (0<-n) (0<-r) (0<-I0) = 1
/// fn 1 "thermal": reads [r, temp] writes [I0.PSD]
pub struct NoisyResistor;
static RN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RN_MODULE: OnceLock<Module> = OnceLock::new();

impl NoisyResistor {
    /// The shape of `NoisyResistor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("r", &R_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t = s.pin(PinSpec {
            global: piperine_abi::raw::global::TEMPERATURE,
            ..PinSpec::discrete("temp", role::GLOBAL, &F64)
        });
        let i0 = s.input(CircuitParts::noise_input("thermal"));
        let t0 = s.term(CircuitParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(p), CircuitParts::pin(n), CircuitParts::pin(0), CircuitParts::input_cell(i0, part::VALUE)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0), CircuitParts::partial(0, 1), CircuitParts::partial(0, 2), CircuitParts::partial(0, 3)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "thermal",
            origin: ORIGIN,
            reads: vec![CircuitParts::pin(0), CircuitParts::pin(t)],
            writes: vec![CircuitParts::input_cell(i0, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `NoisyResistor`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<NoisyResistor>(&RN_MODULE, "Rn")
    }
}

impl Device for NoisyResistor {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&RN_SHAPE, NoisyResistor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NoisyResistor, Arc::clone(CircuitParts::shape_of(&RN_SHAPE, NoisyResistor::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let (vp, vn, r, noise) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
            let g = 1.0 / r;
            cx.set(0, g * vp - g * vn + noise);
            cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r), 1.0]);
        } else {
            cx.set(0, 4.0 * K_B * cx.f64(1) / cx.f64(0));
        }
        Ok(())
    }
}

/// One noise source injected through two terms of one node with gains `1` and
/// `k`: fully correlated (abi.md A2), so `k = -1` cancels it.
/// pins: k=0 (PARAM F64, default -1); p=1 (PORT)
/// inputs: I0 NOISE "n"; terms: T0 FLOW{p, GROUND}, T1 FLOW{p, GROUND}
/// fn 0 "a": reads [I0.VALUE] writes [T0] partials (0<-0) = 1
/// fn 1 "b": reads [I0.VALUE, k] writes [T1] partials (0<-0) = k
/// fn 2 "psd": reads [] writes [I0.PSD] = 1e-18
pub struct NoisePair;
static PAIR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static PAIR_MODULE: OnceLock<Module> = OnceLock::new();
const PAIR_DEFAULT: [u8; 8] = (-1.0_f64).to_le_bytes();

impl NoisePair {
    /// The shape of `NoisePair`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(CircuitParts::real_param("k", &PAIR_DEFAULT));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let i0 = s.input(CircuitParts::noise_input("n"));
        let t0 = s.term(CircuitParts::flow("a", op::FLOW, p, GROUND));
        let t1 = s.term(CircuitParts::flow("b", op::FLOW, p, GROUND));
        s.function(FunctionSpec {
            name: "a",
            origin: ORIGIN,
            reads: vec![CircuitParts::input_cell(i0, part::VALUE)],
            writes: vec![CircuitParts::term(t0)],
            partials: vec![CircuitParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "b",
            origin: ORIGIN,
            reads: vec![CircuitParts::input_cell(i0, part::VALUE), CircuitParts::pin(0)],
            writes: vec![CircuitParts::term(t1)],
            partials: vec![CircuitParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "psd",
            origin: ORIGIN,
            reads: vec![],
            writes: vec![CircuitParts::input_cell(i0, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `NoisePair`, built once from its shape.
    pub fn module() -> &'static Module {
        CircuitParts::module_of::<NoisePair>(&PAIR_MODULE, "Pair")
    }
}

impl Device for NoisePair {
    fn prototype() -> &'static Shape<'static> {
        CircuitParts::shape_of(&PAIR_SHAPE, NoisePair::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NoisePair, Arc::clone(CircuitParts::shape_of(&PAIR_SHAPE, NoisePair::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        match k {
            0 => {
                let noise = cx.f64(0);
                cx.set(0, noise);
                cx.jac().copy_from_slice(&[1.0]);
            }
            1 => {
                let (noise, gain) = (cx.f64(0), cx.f64(1));
                cx.set(0, gain * noise);
                cx.jac().copy_from_slice(&[gain]);
            }
            _ => cx.set(0, 1.0e-18_f64),
        }
        Ok(())
    }
}

/// The cells, specs and shared shapes every circuit fixture device builds its shape from.
pub struct CircuitParts;

impl CircuitParts {
    /// The cell of the value of pin `index`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of the value of term `index`.
    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The cell of the ambient constant `constant`.
    fn ambient_cell(constant: u32) -> Cell {
        Cell { kind: cell::AMBIENT, index: constant, part: part::VALUE }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The term spec named `name` of operator `operator`, between pins `plus` and `minus`.
    fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: ORIGIN, op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// A `PARAM` pin holding one `F64` with its default.
    fn real_param(name: &'static str, default: &'static [u8; 8]) -> PinSpec<'static> {
        PinSpec { flags: pin_flag::HAS_INIT, init: default, ..PinSpec::discrete(name, role::PARAM, &F64) }
    }

    /// The shape `build` makes, built the first time and kept in `slot` so every instance shares it.
    fn shape_of(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// The module of the device type `Kind` named `name`, built the first time and kept in `slot`.
    fn module_of<Kind: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
        slot.get_or_init(|| module::<Kind>(name))
    }

    // ---------------------------------------------------------------------------
    // AC source and inductor
    // ---------------------------------------------------------------------------

    /// The cell of part `part` of input `index`.
    fn input_cell(index: u32, part: u32) -> Cell {
        Cell { kind: cell::INPUT, index, part }
    }

    /// The `NOISE` input named `name`.
    fn noise_input(name: &'static str) -> piperine_abi::shape::InputSpec<'static> {
        piperine_abi::shape::InputSpec { name, origin: ORIGIN, kind: piperine_abi::raw::input::NOISE }
    }
}
