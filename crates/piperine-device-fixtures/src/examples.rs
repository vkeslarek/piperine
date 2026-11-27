//! The worked examples of abi.md §11.2 and §11.6 as `impl Device`, constant for
//! constant: the diode with limiting and noise and the DAC (the other four examples are the
//! `circuit` resistor and capacitor, the `digital` flip-flop and the `mixed` comparator). Each shape is built as its comment states, `PARAM`
//! pins first (P5); the shape is shared by every instance (abi.md §9).

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::{BYTE, F64};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, global, input, op, part, role,
    watch,
};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

/// Boltzmann's constant, in joules per kelvin.
pub const K_B: f64 = 1.380649e-23;
/// The charge of an electron, in coulombs.
pub const Q: f64 = 1.602176634e-19;
/// The flicker-noise coefficient of the diode.
pub const KF: f64 = 1e-16;
const SQRT_2: f64 = std::f64::consts::SQRT_2;

// ─── 11.2 LimitingDiode with limiting and noise ─────────────────────────────────────

/// pins:    is=0, n=1, cj0=2 (PARAM F64); a=3, k=4 (PORT); temperature=5 (GLOBAL TEMPERATURE)
/// inputs:  I0 LIMIT "vd", I1 NOISE "shot", I2 NOISE "flicker"        (ambient: FREQ, STATIC)
/// terms:   T0 FLOW{a,k} "id", T1 DDT{a,k} "qj"          watches: W0 VAR "gd"
/// fn 0 "limit":   reads [a, k, is, n, temperature, I0.LAST, I0.AGE, STATIC]
///                 writes [I0.PROPOSED, I0.LIMITED]            partials (0←0)=1 (0←1)=-1
/// fn 1 "current": reads [I0, is, n, cj0, temperature, I1, I2]
///                 writes [T0, T1]                             partials (0←0) (1←0) (0←5) (0←6)
/// fn 2 "shot":    reads [I0, is, n, temperature]        writes [I1.PSD]
/// fn 3 "flicker": reads [I0, is, n, temperature, FREQ]  writes [I2.PSD]
/// fn 4 "op":      reads [I0, is, n, temperature]        writes [W0]
pub struct LimitingDiode;
static DIODE: OnceLock<ShapeRef> = OnceLock::new();

impl LimitingDiode {
    /// The shape every instance of this device shares.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let is = s.pin(ShapeParts::param("is"));
        let n = s.pin(ShapeParts::param("n"));
        let cj0 = s.pin(ShapeParts::param("cj0"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let t = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temperature", role::GLOBAL, &F64) });
        let i0 = s.input(InputSpec { name: "vd", origin: "abi.md:11.2", kind: input::LIMIT });
        let i1 = s.input(InputSpec { name: "shot", origin: "abi.md:11.2", kind: input::NOISE });
        let i2 = s.input(InputSpec { name: "flicker", origin: "abi.md:11.2", kind: input::NOISE });
        let t0 = s.term(ShapeParts::flow("id", op::FLOW, a, k));
        let t1 = s.term(ShapeParts::flow("qj", op::DDT, a, k));
        let w0 = s.watch(WatchSpec {
            name: "gd",
            origin: "abi.md:11.2",
            kind: watch::VAR,
            layout: &F64,
            event: 0,
            ttol: DEFAULT,
            etol: DEFAULT,
        });
        let value = |i| ShapeParts::input_cell(i, part::VALUE);
        s.function(FunctionSpec {
            name: "limit",
            reads: vec![ShapeParts::pin(a), ShapeParts::pin(k), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(t), ShapeParts::input_cell(i0, part::LAST), ShapeParts::input_cell(i0, part::AGE), ShapeParts::ambient(ambient::STATIC)],
            writes: vec![ShapeParts::input_cell(i0, part::PROPOSED), ShapeParts::input_cell(i0, part::LIMITED)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "current",
            reads: vec![value(i0), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(cj0), ShapeParts::pin(t), value(i1), value(i2)],
            writes: vec![ShapeParts::term(t0), ShapeParts::term(t1)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(1, 0), ShapeParts::partial(0, 5), ShapeParts::partial(0, 6)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "shot",
            reads: vec![value(i0), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(t)],
            writes: vec![ShapeParts::input_cell(i1, part::PSD)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "flicker",
            reads: vec![value(i0), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(t), ShapeParts::ambient(ambient::FREQ)],
            writes: vec![ShapeParts::input_cell(i2, part::PSD)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "op",
            reads: vec![value(i0), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(t)],
            writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// ngspice's junction limiter.
    pub fn limit_junction(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
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

    /// `is·(exp(vd/nvt) − 1)` over reads `[I0, is, n, temperature]`.
    fn current(cx: &Eval) -> f64 {
        let nvt = cx.f64(2) * K_B * cx.f64(3) / Q;
        cx.f64(1) * ((cx.f64(0) / nvt).exp() - 1.0)
    }

    /// `is·exp(vd/nvt)/nvt` over reads `[I0, is, n, temperature]`.
    fn conductance(cx: &Eval) -> f64 {
        let nvt = cx.f64(2) * K_B * cx.f64(3) / Q;
        cx.f64(1) * (cx.f64(0) / nvt).exp() / nvt
    }
}

impl Device for LimitingDiode {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&DIODE, LimitingDiode::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LimitingDiode, Arc::clone(ShapeParts::shared(&DIODE, LimitingDiode::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        match k {
            0 => {
                let nvt = cx.f64(3) * K_B * cx.f64(4) / Q;
                let vcrit = nvt * (nvt / (SQRT_2 * cx.f64(2))).ln();
                let v = cx.f64(0) - cx.f64(1);
                let (age, is_static) = (cx.u8(6), cx.bool(7));
                let old = if age == 0 || (is_static && age == 1) { vcrit } else { cx.f64(5) }; // the seed
                cx.set(0, v);
                cx.set(1, LimitingDiode::limit_junction(v, old, nvt, vcrit));
                cx.jac().copy_from_slice(&[1.0, -1.0]);
            }
            1 => {
                let (vd, is, cj) = (cx.f64(0), cx.f64(1), cx.f64(3));
                let nvt = cx.f64(2) * K_B * cx.f64(4) / Q;
                let e = (vd / nvt).exp();
                cx.set(0, is * (e - 1.0) + cx.f64(5) + cx.f64(6)); // plain values at vd; the solver corrects
                cx.set(1, cj * vd);
                cx.jac().copy_from_slice(&[is * e / nvt, cj, 1.0, 1.0]);
            }
            2 => cx.set(0, 2.0 * Q * LimitingDiode::current(cx).abs()),
            3 => cx.set(0, KF * LimitingDiode::current(cx).abs() / cx.f64(4)), // needed, hence run, only in a Noise sweep
            _ => cx.set(0, LimitingDiode::conductance(cx)),
        }
        Ok(())
    }
}

// ─── 11.6 DAC ───────────────────────────────────────────────────────────────

/// pins: vref=0 (PARAM F64);  code=1 (PORT BYTE);  out=2 (PORT continuous);  f=3 (PRIVATE continuous: the branch flow)
/// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
/// fn 0: reads [code, out, f, vref]  writes [T0, T1]  partials (0←2) (1←1) (1←3)
pub struct DigitalToAnalog;
static DAC: OnceLock<ShapeRef> = OnceLock::new();

impl DigitalToAnalog {
    /// The shape every instance of this device shares.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let vref = s.pin(ShapeParts::param("vref"));
        let code = s.pin(PinSpec::discrete("code", role::PORT, &BYTE));
        let out = s.pin(PinSpec::continuous("out", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, out, GROUND));
        let t1 = s.term(ShapeParts::flow("eq", op::FLOW, f, GROUND));
        s.function(FunctionSpec {
            name: "dac",
            reads: vec![ShapeParts::pin(code), ShapeParts::pin(out), ShapeParts::pin(f), ShapeParts::pin(vref)],
            writes: vec![ShapeParts::term(t0), ShapeParts::term(t1)],
            partials: vec![ShapeParts::partial(0, 2), ShapeParts::partial(1, 1), ShapeParts::partial(1, 3)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for DigitalToAnalog {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&DAC, DigitalToAnalog::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((DigitalToAnalog, Arc::clone(ShapeParts::shared(&DAC, DigitalToAnalog::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let k = cx.bytes(0)[0] as f64 / 255.0;
        cx.set(0, cx.f64(2)); // KCL at out
        cx.set(1, cx.f64(1) - k * cx.f64(3)); // v(out) = k·vref
        cx.jac().copy_from_slice(&[1.0, 1.0, -k]);
        Ok(())
    }
}

/// The cells, specs and shared shapes every fixture device builds its shape from.
pub struct ShapeParts;

impl ShapeParts {
    /// The value of a pin.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// One part (previous, changed, ...) of a pin.
    pub fn pin_part(index: u32, part: u32) -> Cell {
        Cell { kind: cell::PIN, index, part }
    }

    /// The value of a term.
    pub fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// One part of an input.
    pub fn input_cell(index: u32, part: u32) -> Cell {
        Cell { kind: cell::INPUT, index, part }
    }

    /// An ambient cell (frequency, static, ...).
    pub fn ambient(index: u32) -> Cell {
        Cell { kind: cell::AMBIENT, index, part: part::VALUE }
    }

    /// A partial derivative of write `write` with respect to read `read`.
    pub fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// A term of operator `op` between `plus` and `minus`, as abi.md section 11 writes it.
    pub fn flow(name: &'static str, op: u32, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: "abi.md:11", op, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// An `F64` `PARAM` pin.
    pub fn param(name: &'static str) -> PinSpec<'static> {
        PinSpec::discrete(name, role::PARAM, &F64)
    }

    /// A shape built once and shared by every instance.
    pub fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// A static `Module` per device type.
    pub fn module_of<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
        slot.get_or_init(|| module::<D>(name))
    }
}
