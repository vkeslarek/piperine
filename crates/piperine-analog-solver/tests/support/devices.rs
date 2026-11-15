//! Fixture devices of the analog assembly's targets. The worked examples of
//! abi.md §11.1 to §11.6 are copied constant for constant from
//! `crates/piperine-abi/tests/support/examples.rs` (resistor, diode with
//! limiting and noise, capacitor, D flip-flop, comparator, DAC); the devices
//! after them are this crate's own, each built for one rule of the analog
//! side. Every shape lists its `PARAM` pins first (P5).

use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW, REAL};
use piperine_abi::raw::{
    Cell, Created, DEFAULT, Eval as RawEval, GROUND, Instance, List, Module, Note, Partial, React as RawReact, Shape,
    Statement, Str, Value, ambient, cell, fn_flag, global, input, op, part, pin_flag, role, status, watch,
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    static M: [OnceLock<Module>; 27] = [const { OnceLock::new() }; 27];
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
        clamp: module_of::<Clamp>(&M[14], "Clamp"),
        owner: module_of::<Owner>(&M[15], "Owner"),
        reader: module_of::<Reader>(&M[16], "Reader"),
        tracker: module_of::<Tracker>(&M[17], "Tracker"),
        delay_line: module_of::<DelayLine>(&M[18], "DelayLine"),
        trio: module_of::<Trio>(&M[19], "Trio"),
        ac_vsource: module_of::<AcVSource>(&M[20], "AcVSource"),
        noisy: module_of::<NoisyResistor>(&M[21], "NoisyResistor"),
        split: module_of::<Split>(&M[22], "Split"),
        flicker: module_of::<Flicker>(&M[23], "Flicker"),
        capper: module_of::<Capper>(&M[24], "Capper"),
        tracer: module_of::<Tracer>(&M[25], "Tracer"),
        sample_hold: module_of::<SampleHold>(&M[26], "SampleHold"),
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
    pub clamp: &'static Module,
    pub owner: &'static Module,
    pub reader: &'static Module,
    pub tracker: &'static Module,
    pub delay_line: &'static Module,
    pub trio: &'static Module,
    pub ac_vsource: &'static Module,
    pub noisy: &'static Module,
    pub split: &'static Module,
    pub flicker: &'static Module,
    pub capper: &'static Module,
    pub tracer: &'static Module,
    pub sample_hold: &'static Module,
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
    fn prototype() -> &'static Shape<'static> {
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
    // SAFETY: the prototype is a frozen static shape that lives for the
    // process; `create` hands out that shape and a never-dereferenced
    // instance, `destroy` frees nothing, and `eval`/`react` read nothing.
    unsafe {
        Module::from_raw_parts(
            Str::from_static("BadLimit"),
            shared(&BAD_LIMIT, bad_limit_shape).as_raw(),
            std::ptr::null(),
            create,
            destroy,
            eval,
            react,
        )
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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
    fn prototype() -> &'static Shape<'static> {
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

// ─── Limiting fixtures ──────────────────────────────────────────────────────

/// The conductance scale of the limiting fixtures.
pub const G_LIMIT: f64 = 1e-3;

fn limit_reads_and_partials(a: u32, k: u32, extra: Vec<Cell>) -> (Vec<Cell>, Vec<Partial>) {
    let mut reads = vec![pin(a), pin(k)];
    reads.extend(extra);
    (reads, vec![partial(0, 0), partial(0, 1)])
}

/// A limit whose held iterate is offset from the proposed value by a param.
/// pins: off=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, k} "i" = G_LIMIT·x̂²
/// fn 0 "limit": reads [a, k, off] writes [I0.PROPOSED = v(a) − v(k), I0.LIMITED = PROPOSED + off]
///               partials (0←0)=1 (0←1)=−1
/// fn 1 "i":     reads [I0] writes [T0] partials (0←0)=2·G_LIMIT·x̂
pub struct Clamp;
static CLAMP: OnceLock<ShapeRef> = OnceLock::new();

fn clamp_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let off = s.pin(param("off"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let k = s.pin(PinSpec::continuous("k", role::PORT));
    let i0 = s.input(InputSpec { name: "x", origin: "fixture:clamp:I0", kind: input::LIMIT });
    let t0 = s.term(flow("i", op::FLOW, a, k));
    let (reads, partials) = limit_reads_and_partials(a, k, vec![pin(off)]);
    s.function(FunctionSpec {
        name: "limit",
        reads,
        writes: vec![inp(i0, part::PROPOSED), inp(i0, part::LIMITED)],
        partials,
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "i",
        reads: vec![inp(i0, part::VALUE)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Clamp {
    fn prototype() -> &'static Shape<'static> {
        shared(&CLAMP, clamp_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Clamp, Arc::clone(shared(&CLAMP, clamp_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let x = cx.f64(0) - cx.f64(1);
            let off = cx.f64(2);
            cx.set(0, x);
            cx.set(1, x + off);
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let xh = cx.f64(0);
            cx.set(0, G_LIMIT * xh * xh);
            cx.jac()[0] = 2.0 * G_LIMIT * xh;
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A function that writes a limit and the term depending on it in one call
/// (a compiled foreign model, abi.md A3): flagged `OWNS_CORRECTION`.
/// pins: off=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, k} "i" = G_LIMIT·(v(a) − v(k)) + 1e-3
/// fn 0 "all" OWNS_CORRECTION: reads [a, k, off]
///     writes [I0.PROPOSED = v, I0.LIMITED = v + off, T0]
///     partials (0←0)=1 (0←1)=−1 (2←0)=G_LIMIT (2←1)=−G_LIMIT
pub struct Owner;
static OWNER: OnceLock<ShapeRef> = OnceLock::new();

fn owner_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let off = s.pin(param("off"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let k = s.pin(PinSpec::continuous("k", role::PORT));
    let i0 = s.input(InputSpec { name: "x", origin: "fixture:owner:I0", kind: input::LIMIT });
    let t0 = s.term(flow("i", op::FLOW, a, k));
    s.function(FunctionSpec {
        name: "all",
        flags: fn_flag::OWNS_CORRECTION,
        reads: vec![pin(a), pin(k), pin(off)],
        writes: vec![inp(i0, part::PROPOSED), inp(i0, part::LIMITED), term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(2, 0), partial(2, 1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Owner {
    fn prototype() -> &'static Shape<'static> {
        shared(&OWNER, owner_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Owner, Arc::clone(shared(&OWNER, owner_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let v = cx.f64(0) - cx.f64(1);
        cx.set(0, v);
        cx.set(1, v + cx.f64(2));
        cx.set(2, G_LIMIT * v + 1e-3);
        cx.jac().copy_from_slice(&[1.0, -1.0, G_LIMIT, -G_LIMIT]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A limit that no function writes, read by one (abi.md A3).
/// pins: tag=0 (PARAM F64); a=1 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, GROUND} "i" = 2·x̂
/// fn 0 "i": reads [I0] writes [T0] partials (0←0)=2        (records every call)
pub struct Reader {
    tag: u64,
}
static READER: OnceLock<ShapeRef> = OnceLock::new();

fn reader_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("tag"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let i0 = s.input(InputSpec { name: "x", origin: "fixture:reader:I0", kind: input::LIMIT });
    let t0 = s.term(flow("i", op::FLOW, a, GROUND));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![inp(i0, part::VALUE)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Reader {
    fn prototype() -> &'static Shape<'static> {
        shared(&READER, reader_shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Reader { tag: tag_of(values) }, Arc::clone(shared(&READER, reader_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        record(self.tag, cx, 1);
        let xh = cx.f64(0);
        cx.set(0, 2.0 * xh);
        cx.jac()[0] = 2.0;
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A limit that never limits, recording what its function reads of it.
/// pins: tag=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, k} "i" = G_LIMIT·x̂
/// fn 0 "limit": reads [a, k, I0.LAST, I0.AGE, STATIC] writes [I0.PROPOSED, I0.LIMITED] = v, v
///               partials (0←0)=1 (0←1)=−1            (records every call)
/// fn 1 "i":     reads [I0] writes [T0] partials (0←0)=G_LIMIT
pub struct Tracker {
    tag: u64,
}
static TRACKER: OnceLock<ShapeRef> = OnceLock::new();

fn tracker_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("tag"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let k = s.pin(PinSpec::continuous("k", role::PORT));
    let i0 = s.input(InputSpec { name: "x", origin: "fixture:tracker:I0", kind: input::LIMIT });
    let t0 = s.term(flow("i", op::FLOW, a, k));
    let extra = vec![inp(i0, part::LAST), inp(i0, part::AGE), ambient(ambient::STATIC)];
    let (reads, partials) = limit_reads_and_partials(a, k, extra);
    s.function(FunctionSpec {
        name: "limit",
        reads,
        writes: vec![inp(i0, part::PROPOSED), inp(i0, part::LIMITED)],
        partials,
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "i",
        reads: vec![inp(i0, part::VALUE)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Tracker {
    fn prototype() -> &'static Shape<'static> {
        shared(&TRACKER, tracker_shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Tracker { tag: tag_of(values) }, Arc::clone(shared(&TRACKER, tracker_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            record(self.tag, cx, 2);
            let v = cx.f64(0) - cx.f64(1);
            cx.set(0, v);
            cx.set(1, v);
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let xh = cx.f64(0);
            cx.set(0, G_LIMIT * xh);
            cx.jac()[0] = G_LIMIT;
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── Delay fixture ──────────────────────────────────────────────────────────

/// pins: td=0, max=1 (PARAM F64); a=2, o=3 (PORT continuous)
/// terms: T0 DELAY {o, GROUND} "d" = v(a), delay td, bound `max`;  origin "fixture:delay:T0"
/// fn 0 "d": reads [a, td] writes [T0.VALUE, T0.TIME] partials (0←0)=1
pub struct DelayLine;
static DELAY_LINE: OnceLock<ShapeRef> = OnceLock::new();

fn delay_line_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let td = s.pin(param("td"));
    let max = s.pin(param("max"));
    let a = s.pin(PinSpec::continuous("a", role::PORT));
    let o = s.pin(PinSpec::continuous("o", role::PORT));
    let t0 = s.term(TermSpec { origin: "fixture:delay:T0", max, ..flow("d", op::DELAY, o, GROUND) });
    s.function(FunctionSpec {
        name: "d",
        reads: vec![pin(a), pin(td)],
        writes: vec![term(t0), Cell { kind: cell::TERM, index: t0, part: part::TIME }],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for DelayLine {
    fn prototype() -> &'static Shape<'static> {
        shared(&DELAY_LINE, delay_line_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((DelayLine, Arc::clone(shared(&DELAY_LINE, delay_line_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, td) = (cx.f64(0), cx.f64(1));
        cx.set(0, v);
        cx.set(1, td);
        cx.jac()[0] = 1.0;
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── Small-signal fixtures ──────────────────────────────────────────────────

/// One term of each operator, each on its own pin.
/// pins: a=0, b=1, c=2, td=3 (PARAM F64); p0=4, p1=5, p2=6 (PORT continuous)
/// terms: T0 FLOW {p0, GROUND} = a·v(p0);  T1 DDT {p1, GROUND} = b·v(p1);
///        T2 DELAY {p2, GROUND} = c·v(p2), delay td
/// fn 0 "all": reads [p0, p1, p2, a, b, c, td] writes [T0, T1, T2, T2.TIME]
///             partials (0←0)=a (1←1)=b (2←2)=c
pub struct Trio;
static TRIO: OnceLock<ShapeRef> = OnceLock::new();

fn trio_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let (a, b, c, td) = (s.pin(param("a")), s.pin(param("b")), s.pin(param("c")), s.pin(param("td")));
    let p0 = s.pin(PinSpec::continuous("p0", role::PORT));
    let p1 = s.pin(PinSpec::continuous("p1", role::PORT));
    let p2 = s.pin(PinSpec::continuous("p2", role::PORT));
    let t0 = s.term(flow("f", op::FLOW, p0, GROUND));
    let t1 = s.term(flow("q", op::DDT, p1, GROUND));
    let t2 = s.term(flow("d", op::DELAY, p2, GROUND));
    s.function(FunctionSpec {
        name: "all",
        reads: vec![pin(p0), pin(p1), pin(p2), pin(a), pin(b), pin(c), pin(td)],
        writes: vec![term(t0), term(t1), term(t2), Cell { kind: cell::TERM, index: t2, part: part::TIME }],
        partials: vec![partial(0, 0), partial(1, 1), partial(2, 2)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Trio {
    fn prototype() -> &'static Shape<'static> {
        shared(&TRIO, trio_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Trio, Arc::clone(shared(&TRIO, trio_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (a, b, c, td) = (cx.f64(3), cx.f64(4), cx.f64(5), cx.f64(6));
        cx.set(0, a * cx.f64(0));
        cx.set(1, b * cx.f64(1));
        cx.set(2, c * cx.f64(2));
        cx.set(3, td);
        cx.jac().copy_from_slice(&[a, b, c]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// The potential source of abi.md A1 with a small-signal stimulus.
/// pins: dc=0, re=1, im=2 (PARAM F64); p=3, n=4 (PORT continuous); f=5 (PRIVATE continuous)
/// inputs: I0 STIM "ac"
/// terms: T0 FLOW {p, n} "i" = f;  T1 FLOW {f, GROUND} "eq" = x(p) − x(n) − dc − I0
/// fn 0 "v":  reads [p, n, f, dc, I0] writes [T0, T1] partials (0←2)=1 (1←0)=1 (1←1)=−1 (1←4)=−1
/// fn 1 "ac": reads [re, im] writes [I0.RE, I0.IM]
pub struct AcVSource;
static AC_VSOURCE: OnceLock<ShapeRef> = OnceLock::new();

fn ac_vsource_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let (dc, re, im) = (s.pin(param("dc")), s.pin(param("re")), s.pin(param("im")));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let i0 = s.input(InputSpec { name: "ac", origin: "fixture:ac_vsource:I0", kind: input::STIM });
    let t0 = s.term(flow("i", op::FLOW, p, n));
    let t1 = s.term(flow("eq", op::FLOW, f, GROUND));
    s.function(FunctionSpec {
        name: "v",
        reads: vec![pin(p), pin(n), pin(f), pin(dc), inp(i0, part::VALUE)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 0), partial(1, 1), partial(1, 4)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "ac",
        reads: vec![pin(re), pin(im)],
        writes: vec![inp(i0, part::RE), inp(i0, part::IM)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for AcVSource {
    fn prototype() -> &'static Shape<'static> {
        shared(&AC_VSOURCE, ac_vsource_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AcVSource, Arc::clone(shared(&AC_VSOURCE, ac_vsource_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            cx.set(0, cx.f64(2));
            cx.set(1, cx.f64(0) - cx.f64(1) - cx.f64(3) - cx.f64(4));
            cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -1.0]);
        } else {
            cx.set(0, cx.f64(0));
            cx.set(1, cx.f64(1));
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── Noise fixtures ─────────────────────────────────────────────────────────

/// The temperature of the noise fixtures, kelvin.
pub const T_NOISE: f64 = 300.15;

/// pins: r=0 (PARAM F64); p=1, n=2 (PORT continuous)
/// inputs: I0 NOISE "thermal"       terms: T0 FLOW {p, n} "i" = v/r + I0
/// fn 0 "i":   reads [p, n, r, I0] writes [T0] partials (0←0)=1/r (0←1)=−1/r (0←2)=−v/r² (0←3)=1
/// fn 1 "psd": reads [r] writes [I0.PSD] = 4·k·T/r
pub struct NoisyResistor;
static NOISY: OnceLock<ShapeRef> = OnceLock::new();

fn noisy_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let r = s.pin(param("r"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let i0 = s.input(InputSpec { name: "thermal", origin: "fixture:noisy:I0", kind: input::NOISE });
    let t0 = s.term(flow("i", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(p), pin(n), pin(r), inp(i0, part::VALUE)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1), partial(0, 2), partial(0, 3)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "psd",
        reads: vec![pin(r)],
        writes: vec![inp(i0, part::PSD)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for NoisyResistor {
    fn prototype() -> &'static Shape<'static> {
        shared(&NOISY, noisy_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NoisyResistor, Arc::clone(shared(&NOISY, noisy_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
            cx.set(0, v / r + cx.f64(3));
            cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r), 1.0]);
        } else {
            cx.set(0, 4.0 * K_B * T_NOISE / cx.f64(0));
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A resistor split into two parallel halves, each with a thermal source;
/// with `shared` = 1 the second half reads the first half's input with
/// partial −1 (one source on two paths), with `shared` = 0 its own input.
/// pins: r=0, shared=1 (PARAM F64); p=2, n=3 (PORT continuous)
/// inputs: I0 NOISE "a", I1 NOISE "b"
/// terms: T0 FLOW {p, n} "ha" = v/(2r) + I0;  T1 FLOW {p, n} "hb" = v/(2r) − shared·I0 + (1 − shared)·I1
/// fn 0 "i":   reads [p, n, r, shared, I0, I1] writes [T0, T1]
///             partials (0←0) (0←1) (0←4)=1 (1←0) (1←1) (1←4)=−shared (1←5)=1−shared
/// fn 1 "psd": reads [r] writes [I0.PSD, I1.PSD] = 4·k·T/(2r) each
pub struct Split;
static SPLIT: OnceLock<ShapeRef> = OnceLock::new();

fn split_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let r = s.pin(param("r"));
    let sh = s.pin(param("shared"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let i0 = s.input(InputSpec { name: "a", origin: "fixture:split:I0", kind: input::NOISE });
    let i1 = s.input(InputSpec { name: "b", origin: "fixture:split:I1", kind: input::NOISE });
    let t0 = s.term(flow("ha", op::FLOW, p, n));
    let t1 = s.term(flow("hb", op::FLOW, p, n));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(p), pin(n), pin(r), pin(sh), inp(i0, part::VALUE), inp(i1, part::VALUE)],
        writes: vec![term(t0), term(t1)],
        partials: vec![
            partial(0, 0),
            partial(0, 1),
            partial(0, 4),
            partial(1, 0),
            partial(1, 1),
            partial(1, 4),
            partial(1, 5),
        ],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "psd",
        reads: vec![pin(r)],
        writes: vec![inp(i0, part::PSD), inp(i1, part::PSD)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Split {
    fn prototype() -> &'static Shape<'static> {
        shared(&SPLIT, split_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Split, Arc::clone(shared(&SPLIT, split_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let (v, r, sh) = (cx.f64(0) - cx.f64(1), cx.f64(2), cx.f64(3));
            let g = 1.0 / (2.0 * r);
            cx.set(0, g * v + cx.f64(4));
            cx.set(1, g * v - sh * cx.f64(4) + (1.0 - sh) * cx.f64(5));
            cx.jac().copy_from_slice(&[g, -g, 1.0, g, -g, -sh, 1.0 - sh]);
        } else {
            let psd = 4.0 * K_B * T_NOISE / (2.0 * cx.f64(0));
            cx.set(0, psd);
            cx.set(1, psd);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A flicker source whose density reads `FREQ`.
/// pins: tag=0 (PARAM F64); p=1 (PORT continuous)
/// inputs: I0 NOISE "flicker"       terms: T0 FLOW {p, GROUND} "i" = G_LIMIT·v + I0
/// fn 0 "i":   reads [p, I0] writes [T0] partials (0←0)=G_LIMIT (0←1)=1
/// fn 1 "psd": reads [FREQ] writes [I0.PSD] = KF/freq        (records every call)
pub struct Flicker {
    tag: u64,
}
static FLICKER: OnceLock<ShapeRef> = OnceLock::new();

fn flicker_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(param("tag"));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let i0 = s.input(InputSpec { name: "flicker", origin: "fixture:flicker:I0", kind: input::NOISE });
    let t0 = s.term(flow("i", op::FLOW, p, GROUND));
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(p), inp(i0, part::VALUE)],
        writes: vec![term(t0)],
        partials: vec![partial(0, 0), partial(0, 1)],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "psd",
        reads: vec![ambient(ambient::FREQ)],
        writes: vec![inp(i0, part::PSD)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Flicker {
    fn prototype() -> &'static Shape<'static> {
        shared(&FLICKER, flicker_shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Flicker { tag: tag_of(values) }, Arc::clone(shared(&FLICKER, flicker_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            cx.set(0, G_LIMIT * cx.f64(0) + cx.f64(1));
            cx.jac().copy_from_slice(&[G_LIMIT, 1.0]);
        } else {
            record(self.tag, cx, 0);
            cx.set(0, KF / cx.f64(0));
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ─── Watch fixtures ─────────────────────────────────────────────────────────

/// Two step-size caps.
/// pins: c1=0, c2=1 (PARAM F64); p=2 (PORT continuous)
/// watches: W0 CAP "a" = c1·(1 + v(p)), W1 CAP "b" = c2
/// fn 0 "caps": reads [p, c1, c2] writes [W0, W1]
pub struct Capper;
static CAPPER: OnceLock<ShapeRef> = OnceLock::new();

fn cap_watch(name: &'static str) -> WatchSpec<'static> {
    WatchSpec { name, origin: "fixture:capper", kind: watch::CAP, layout: &F64, event: 0, ttol: DEFAULT, etol: DEFAULT }
}

fn capper_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let (c1, c2) = (s.pin(param("c1")), s.pin(param("c2")));
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let w0 = s.watch(cap_watch("a"));
    let w1 = s.watch(cap_watch("b"));
    let w = |index| Cell { kind: cell::WATCH, index, part: part::VALUE };
    s.function(FunctionSpec {
        name: "caps",
        reads: vec![pin(p), pin(c1), pin(c2)],
        writes: vec![w(w0), w(w1)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Capper {
    fn prototype() -> &'static Shape<'static> {
        shared(&CAPPER, capper_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capper, Arc::clone(shared(&CAPPER, capper_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c1, c2) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, c1 * (1.0 + v));
        cx.set(1, c2);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A conductance that traces its voltage whenever it exceeds 0.5 V.
/// pins: p=0 (PORT continuous)
/// terms: T0 FLOW {p, GROUND} "g" = v(p)     watches: W0 TRACE "hi" (layout REAL)
/// fn 0 "g": reads [p] writes [T0, W0] partials (0←0)=1; W0 = {v, known} when v > 0.5,
///           else left empty (all zero, P3)
pub struct Tracer;
static TRACER: OnceLock<ShapeRef> = OnceLock::new();

fn tracer_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let t0 = s.term(flow("g", op::FLOW, p, GROUND));
    let w0 = s.watch(WatchSpec {
        name: "hi",
        origin: "fixture:tracer:W0",
        kind: watch::TRACE,
        layout: &REAL,
        event: 0,
        ttol: DEFAULT,
        etol: DEFAULT,
    });
    s.function(FunctionSpec {
        name: "g",
        reads: vec![pin(p)],
        writes: vec![term(t0), Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
        partials: vec![partial(0, 0)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Tracer {
    fn prototype() -> &'static Shape<'static> {
        shared(&TRACER, tracer_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Tracer, Arc::clone(shared(&TRACER, tracer_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let v = cx.f64(0);
        cx.set(0, v);
        cx.jac()[0] = 1.0;
        if v > 0.5 {
            let rec = cx.out_bytes(1);
            rec[..8].copy_from_slice(&v.to_le_bytes());
            rec[8] = 1;
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A sample-and-hold: a reaction reads a continuous pin.
/// pins: inp=0 (PORT continuous); clk=1 (PORT event); out=2 (PORT REAL)
/// reaction 0 "sample": on [clk] reads [inp] writes [S0 {out}]
pub struct SampleHold;
static SAMPLE_HOLD: OnceLock<ShapeRef> = OnceLock::new();

fn sample_hold_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let inp_ = s.pin(PinSpec::continuous("inp", role::PORT));
    let clk = s.pin(PinSpec::event("clk", role::PORT));
    let out = s.pin(PinSpec::discrete("out", role::PORT, &REAL));
    s.reaction(ReactionSpec {
        name: "sample",
        on: vec![clk],
        reads: vec![pin(inp_)],
        writes: vec![Statement { pin: out, offset: 0, len: 16, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for SampleHold {
    fn prototype() -> &'static Shape<'static> {
        shared(&SAMPLE_HOLD, sample_hold_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SampleHold, Arc::clone(shared(&SAMPLE_HOLD, sample_hold_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let mut real = [0u8; 16];
        real[..8].copy_from_slice(&cx.f64(0).to_le_bytes());
        real[8] = 1;
        cx.commit(0, &real, 0.0)?;
        Ok(Flow::Continue)
    }
}
