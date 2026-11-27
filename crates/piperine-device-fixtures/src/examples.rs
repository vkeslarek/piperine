//! The worked examples of abi.md §11.1 to §11.6 as `impl Device`, constant for
//! constant: resistor, diode with limiting and noise, capacitor, D flip-flop,
//! comparator and DAC. Each shape is built as its comment states, `PARAM`
//! pins first (P5); the shape is shared by every instance (abi.md §9).

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, ambient, cell, global, input, op, part, pin_flag, role,
    watch,
};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

/// Boltzmann's constant, in joules per kelvin.
pub const K_B: f64 = 1.380649e-23;
/// The charge of an electron, in coulombs.
pub const Q: f64 = 1.602176634e-19;
/// The flicker-noise coefficient of the diode.
pub const KF: f64 = 1e-16;
const SQRT_2: f64 = std::f64::consts::SQRT_2;

// ─── 11.1 Resistor ──────────────────────────────────────────────────────────

/// pins: r=0 (PARAM F64, default 1e3); p=1, n=2 (PORT continuous)
/// terms: T0 FLOW {p, n} "i"
/// fn 0 "i": reads [p, n, r] writes [T0] partials (0←0) (0←1) (0←2)
pub struct Resistor;
static RESISTOR: OnceLock<ShapeRef> = OnceLock::new();
const R_DEFAULT: [u8; 8] = [0, 0, 0, 0, 0, 0x40, 0x8f, 0x40]; // 1e3

impl Resistor {
    /// The shape every instance of this device shares.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let r = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &R_DEFAULT, ..ShapeParts::param("r") });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(r)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Resistor {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&RESISTOR, Resistor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resistor, Arc::clone(ShapeParts::shared(&RESISTOR, Resistor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, v / r);
        cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]); // the last matters only when seeded
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── 11.2 Diode with limiting and noise ─────────────────────────────────────

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
pub struct Diode;
static DIODE: OnceLock<ShapeRef> = OnceLock::new();

impl Diode {
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

impl Device for Diode {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&DIODE, Diode::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Diode, Arc::clone(ShapeParts::shared(&DIODE, Diode::shape))))
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
                cx.set(1, Diode::limit_junction(v, old, nvt, vcrit));
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
            2 => cx.set(0, 2.0 * Q * Diode::current(cx).abs()),
            3 => cx.set(0, KF * Diode::current(cx).abs() / cx.f64(4)), // needed, hence run, only in a Noise sweep
            _ => cx.set(0, Diode::conductance(cx)),
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── 11.3 Capacitor ─────────────────────────────────────────────────────────

/// pins: c=0 (PARAM F64); p=1, n=2 (PORT)      terms: T0 DDT{p,n} "q"
/// fn 0: reads [p, n, c]  writes [T0]  partials (0←0) (0←1) (0←2)
pub struct Capacitor;
static CAPACITOR: OnceLock<ShapeRef> = OnceLock::new();

impl Capacitor {
    /// The shape every instance of this device shares.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let c = s.pin(ShapeParts::param("c"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ShapeParts::flow("q", op::DDT, p, n));
        s.function(FunctionSpec {
            name: "q",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(c)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Capacitor {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&CAPACITOR, Capacitor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capacitor, Arc::clone(ShapeParts::shared(&CAPACITOR, Capacitor::shape))))
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

// ─── 11.4 D flip-flop ───────────────────────────────────────────────────────

/// pins: clk=0, d=1 (PORT BIT);  q=2 (PORT BIT, init LOW);  rise=3 (PRIVATE event)
/// fn 0 "rise":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]
/// reaction 0:   on [rise]   reads [d]   writes [S0 {q}]
pub struct FlipFlop;
static FLIP_FLOP: OnceLock<ShapeRef> = OnceLock::new();

impl FlipFlop {
    /// The shape every instance of this device shares.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
        let d = s.pin(PinSpec::discrete("d", role::PORT, &BIT));
        let q = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("q", role::PORT, &BIT) });
        let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
        s.function(FunctionSpec {
            name: "rise",
            reads: vec![ShapeParts::pin(clk), ShapeParts::pin_part(clk, part::PREVIOUS), ShapeParts::pin_part(clk, part::CHANGED)],
            writes: vec![ShapeParts::pin_part(rise, part::CHANGED)],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "sample",
            on: vec![rise],
            reads: vec![ShapeParts::pin(d)],
            writes: vec![Statement { pin: q, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }
}

impl Device for FlipFlop {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&FLIP_FLOP, FlipFlop::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlipFlop, Arc::clone(ShapeParts::shared(&FLIP_FLOP, FlipFlop::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, (cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW) as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, cx.bytes(0), 0.0)?; // q <- d : applies at (t, n+1)
        Ok(Flow::Continue)
    }
}

// ─── 11.5 Comparator ────────────────────────────────────────────────────────

/// pins: inp=0, inn=1 (PORT continuous);  out=2 (PORT BIT, init LOW);  up=3, down=4 (PRIVATE event)
/// watches: W0 CROSS → up, W1 CROSS → down
/// fn 0: reads [inp, inn]  writes [W0, W1]  partials (0←0) (0←1) (1←0) (1←1)
/// reaction 0: on [up] writes [S0 {out}]        reaction 1: on [down] writes [S0 {out}]
pub struct Comparator;
static COMPARATOR: OnceLock<ShapeRef> = OnceLock::new();

impl Comparator {
    /// The shape every instance of this device shares.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let inp_ = s.pin(PinSpec::continuous("inp", role::PORT));
        let inn = s.pin(PinSpec::continuous("inn", role::PORT));
        let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let down = s.pin(PinSpec::event("down", role::PRIVATE));
        let cross = |name, event| WatchSpec {
            name,
            origin: "abi.md:11.5",
            kind: watch::CROSS,
            layout: &F64,
            event,
            ttol: DEFAULT,
            etol: DEFAULT,
        };
        let w0 = s.watch(cross("up", up));
        let w1 = s.watch(cross("down", down));
        let w = |index| Cell { kind: cell::WATCH, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "e",
            reads: vec![ShapeParts::pin(inp_), ShapeParts::pin(inn)],
            writes: vec![w(w0), w(w1)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(1, 0), ShapeParts::partial(1, 1)],
            ..FunctionSpec::default()
        });
        for (name, on) in [("up", up), ("down", down)] {
            s.reaction(ReactionSpec {
                name,
                on: vec![on],
                writes: vec![Statement { pin: out, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
                ..ReactionSpec::default()
            });
        }
        s.freeze()
    }
}

impl Device for Comparator {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&COMPARATOR, Comparator::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Comparator, Arc::clone(ShapeParts::shared(&COMPARATOR, Comparator::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let e = cx.f64(0) - cx.f64(1);
        cx.set(0, e);
        cx.set(1, -e);
        cx.jac().copy_from_slice(&[1.0, -1.0, -1.0, 1.0]);
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, if k == 0 { &HIGH } else { &LOW }, 0.0)?;
        Ok(Flow::Continue)
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
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
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
