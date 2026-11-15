//! Fixture devices of the analog assembly's targets. The worked examples of
//! abi.md §11.1 to §11.6 are copied constant for constant from
//! `crates/piperine-abi/tests/support/examples.rs` (resistor, diode with
//! limiting and noise, capacitor, D flip-flop, comparator, DAC); the devices
//! after them are this crate's own, each built for one rule of the analog
//! side. Every shape lists its `PARAM` pins first (P5).

use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW};
use piperine_abi::raw::{
    Cell, Created, DEFAULT, Eval as RawEval, GROUND, Instance, List, Module, Note, Partial, React as RawReact, Shape,
    Statement, Str, Value, ambient, cell, global, input, op, part, pin_flag, role, status, watch,
};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

pub const K_B: f64 = 1.380649e-23;
pub const Q: f64 = 1.602176634e-19;
pub const KF: f64 = 1e-16;
const SQRT_2: f64 = std::f64::consts::SQRT_2;

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}
fn pin_part(index: u32, part: u32) -> Cell {
    Cell { kind: cell::PIN, index, part }
}
fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}
fn inp(index: u32, part: u32) -> Cell {
    Cell { kind: cell::INPUT, index, part }
}
fn ambient(index: u32) -> Cell {
    Cell { kind: cell::AMBIENT, index, part: part::VALUE }
}
fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}
fn flow(name: &'static str, op: u32, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: "abi.md:11", op, plus, minus, abstol: DEFAULT, max: DEFAULT }
}
fn param(name: &'static str) -> PinSpec<'static> {
    PinSpec::discrete(name, role::PARAM, &F64)
}

/// A shape built once and shared by every instance.
fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
    slot.get_or_init(build)
}

/// A static `Module` per device type.
pub fn module_of<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
    slot.get_or_init(|| module::<D>(name))
}

// ─── 11.1 Resistor ──────────────────────────────────────────────────────────

/// pins: r=0 (PARAM F64, default 1e3); p=1, n=2 (PORT continuous)
/// terms: T0 FLOW {p, n} "i"
/// fn 0 "i": reads [p, n, r] writes [T0] partials (0←0) (0←1) (0←2)
pub struct Resistor;
static RESISTOR: OnceLock<ShapeRef> = OnceLock::new();
const R_DEFAULT: [u8; 8] = [0, 0, 0, 0, 0, 0x40, 0x8f, 0x40]; // 1e3

fn resistor_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let r = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &R_DEFAULT, ..param("r") });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(p), pin(n), pin(r)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Resistor {
    fn prototype() -> &'static Shape {
        shared(&RESISTOR, resistor_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resistor, Arc::clone(shared(&RESISTOR, resistor_shape))))
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

fn diode_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let is = s.pin(param("is"));
    let n = s.pin(param("n"));
    let cj0 = s.pin(param("cj0"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let k = s.pin(PinSpec::continuous("k", role::PORT));
    let t = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temperature", role::GLOBAL, &F64) });
    let i0 = s.input(InputSpec { name: "vd", origin: "abi.md:11.2", kind: input::LIMIT });
    let i1 = s.input(InputSpec { name: "shot", origin: "abi.md:11.2", kind: input::NOISE });
    let i2 = s.input(InputSpec { name: "flicker", origin: "abi.md:11.2", kind: input::NOISE });
    let t0 = s.term(flow("id", op::FLOW, a, k));
    let t1 = s.term(flow("qj", op::DDT, a, k));
    let w0 = s.watch(WatchSpec {
        name: "gd",
        origin: "abi.md:11.2",
        kind: watch::VAR,
        layout: &F64,
        event: 0,
        ttol: DEFAULT,
        etol: DEFAULT,
    });
    let value = |i| inp(i, part::VALUE);
    s.function(FunctionSpec {
        name: "limit",
        reads: vec![pin(a), pin(k), pin(is), pin(n), pin(t), inp(i0, part::LAST), inp(i0, part::AGE), ambient(ambient::STATIC)],
        writes: vec![inp(i0, part::PROPOSED), inp(i0, part::LIMITED)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "current",
        reads: vec![value(i0), pin(is), pin(n), pin(cj0), pin(t), value(i1), value(i2)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 0), partial(1, 0), partial(0, 5), partial(0, 6)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "shot",
        reads: vec![value(i0), pin(is), pin(n), pin(t)],
        writes: vec![inp(i1, part::PSD)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "flicker",
        reads: vec![value(i0), pin(is), pin(n), pin(t), ambient(ambient::FREQ)],
        writes: vec![inp(i2, part::PSD)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "op",
        reads: vec![value(i0), pin(is), pin(n), pin(t)],
        writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

/// ngspice's junction limiter.
pub fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
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
fn id(cx: &Eval) -> f64 {
    let nvt = cx.f64(2) * K_B * cx.f64(3) / Q;
    cx.f64(1) * ((cx.f64(0) / nvt).exp() - 1.0)
}

/// `is·exp(vd/nvt)/nvt` over reads `[I0, is, n, temperature]`.
fn gd(cx: &Eval) -> f64 {
    let nvt = cx.f64(2) * K_B * cx.f64(3) / Q;
    cx.f64(1) * (cx.f64(0) / nvt).exp() / nvt
}

impl Device for Diode {
    fn prototype() -> &'static Shape {
        shared(&DIODE, diode_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Diode, Arc::clone(shared(&DIODE, diode_shape))))
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
                cx.set(1, pnjlim(v, old, nvt, vcrit));
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
            2 => cx.set(0, 2.0 * Q * id(cx).abs()),
            3 => cx.set(0, KF * id(cx).abs() / cx.f64(4)), // needed, hence run, only in a Noise sweep
            _ => cx.set(0, gd(cx)),
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

fn capacitor_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let c = s.pin(param("c"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("q", op::DDT, p, n));
    s.function(FunctionSpec {
        name: "q",
        reads: vec![pin(p), pin(n), pin(c)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Capacitor {
    fn prototype() -> &'static Shape {
        shared(&CAPACITOR, capacitor_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capacitor, Arc::clone(shared(&CAPACITOR, capacitor_shape))))
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

fn flip_flop_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
    let d = s.pin(PinSpec::discrete("d", role::PORT, &BIT));
    let q = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("q", role::PORT, &BIT) });
    let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
    s.function(FunctionSpec {
        name: "rise",
        reads: vec![pin(clk), pin_part(clk, part::PREVIOUS), pin_part(clk, part::CHANGED)],
        writes: vec![pin_part(rise, part::CHANGED)],
        ..FunctionSpec::default()
    });
    s.reaction(ReactionSpec {
        name: "sample",
        on: vec![rise],
        reads: vec![pin(d)],
        writes: vec![Statement { pin: q, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for FlipFlop {
    fn prototype() -> &'static Shape {
        shared(&FLIP_FLOP, flip_flop_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlipFlop, Arc::clone(shared(&FLIP_FLOP, flip_flop_shape))))
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

fn comparator_shape() -> ShapeRef {
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
        reads: vec![pin(inp_), pin(inn)],
        writes: vec![w(w0), w(w1)],
        partials: vec![partial(0, 0), partial(0, 1), partial(1, 0), partial(1, 1)],
        ..FunctionSpec::default()
    });
    for (name, on) in [("up", up), ("down", down)] {
        s.reaction(ReactionSpec {
            name,
            on: vec![on],
            writes: vec![Statement { pin: out, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
    }
    s.freeze()
}

impl Device for Comparator {
    fn prototype() -> &'static Shape {
        shared(&COMPARATOR, comparator_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Comparator, Arc::clone(shared(&COMPARATOR, comparator_shape))))
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
pub struct Dac;
static DAC: OnceLock<ShapeRef> = OnceLock::new();

fn dac_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let vref = s.pin(param("vref"));
    let code = s.pin(PinSpec::discrete("code", role::PORT, &BYTE));
    let out = s.pin(PinSpec::continuous("out", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", op::FLOW, out, GROUND));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    s.function(FunctionSpec {
        name: "dac",
        reads: vec![pin(code), pin(out), pin(f), pin(vref)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 1), partial(1, 3)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Dac {
    fn prototype() -> &'static Shape {
        shared(&DAC, dac_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Dac, Arc::clone(shared(&DAC, dac_shape))))
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

// ─── The analog side's own fixtures ─────────────────────────────────────────

/// One module per fixture type, built once.
pub fn modules() -> Modules {
    static M: [OnceLock<Module>; 14] = [const { OnceLock::new() }; 14];
    Modules {
        resistor: module_of::<Resistor>(&M[0], "Resistor"),
        diode: module_of::<Diode>(&M[1], "Diode"),
        capacitor: module_of::<Capacitor>(&M[2], "Capacitor"),
        flip_flop: module_of::<FlipFlop>(&M[3], "FlipFlop"),
        comparator: module_of::<Comparator>(&M[4], "Comparator"),
        dac: module_of::<Dac>(&M[5], "Dac"),
        cubic: module_of::<Cubic>(&M[6], "Cubic"),
        bad_limit: M[7].get_or_init(bad_limit_module),
        source: module_of::<CurrentSource>(&M[8], "CurrentSource"),
        counted: module_of::<CountedResistor>(&M[9], "CountedResistor"),
        probe: module_of::<Probe>(&M[10], "Probe"),
        faulty: module_of::<Faulty>(&M[11], "Faulty"),
        plain_diode: module_of::<PlainDiode>(&M[12], "PlainDiode"),
        vsource: module_of::<VSource>(&M[13], "VSource"),
    }
}

/// Every fixture module, by name.
#[derive(Clone, Copy)]
pub struct Modules {
    pub resistor: &'static Module,
    pub diode: &'static Module,
    pub capacitor: &'static Module,
    pub flip_flop: &'static Module,
    pub comparator: &'static Module,
    pub dac: &'static Module,
    pub cubic: &'static Module,
    pub bad_limit: &'static Module,
    pub source: &'static Module,
    pub counted: &'static Module,
    pub probe: &'static Module,
    pub faulty: &'static Module,
    pub plain_diode: &'static Module,
    pub vsource: &'static Module,
}

/// pins: k=0 (PARAM F64); p=1, n=2 (PORT continuous)
/// terms: T0 FLOW {p, n} "i" = k·v³, whose partials vanish at v = 0
/// fn 0 "i": reads [p, n, k] writes [T0] partials (0←0) (0←1) (0←2)
pub struct Cubic;
static CUBIC: OnceLock<ShapeRef> = OnceLock::new();

fn cubic_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let k = s.pin(param("k"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(p), pin(n), pin(k)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Cubic {
    fn prototype() -> &'static Shape {
        shared(&CUBIC, cubic_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Cubic, Arc::clone(shared(&CUBIC, cubic_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, k) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, k * v * v * v);
        let g = 3.0 * k * v * v;
        cx.jac().copy_from_slice(&[g, -g, v * v * v]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A shape that breaks abi.md A3: one function writes a limit and a term
/// without `OWNS_CORRECTION`.
/// pins: a=0, k=1 (PORT continuous)   inputs: I0 LIMIT "vd"   terms: T0 FLOW {a, k} "i"
/// fn 0 "both": reads [a, k] writes [I0.PROPOSED, I0.LIMITED, T0]
static BAD_LIMIT: OnceLock<ShapeRef> = OnceLock::new();

fn bad_limit_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let k = s.pin(PinSpec::continuous("k", role::PORT));
    let i0 = s.input(InputSpec { name: "vd", origin: "fixture:bad_limit", kind: input::LIMIT });
    let t0 = s.term(flow("i", op::FLOW, a, k));
    s.function(FunctionSpec {
        name: "both",
        origin: "fixture:bad_limit:fn0",
        reads: vec![pin(a), pin(k)],
        writes: vec![inp(i0, part::PROPOSED), inp(i0, part::LIMITED), term(t0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

/// A foreign module whose `create` hands out the shape unchecked, as a C
/// library could: the thunks of `piperine_abi::thunk` validate before they
/// return a shape, so only a hand-written module reaches bind with one that
/// breaks a rule. Its instance is never called.
fn bad_limit_module() -> Module {
    extern "C" fn create(_: *const Module, _: List<Value>, _: *mut Note) -> Created {
        let shape = shared(&BAD_LIMIT, bad_limit_shape).as_raw();
        Created { status: status::OK, instance: NonNull::dangling().as_ptr(), shape }
    }
    extern "C" fn destroy(_: *mut Instance) {}
    extern "C" fn eval(_: *mut Instance, _: u32, _: *const RawEval) -> u32 {
        status::FAULT
    }
    extern "C" fn react(_: *mut Instance, _: u32, _: *const RawReact) -> u32 {
        status::FAULT
    }
    Module {
        name: Str::from_static("BadLimit"),
        prototype: shared(&BAD_LIMIT, bad_limit_shape).as_raw(),
        data: std::ptr::null(),
        create,
        destroy,
        eval,
        react,
    }
}

// ─── Recording fixtures ─────────────────────────────────────────────────────

/// One call of a recording fixture: its tag, the bytes it read and which of
/// its partials were seeded.
#[derive(Clone, Debug)]
pub struct Call {
    pub tag: u64,
    pub reads: Vec<Vec<u8>>,
    pub seed: Vec<bool>,
}

static CALLS: Mutex<Vec<Call>> = Mutex::new(Vec::new());

fn record(tag: u64, cx: &Eval, partials: usize) {
    let call = Call {
        tag,
        reads: (0..cx.reads()).map(|i| cx.bytes(i).to_vec()).collect(),
        seed: (0..partials).map(|p| cx.wanted(p)).collect(),
    };
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).push(call);
}

/// Every recorded call of the fixture tagged `tag`, in order. Each test uses
/// its own tags, so parallel tests do not see each other's calls.
pub fn calls(tag: u64) -> Vec<Call> {
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|c| c.tag == tag).cloned().collect()
}

/// The tag a recording fixture was created with (its `tag` param, pin 0).
fn tag_of(values: &Values<'_>) -> u64 {
    values.f64(0).map_or(0, |t| t as u64)
}

/// pins: tag=0, idc=1 (PARAM F64); p=2, n=3 (PORT continuous)
/// terms: T0 FLOW {p, n} "i" = idc: a current source that reads only a param
/// fn 0 "i": reads [idc] writes [T0] partials (0←0)=1        (records every call)
pub struct CurrentSource {
    tag: u64,
}
static SOURCE: OnceLock<ShapeRef> = OnceLock::new();

fn source_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("tag"));
    let idc = s.pin(param("idc"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(idc)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for CurrentSource {
    fn prototype() -> &'static Shape {
        shared(&SOURCE, source_shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CurrentSource { tag: tag_of(values) }, Arc::clone(shared(&SOURCE, source_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        record(self.tag, cx, 1);
        let idc = cx.f64(0);
        cx.set(0, idc);
        cx.jac()[0] = 1.0;
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// pins: tag=0, r=1 (PARAM F64); p=2, n=3 (PORT continuous)
/// terms: T0 FLOW {p, n} "i"
/// fn 0 "i": reads [p, n, r] writes [T0] partials (0←0) (0←1) (0←2)   (records every call)
pub struct CountedResistor {
    tag: u64,
}
static COUNTED: OnceLock<ShapeRef> = OnceLock::new();

fn counted_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("tag"));
    let r = s.pin(param("r"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(p), pin(n), pin(r)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for CountedResistor {
    fn prototype() -> &'static Shape {
        shared(&COUNTED, counted_shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CountedResistor { tag: tag_of(values) }, Arc::clone(shared(&COUNTED, counted_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        record(self.tag, cx, 3);
        let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, v / r);
        cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// pins: tag=0 (PARAM F64); p=1 (PORT continuous)
/// terms: T0 FLOW {p, GROUND} "g" = v(p): a 1 S conductance
/// fn 0 "g": reads [p, TIME, FREQ, STATIC, GMIN, SOURCE_SCALE] writes [T0] partials (0←0)=1   (records)
pub struct Probe {
    tag: u64,
}
static PROBE: OnceLock<ShapeRef> = OnceLock::new();

fn probe_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("tag"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let t0 = s.term(flow("g", op::FLOW, p, GROUND));
    s.function(FunctionSpec {
        name: "g",
        reads: vec![
            pin(p),
            ambient(ambient::TIME),
            ambient(ambient::FREQ),
            ambient(ambient::STATIC),
            ambient(ambient::GMIN),
            ambient(ambient::SOURCE_SCALE),
        ],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Probe {
    fn prototype() -> &'static Shape {
        shared(&PROBE, probe_shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Probe { tag: tag_of(values) }, Arc::clone(shared(&PROBE, probe_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        record(self.tag, cx, 1);
        let v = cx.f64(0);
        cx.set(0, v);
        cx.jac()[0] = 1.0;
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// pins: mode=0 (PARAM F64); p=1, n=2 (PORT continuous)
/// terms: T0 FLOW {p, n} "i" = v(p) − v(n), origin "fixture:faulty:T0"
/// fn 0 "i": reads [p, n, mode] writes [T0] partials (0←0) (0←1) (0←2)
/// mode 1: the value is NaN; 2: the seeded partial (0←0) is +inf; 3: the
/// partial against the param (never seeded in a Newton solve) is +inf;
/// 4: the call faults with code 7.
pub struct Faulty;
static FAULTY: OnceLock<ShapeRef> = OnceLock::new();

fn faulty_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let mode = s.pin(param("mode"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { origin: "fixture:faulty:T0", ..flow("i", op::FLOW, p, n) });
    s.function(FunctionSpec {
        name: "i",
        origin: "fixture:faulty:fn0",
        reads: vec![pin(p), pin(n), pin(mode)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Faulty {
    fn prototype() -> &'static Shape {
        shared(&FAULTY, faulty_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Faulty, Arc::clone(shared(&FAULTY, faulty_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let mode = cx.f64(2);
        if mode == 4.0 {
            return Err(Fault::new(7, "mode four"));
        }
        let v = cx.f64(0) - cx.f64(1);
        cx.set(0, if mode == 1.0 { f64::NAN } else { v });
        let g = if mode == 2.0 { f64::INFINITY } else { 1.0 };
        let dm = if mode == 3.0 { f64::INFINITY } else { 0.0 };
        cx.jac().copy_from_slice(&[g, -1.0, dm]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── Plain devices for analytic circuits ────────────────────────────────────

/// The thermal voltage of the plain diode, as `tests/helpers/devices.rs` of
/// `piperine-solver` writes it.
pub const VT: f64 = 0.02585;

/// pins: is=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// terms: T0 FLOW {a, k} "id" = is·(exp(v/VT) − 1), no limiting
/// fn 0 "id": reads [a, k, is] writes [T0] partials (0←0) (0←1) (0←2)
pub struct PlainDiode;
static PLAIN_DIODE: OnceLock<ShapeRef> = OnceLock::new();

fn plain_diode_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let is = s.pin(param("is"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let k = s.pin(PinSpec::continuous("k", role::PORT));
    let t0 = s.term(flow("id", op::FLOW, a, k));
    s.function(FunctionSpec {
        name: "id",
        reads: vec![pin(a), pin(k), pin(is)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for PlainDiode {
    fn prototype() -> &'static Shape {
        shared(&PLAIN_DIODE, plain_diode_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((PlainDiode, Arc::clone(shared(&PLAIN_DIODE, plain_diode_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, is) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        let e = (v / VT).exp();
        cx.set(0, is * (e - 1.0));
        let gd = is * e / VT;
        cx.jac().copy_from_slice(&[gd, -gd, e - 1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A potential source written as abi.md A1 says: a private continuous pin
/// `f` carries the branch flow.
/// pins: dc=0 (PARAM F64); p=1, n=2 (PORT continuous); f=3 (PRIVATE continuous)
/// terms: T0 FLOW {p, n} "i" = f;  T1 FLOW {f, GROUND} "eq" = x(p) − x(n) − dc·SOURCE_SCALE
/// fn 0 "v": reads [p, n, f, dc, SOURCE_SCALE] writes [T0, T1]
///           partials (0←2)=1 (1←0)=1 (1←1)=−1 (1←3)=−SOURCE_SCALE
pub struct VSource;
static VSOURCE: OnceLock<ShapeRef> = OnceLock::new();

fn vsource_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let dc = s.pin(param("dc"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", op::FLOW, p, n));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    s.function(FunctionSpec {
        name: "v",
        reads: vec![pin(p), pin(n), pin(f), pin(dc), ambient(ambient::SOURCE_SCALE)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 0), partial(1, 1), partial(1, 3)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for VSource {
    fn prototype() -> &'static Shape {
        shared(&VSOURCE, vsource_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((VSource, Arc::clone(shared(&VSOURCE, vsource_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let scale = cx.f64(4);
        cx.set(0, cx.f64(2));
        cx.set(1, cx.f64(0) - cx.f64(1) - cx.f64(3) * scale);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -scale]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}
