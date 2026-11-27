//! The devices of the solver target `parity_baseline`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, ambient, cell, global, op, part, pin_flag, role, watch,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "parity_baseline.rs";
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();
const ONE_MS: [u8; 8] = 1e-3_f64.to_le_bytes();

// ─────────────────────────── Analog device doubles ───────────────────────────
// The resistor, capacitor, DC source, AC source and inductor are those of
// `helpers/abi_devices.rs`; the diode and the sine source keep this target's
// own constants (`vt = 0.025_852`, `std::f64::consts::PI`).

/// Shockley diode `i = is·(exp(v/vt) − 1)` between `p` and `n`, `vt = 0.025_852`.
/// pins: p=0, n=1 (PORT); terms T0 FLOW{p, n}; fn 0: reads [p, n] writes [T0] partials (0<-0) (0<-1)
pub struct FixedDiode;
static DIODE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
const IS: f64 = 1e-14;
const VT: f64 = 0.025_852;

impl Device for FixedDiode {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineShapes::shared(&DIODE_SHAPE, ParityBaselineShapes::diode_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FixedDiode, Arc::clone(ParityBaselineShapes::shared(&DIODE_SHAPE, ParityBaselineShapes::diode_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let vd = cx.f64(0) - cx.f64(1);
        let ex = (vd / VT).exp();
        let gd = IS / VT * ex;
        cx.set(0, IS * (ex - 1.0));
        cx.jac().copy_from_slice(&[gd, -gd]);
        Ok(())
    }
}

/// Sine voltage source `amp·sin(2π·freq·t)` on a private branch current `f`;
/// at the t=0 operating point it is `sin(0) = 0`.
/// pins: p=0, n=1 (PORT); f=2 (PRIVATE); terms T0 FLOW{p, n}, T1 FLOW{f, GROUND}
/// fn 0: reads [p, n, f, TIME] writes [T0, T1] partials (0<-f) (1<-p) (1<-n)
pub struct SineVsrc;
static SINE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
const AMP: f64 = 1.0;
const FREQ: f64 = 1000.0;

impl Device for SineVsrc {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineShapes::shared(&SINE_SHAPE, ParityBaselineShapes::sine_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SineVsrc, Arc::clone(ParityBaselineShapes::shared(&SINE_SHAPE, ParityBaselineShapes::sine_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let v = AMP * (2.0 * std::f64::consts::PI * FREQ * cx.f64(3)).sin();
        cx.set(0, cx.f64(2));
        cx.set(1, cx.f64(0) - cx.f64(1) - v);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
}

static DIODE: OnceLock<Module> = OnceLock::new();
static SINE: OnceLock<Module> = OnceLock::new();

// ────────────────────── T2 mixed-signal / digital baselines ──────────────────

/// Analog comparator that samples a node voltage and drives a digital net (A2D):
/// a reaction on `START` and on each crossing of `threshold` writes
/// `out = v ≥ threshold`.
/// pins: threshold=0 (PARAM F64, default 1.0); sense=1 (PORT continuous); out=2 (PORT BIT);
///       up=3, down=4 (PRIVATE event); start=5 (GLOBAL START)
/// watches: W0 CROSS -> up (v − threshold), W1 CROSS -> down (threshold − v)
/// fn 0: reads [sense, threshold] writes [W0, W1] partials (0<-0) (1<-0)
/// reaction 0: on [start, up, down] reads [sense, threshold] writes [S0 {out}]
pub struct ThresholdComparator;
static CMP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CMP: OnceLock<Module> = OnceLock::new();

impl Device for ThresholdComparator {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineShapes::shared(&CMP_SHAPE, ParityBaselineShapes::cmp_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ThresholdComparator, Arc::clone(ParityBaselineShapes::shared(&CMP_SHAPE, ParityBaselineShapes::cmp_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let e = cx.f64(0) - cx.f64(1);
        cx.set(0, e);
        cx.set(1, -e);
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let level = if cx.f64(0) >= cx.f64(1) { HIGH } else { LOW };
        cx.commit(0, &level, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// Digital-gated conductance to ground: when `ctrl` is HIGH it adds `g` from
/// `node` to ground, otherwise nothing (D2A).
/// pins: g=0 (PARAM F64, default 1 mS); ctrl=1 (PORT BIT); node=2 (PORT continuous)
/// terms: T0 FLOW{node, GROUND}; fn 0: reads [ctrl, node, g] writes [T0] partials (0<-1)
pub struct GatedISink;
static SINK_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SINK: OnceLock<Module> = OnceLock::new();

impl Device for GatedISink {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineShapes::shared(&SINK_SHAPE, ParityBaselineShapes::sink_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((GatedISink, Arc::clone(ParityBaselineShapes::shared(&SINK_SHAPE, ParityBaselineShapes::sink_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = if cx.bytes(0) == HIGH { cx.f64(2) } else { 0.0 };
        cx.set(0, g * cx.f64(1));
        cx.jac().copy_from_slice(&[g]);
        Ok(())
    }
}

// ───────────────────── Digital scheduler snapshot (topology) ──────────────────

/// Constant digital source: `out` is HIGH from power-on (its `init`).
/// pins: out=0 (PORT BIT, init HIGH)
pub struct DigitalSource;
static SRC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SRC: OnceLock<Module> = OnceLock::new();

impl Device for DigitalSource {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineShapes::shared(&SRC_SHAPE, ParityBaselineShapes::src_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((DigitalSource, Arc::clone(ParityBaselineShapes::shared(&SRC_SHAPE, ParityBaselineShapes::src_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
}
impl ThresholdComparator {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        CMP.get_or_init(|| module::<ThresholdComparator>("Comparator"))
    }
}

impl DigitalSource {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SRC.get_or_init(|| module::<DigitalSource>("DigitalSource"))
    }
}

impl FixedDiode {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        DIODE.get_or_init(|| module::<FixedDiode>("Diode"))
    }
}

impl GatedISink {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SINK.get_or_init(|| module::<GatedISink>("GatedISink"))
    }
}

impl SineVsrc {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SINE.get_or_init(|| module::<SineVsrc>("SineVsrc"))
    }
}

/// The shapes the `parity_baseline` devices are built from.
pub struct ParityBaselineShapes;

impl ParityBaselineShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of the value of term `index`.
    pub fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    pub fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The term spec named `name` of operator `operator`, between pins `plus` and `minus`.
    pub fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The shape `build` makes, built the first time and kept in `slot` so every instance shares it.
    pub fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// The shape of `FixedDiode`, built once and shared by every instance.
    pub fn diode_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ParityBaselineShapes::flow("i", p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![ParityBaselineShapes::pin(p), ParityBaselineShapes::pin(n)],
            writes: vec![ParityBaselineShapes::term(t0)],
            partials: vec![ParityBaselineShapes::partial(0, 0), ParityBaselineShapes::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `SineVsrc`, built once and shared by every instance.
    pub fn sine_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(ParityBaselineShapes::flow("i", p, n));
        let t1 = s.term(ParityBaselineShapes::flow("eq", f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![ParityBaselineShapes::pin(p), ParityBaselineShapes::pin(n), ParityBaselineShapes::pin(f), Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }],
            writes: vec![ParityBaselineShapes::term(t0), ParityBaselineShapes::term(t1)],
            partials: vec![ParityBaselineShapes::partial(0, 2), ParityBaselineShapes::partial(1, 0), ParityBaselineShapes::partial(1, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `ThresholdComparator`, built once and shared by every instance.
    pub fn cmp_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let th = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("threshold", role::PARAM, &F64) });
        let sense = s.pin(PinSpec::continuous("sense", role::PORT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let down = s.pin(PinSpec::event("down", role::PRIVATE));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let w = |name, event| WatchSpec { name, origin: ORIGIN, kind: watch::CROSS, layout: &F64, event, ttol: DEFAULT, etol: DEFAULT };
        let w0 = s.watch(w("rise", up));
        let w1 = s.watch(w("fall", down));
        let wc = |index| Cell { kind: cell::WATCH, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "e",
            origin: ORIGIN,
            reads: vec![ParityBaselineShapes::pin(sense), ParityBaselineShapes::pin(th)],
            writes: vec![wc(w0), wc(w1)],
            partials: vec![ParityBaselineShapes::partial(0, 0), ParityBaselineShapes::partial(1, 0)],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "sample",
            origin: ORIGIN,
            on: vec![start, up, down],
            reads: vec![ParityBaselineShapes::pin(sense), ParityBaselineShapes::pin(th)],
            writes: vec![Statement { pin: out, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `GatedISink`, built once and shared by every instance.
    pub fn sink_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let g = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_MS, ..PinSpec::discrete("g", role::PARAM, &F64) });
        let ctrl = s.pin(PinSpec::discrete("ctrl", role::PORT, &BIT));
        let node = s.pin(PinSpec::continuous("node", role::PORT));
        let t0 = s.term(ParityBaselineShapes::flow("i", node, GROUND));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![ParityBaselineShapes::pin(ctrl), ParityBaselineShapes::pin(node), ParityBaselineShapes::pin(g)],
            writes: vec![ParityBaselineShapes::term(t0)],
            partials: vec![ParityBaselineShapes::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `DigitalSource`, built once and shared by every instance.
    pub fn src_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &HIGH, ..PinSpec::discrete("out", role::PORT, &BIT) });
        s.freeze()
    }
}
