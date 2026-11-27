//! Mixed-signal fixture devices on the `Device` trait for the protocol tests
//! (`solver-abi-integration` Phase 3): the comparator, D flip-flop and DAC of
//! abi.md §11.4 to §11.6, an inverter, a `SETTLE` counter, a probe of the
//! session globals, and the timer, recorder, stub and sampler of
//! `solver-abi-digital`'s fixtures. Each reaction bumps a per-thread counter
//! (`reactions`), so a test counts the calls its own run made.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, ambient, cell, global, op, part, pin_flag, role, statement_flag, watch,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

use crate::digital::CallCounts;

const ORIGIN: &str = "abi_mixed.rs";

// ─── Comparator (abi.md §11.5) ──────────────────────────────────────────────

/// pins: inp=0, inn=1 (PORT continuous); out=2 (PORT BIT, init LOW); up=3, down=4 (PRIVATE event)
/// watches: W0 CROSS -> up, W1 CROSS -> down
/// fn 0: reads [inp, inn] writes [W0, W1] partials (0<-0) (0<-1) (1<-0) (1<-1)
/// reaction 0: on [up] writes [S0 {out}]; out <- HIGH.   reaction 1: on [down] writes [S0 {out}]; out <- LOW
pub struct Comparator;
static COMPARATOR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COMPARATOR: OnceLock<Module> = OnceLock::new();

impl Comparator {
    /// The shape of `Comparator`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let inp = s.pin(PinSpec::continuous("inp", role::PORT));
        let inn = s.pin(PinSpec::continuous("inn", role::PORT));
        let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let down = s.pin(PinSpec::event("down", role::PRIVATE));
        let w0 = s.watch(MixedParts::watch_spec("up", watch::CROSS, up));
        let w1 = s.watch(MixedParts::watch_spec("down", watch::CROSS, down));
        s.function(FunctionSpec {
            name: "e",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(inp), MixedParts::pin(inn)],
            writes: vec![MixedParts::watch_cell(w0), MixedParts::watch_cell(w1)],
            partials: vec![MixedParts::partial(0, 0), MixedParts::partial(0, 1), MixedParts::partial(1, 0), MixedParts::partial(1, 1)],
            ..FunctionSpec::default()
        });
        for (name, on) in [("up", up), ("down", down)] {
            s.reaction(ReactionSpec { name, origin: ORIGIN, on: vec![on], writes: vec![MixedParts::statement(out, 1, 0)], ..ReactionSpec::default() });
        }
        s.freeze()
    }

    /// The module of `Comparator`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<Comparator>(&COMPARATOR, "Comparator")
    }
}

impl Device for Comparator {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&COMPARATOR_SHAPE, Comparator::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Comparator, Arc::clone(MixedParts::shared(&COMPARATOR_SHAPE, Comparator::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let e = cx.f64(0) - cx.f64(1);
        cx.set(0, e);
        cx.set(1, -e);
        cx.jac().copy_from_slice(&[1.0, -1.0, -1.0, 1.0]);
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("comparator");
        cx.commit(0, if k == 0 { &HIGH } else { &LOW }, 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Dac (abi.md §11.6, a one-bit code) ─────────────────────────────────────

/// abi.md §11.6 with `code` a `BIT` (k = its value, 0 or 1), so a flip-flop or a
/// timer drives it directly.
///
/// pins: vref=0 (PARAM F64, default 1.0); code=1 (PORT BIT); out=2 (PORT continuous);
///       f=3 (PRIVATE continuous: the branch flow)
/// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
/// fn 0: reads [code, out, f, vref] writes [T0, T1] partials (0<-f) (1<-out) (1<-vref)
pub struct BitDigitalToAnalog;
static DAC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DAC: OnceLock<Module> = OnceLock::new();
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();

impl BitDigitalToAnalog {
    /// The shape of `BitDigitalToAnalog`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let vref = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("vref", role::PARAM, &F64) });
        let code = s.pin(PinSpec::discrete("code", role::PORT, &BIT));
        let out = s.pin(PinSpec::continuous("out", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(MixedParts::flow("i", out, GROUND));
        let t1 = s.term(MixedParts::flow("eq", f, GROUND));
        s.function(FunctionSpec {
            name: "dac",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(code), MixedParts::pin(out), MixedParts::pin(f), MixedParts::pin(vref)],
            writes: vec![MixedParts::term(t0), MixedParts::term(t1)],
            partials: vec![MixedParts::partial(0, 2), MixedParts::partial(1, 1), MixedParts::partial(1, 3)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `BitDigitalToAnalog`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<BitDigitalToAnalog>(&DAC, "Dac")
    }
}

impl Device for BitDigitalToAnalog {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&DAC_SHAPE, BitDigitalToAnalog::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((BitDigitalToAnalog, Arc::clone(MixedParts::shared(&DAC_SHAPE, BitDigitalToAnalog::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let k = cx.bytes(0)[0] as f64;
        cx.set(0, cx.f64(2));
        cx.set(1, cx.f64(1) - k * cx.f64(3));
        cx.jac().copy_from_slice(&[1.0, 1.0, -k]);
        Ok(())
    }
}

// ─── SettleCount ────────────────────────────────────────────────────────────

/// Counts the `SETTLE` firings it sees (`reactions("settle")`).
/// pins: settle=0 (GLOBAL SETTLE event)   reaction 0: on [settle]
pub struct SettleCount;
static SETTLE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE: OnceLock<Module> = OnceLock::new();

impl SettleCount {
    /// The shape of `SettleCount`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        s.reaction(ReactionSpec { name: "count", origin: ORIGIN, on: vec![settle], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The module of `SettleCount`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<SettleCount>(&SETTLE, "SettleCount")
    }
}

impl Device for SettleCount {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&SETTLE_SHAPE, SettleCount::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleCount, Arc::clone(MixedParts::shared(&SETTLE_SHAPE, SettleCount::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("settle");
        Ok(Flow::Continue)
    }
}

// ─── Globals ────────────────────────────────────────────────────────────────

/// Reads the session globals in an analog function and shows them as `VAR` watches.
/// pins: temp=0 (GLOBAL TEMPERATURE F64); res=1 (GLOBAL RESOLUTION F64); dc=2 (GLOBAL DC BIT);
///       vdd=3 (GLOBAL NAMED F64)
/// watches: VAR temp, res, dc (1.0 when the flag is HIGH), vdd
/// fn 0: reads [temp, res, dc, vdd] writes [W0..W3]
pub struct Globals;
static GLOBALS_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GLOBALS: OnceLock<Module> = OnceLock::new();

impl Globals {
    /// The shape of `Globals`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let temp = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temp", role::GLOBAL, &F64) });
        let res = s.pin(PinSpec { global: global::RESOLUTION, ..PinSpec::discrete("res", role::GLOBAL, &F64) });
        let dc = s.pin(PinSpec { global: global::DC, ..PinSpec::discrete("dc", role::GLOBAL, &BIT) });
        let vdd = s.pin(PinSpec { global: global::NAMED, ..PinSpec::discrete("vdd", role::GLOBAL, &F64) });
        let watches: Vec<Cell> = ["temp", "res", "dc", "vdd"]
            .into_iter()
            .map(|name| MixedParts::watch_cell(s.watch(MixedParts::watch_spec(name, watch::VAR, DEFAULT))))
            .collect();
        s.function(FunctionSpec {
            name: "show",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(temp), MixedParts::pin(res), MixedParts::pin(dc), MixedParts::pin(vdd)],
            writes: watches,
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Globals`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<Globals>(&GLOBALS, "Globals")
    }
}

impl Device for Globals {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&GLOBALS_SHAPE, Globals::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Globals, Arc::clone(MixedParts::shared(&GLOBALS_SHAPE, Globals::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (temp, res, vdd) = (cx.f64(0), cx.f64(1), cx.f64(3));
        let dc = if cx.bytes(2) == HIGH { 1.0 } else { 0.0 };
        cx.set(0, temp);
        cx.set(1, res);
        cx.set(2, dc);
        cx.set(3, vdd);
        Ok(())
    }
}

// ─── SettleTime ─────────────────────────────────────────────────────────────

thread_local! {
    static SETTLES: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

/// Writes down, per thread, the time of each `SETTLE` it sees (`settle_times`).
/// pins: settle=0 (GLOBAL SETTLE event)
/// reaction 0: on [settle] reads [TIME]
pub struct SettleTime;
static SETTLE_TIME_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_TIME: OnceLock<Module> = OnceLock::new();

impl SettleTime {
    /// The times of the `SETTLE` firings this thread's `SettleTime` devices saw.
    pub fn times() -> Vec<f64> {
        SETTLES.with(|s| s.borrow().clone())
    }

    /// The shape of `SettleTime`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        s.reaction(ReactionSpec {
            name: "stamp",
            origin: ORIGIN,
            on: vec![settle],
            reads: vec![Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of `SettleTime`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<SettleTime>(&SETTLE_TIME, "SettleTime")
    }
}

impl Device for SettleTime {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&SETTLE_TIME_SHAPE, SettleTime::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleTime, Arc::clone(MixedParts::shared(&SETTLE_TIME_SHAPE, SettleTime::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("settle_time");
        let t = cx.f64(0);
        SETTLES.with(|s| s.borrow_mut().push(t));
        Ok(Flow::Continue)
    }
}

// ─── Delay ──────────────────────────────────────────────────────────────────

const ONE_US: [u8; 8] = 1e-6f64.to_le_bytes();

/// Fires `out` once, `at` seconds after `START`.
/// pins: at=0 (PARAM F64, default 1 µs); start=1 (GLOBAL START event); out=2 (PORT event)
/// reaction 0: on [start] reads [at] writes [S0 {out}]; fires `out` after `at`
pub struct Delay;
static DELAY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DELAY: OnceLock<Module> = OnceLock::new();

impl Delay {
    /// The shape of `Delay`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let at = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_US, ..PinSpec::discrete("at", role::PARAM, &F64) });
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let out = s.pin(PinSpec::event("out", role::PORT));
        s.reaction(ReactionSpec {
            name: "arm",
            origin: ORIGIN,
            on: vec![start],
            reads: vec![MixedParts::pin(at)],
            writes: vec![MixedParts::statement(out, 0, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Delay`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<Delay>(&DELAY, "Delay")
    }
}

impl Device for Delay {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&DELAY_SHAPE, Delay::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Delay, Arc::clone(MixedParts::shared(&DELAY_SHAPE, Delay::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("delay");
        let at = cx.f64(0);
        cx.commit(0, &[], at)?;
        Ok(Flow::Continue)
    }
}

// ─── Finisher ───────────────────────────────────────────────────────────────

/// Ends the analysis when `go` fires.
/// pins: go=0 (PORT event)   reaction 0: on [go]; returns FINISH
pub struct Finisher;
static FINISHER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FINISHER: OnceLock<Module> = OnceLock::new();

impl Finisher {
    /// The shape of `Finisher`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let go = s.pin(PinSpec::event("go", role::PORT));
        s.reaction(ReactionSpec { name: "finish", origin: ORIGIN, on: vec![go], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The module of `Finisher`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<Finisher>(&FINISHER, "Finisher")
    }
}

impl Device for Finisher {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&FINISHER_SHAPE, Finisher::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Finisher, Arc::clone(MixedParts::shared(&FINISHER_SHAPE, Finisher::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("finisher");
        Ok(Flow::Finish)
    }
}

// ─── ConductanceTracer ─────────────────────────────────────────────────────────────────

/// A 1 mS conductance to ground that writes a `TRACE` record at every evaluation.
/// pins: p=0 (PORT continuous)   terms: T0 FLOW{p, GROUND}   watches: W0 TRACE (BYTE)
/// fn 0: reads [p] writes [T0, W0] partials (0<-0); W0 <- [1]
pub struct ConductanceTracer;
static TRACER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TRACER: OnceLock<Module> = OnceLock::new();
const TRACER_G: f64 = 1.0e-3;

impl ConductanceTracer {
    /// The shape of `ConductanceTracer`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(MixedParts::flow("i", p, GROUND));
        let w0 = s.watch(WatchSpec { layout: &BYTE, ..MixedParts::watch_spec("seen", watch::TRACE, DEFAULT) });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(p)],
            writes: vec![MixedParts::term(t0), MixedParts::watch_cell(w0)],
            partials: vec![MixedParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `ConductanceTracer`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<ConductanceTracer>(&TRACER, "ConductanceTracer")
    }
}

impl Device for ConductanceTracer {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&TRACER_SHAPE, ConductanceTracer::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ConductanceTracer, Arc::clone(MixedParts::shared(&TRACER_SHAPE, ConductanceTracer::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, TRACER_G * cx.f64(0));
        cx.out_bytes(1).copy_from_slice(&[1]);
        cx.jac()[0] = TRACER_G;
        Ok(())
    }
}

// ─── FlakyG ─────────────────────────────────────────────────────────────────

thread_local! {
    static FAULT_WINDOW: std::cell::Cell<Option<(f64, f64)>> = const { std::cell::Cell::new(None) };
    static FAULTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A 1 mS conductance whose function faults once (a Newton failure) when armed (`arm_fault`).
/// pins: p=0, n=1 (PORT)   terms: T0 FLOW{p, n}
/// fn 0: reads [p, n, TIME] writes [T0] partials (0<-0) (0<-1)
pub struct FlakyConductance;
static FLAKY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAKY: OnceLock<Module> = OnceLock::new();

impl FlakyConductance {
    /// Arms this thread's `FlakyG`s: the first evaluation with a time inside `window` faults.
    pub fn arm(window: (f64, f64)) {
        FAULT_WINDOW.with(|w| w.set(Some(window)));
        FAULTED.with(|f| f.set(false));
    }

    /// Whether an armed `FlakyG` has faulted on this thread.
    pub fn has_faulted() -> bool {
        FAULTED.with(|f| f.get())
    }

    /// The shape of `FlakyConductance`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(MixedParts::flow("i", p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(p), MixedParts::pin(n), Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }],
            writes: vec![MixedParts::term(t0)],
            partials: vec![MixedParts::partial(0, 0), MixedParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `FlakyConductance`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<FlakyConductance>(&FLAKY, "FlakyG")
    }
}

impl Device for FlakyConductance {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&FLAKY_SHAPE, FlakyConductance::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlakyConductance, Arc::clone(MixedParts::shared(&FLAKY_SHAPE, FlakyConductance::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let t = cx.f64(2);
        let inside = FAULT_WINDOW.with(|w| w.get()).is_some_and(|(a, b)| t > a && t < b);
        if inside && !FAULTED.with(|f| f.replace(true)) {
            return Err(Fault::new(7, "once"));
        }
        let g = 1.0e-3;
        cx.set(0, g * cx.f64(0) - g * cx.f64(1));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}

// ─── Capped ─────────────────────────────────────────────────────────────────

const ONE_NS: [u8; 8] = 1e-9f64.to_le_bytes();

/// Bounds every step by its `cap` param through a `CAP` watch.
/// pins: cap=0 (PARAM F64, default 1 ns); p=1 (PORT continuous)
/// watches: W0 CAP;  fn 0: reads [p, cap] writes [W0]; W0 <- cap
pub struct Capped;
static CAPPED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CAPPED: OnceLock<Module> = OnceLock::new();

impl Capped {
    /// The shape of `Capped`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let cap = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_NS, ..PinSpec::discrete("cap", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let w0 = s.watch(MixedParts::watch_spec("cap", watch::CAP, DEFAULT));
        s.function(FunctionSpec {
            name: "cap",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(p), MixedParts::pin(cap)],
            writes: vec![MixedParts::watch_cell(w0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The module of `Capped`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<Capped>(&CAPPED, "Capped")
    }
}

impl Device for Capped {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&CAPPED_SHAPE, Capped::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capped, Arc::clone(MixedParts::shared(&CAPPED_SHAPE, Capped::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let cap = cx.f64(1);
        cx.set(0, cap);
        Ok(())
    }
}

// ─── ClockSrc (tests/helpers/devices.rs) ────────────────────────────────────

const ONE_MS: [u8; 8] = 1e-3f64.to_le_bytes();

/// ```verilog
/// module ClockSrc (inout electrical clk_a, inout electrical gnd);
///     parameter real period = 1.0e-3;
///     real ph;
///     analog begin
///         ph = $abstime - period * floor($abstime / period);
///         V(clk_a, gnd) <+ (ph > period * 0.25 && ph < period * 0.75) ? 1.0 : 0.0;
///     end
/// endmodule
/// ```
/// A timer reaction toggles the private phase pin `hi` at `period/4` and every
/// `period/2` after; the analog function drives `V(clk_a, gnd)` from it, so each
/// edge is a landing solved on the level before it (the user's order: at the
/// falling edge's own instant the source still reads 1, `ClockSrc::at` 0).
///
/// pins: period=0 (PARAM F64, default 1 ms); clk_a=1, gnd=2 (PORT); f=3 (PRIVATE continuous);
///       hi=4 (PRIVATE BIT, init LOW); tick=5 (PRIVATE event); start=6 (GLOBAL START)
/// terms: T0 FLOW{clk_a, gnd} "i", T1 FLOW{f, GROUND} "eq"
/// fn 0: reads [clk_a, gnd, f, hi] writes [T0, T1] partials (0<-f) (1<-clk_a) (1<-gnd)
/// reaction 0 "start": on [start] reads [period] writes [S0 {tick, REPLACE}]; tick after period/4
/// reaction 1 "edge": on [tick] reads [hi, period] writes [S0 {hi}, S1 {tick, REPLACE}];
///   hi <- not hi, no delay; tick after period/2
pub struct ClockSource;
static CLOCK_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CLOCK: OnceLock<Module> = OnceLock::new();

impl ClockSource {
    /// The shape of `ClockSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let period = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_MS, ..PinSpec::discrete("period", role::PARAM, &F64) });
        let a = s.pin(PinSpec::continuous("clk_a", role::PORT));
        let g = s.pin(PinSpec::continuous("gnd", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let hi = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("hi", role::PRIVATE, &BIT) });
        let tick = s.pin(PinSpec::event("tick", role::PRIVATE));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let t0 = s.term(MixedParts::flow("i", a, g));
        let t1 = s.term(MixedParts::flow("eq", f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(a), MixedParts::pin(g), MixedParts::pin(f), MixedParts::pin(hi)],
            writes: vec![MixedParts::term(t0), MixedParts::term(t1)],
            partials: vec![MixedParts::partial(0, 2), MixedParts::partial(1, 0), MixedParts::partial(1, 1)],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "start",
            origin: ORIGIN,
            on: vec![start],
            reads: vec![MixedParts::pin(period)],
            writes: vec![MixedParts::statement(tick, 0, statement_flag::REPLACE)],
            ..ReactionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "edge",
            origin: ORIGIN,
            on: vec![tick],
            reads: vec![MixedParts::pin(hi), MixedParts::pin(period)],
            writes: vec![MixedParts::statement(hi, 1, 0), MixedParts::statement(tick, 0, statement_flag::REPLACE)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of `ClockSource`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<ClockSource>(&CLOCK, "ClockSrc")
    }
}

impl Device for ClockSource {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&CLOCK_SHAPE, ClockSource::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ClockSource, Arc::clone(MixedParts::shared(&CLOCK_SHAPE, ClockSource::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = if cx.bytes(3) == HIGH { 1.0 } else { 0.0 };
        cx.set(0, cx.f64(2));
        cx.set(1, cx.f64(0) - cx.f64(1) - level);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("clock");
        if k == 0 {
            cx.commit(0, &[], 0.25 * cx.f64(0))?;
        } else {
            cx.commit(0, &MixedParts::bit(cx.bytes(0) != HIGH), 0.0)?;
            cx.commit(1, &[], 0.5 * cx.f64(1))?;
        }
        Ok(Flow::Continue)
    }
}

// ─── ToggleDff (tests/helpers/devices.rs) ───────────────────────────────────

/// ```verilog
/// module ToggleDff (input logic clk, output logic q);
///     integer st;
///     always begin
///         q <= (st % 2) > 0;
///         @(posedge clk) st = st + 1;
///     end
/// endmodule
/// ```
/// pins: clk=0 (PORT BIT); q=1 (PORT BIT, init LOW); rise=2 (PRIVATE event)
/// fn 0 "rise": as the flip-flop's, writes [rise]
/// reaction 0: on [rise] reads [q] writes [S0 {q}]; q <- not q, no delay
pub struct ToggleFlipFlop;
static TOGGLE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TOGGLE: OnceLock<Module> = OnceLock::new();

impl ToggleFlipFlop {
    /// The shape of `ToggleFlipFlop`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
        let q = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("q", role::PORT, &BIT) });
        let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
        s.function(FunctionSpec {
            name: "rise",
            origin: ORIGIN,
            reads: vec![MixedParts::pin(clk), MixedParts::pin_part(clk, part::PREVIOUS), MixedParts::pin_part(clk, part::CHANGED)],
            writes: vec![MixedParts::pin_part(rise, part::CHANGED)],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "toggle",
            origin: ORIGIN,
            on: vec![rise],
            reads: vec![MixedParts::pin(q)],
            writes: vec![MixedParts::statement(q, 1, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of `ToggleFlipFlop`, built once from its shape.
    pub fn module() -> &'static Module {
        MixedParts::module_of::<ToggleFlipFlop>(&TOGGLE, "ToggleDff")
    }
}

impl Device for ToggleFlipFlop {
    fn prototype() -> &'static Shape<'static> {
        MixedParts::shared(&TOGGLE_SHAPE, ToggleFlipFlop::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ToggleFlipFlop, Arc::clone(MixedParts::shared(&TOGGLE_SHAPE, ToggleFlipFlop::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let rising = cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW;
        cx.set(0, rising as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        ReactionCounts::bump("toggle");
        cx.commit(0, &MixedParts::bit(cx.bytes(0) != HIGH), 0.0)?;
        Ok(Flow::Continue)
    }
}

/// The per-thread count of the reactions each fixture device ran: the react half of `digital::CallCounts`.
pub struct ReactionCounts;

impl ReactionCounts {
    /// Counts one reaction of the device called `device`.
    fn bump(device: &'static str) {
        CallCounts::bump(device, true);
    }

    /// The reactions this thread ran on devices called `device`.
    pub fn of(device: &str) -> u32 {
        CallCounts::of(device).1
    }
}

/// The cells, bytes and shared shapes every mixed-signal fixture device builds its shape from.
pub struct MixedParts;

impl MixedParts {
    // ─── Shape helpers ──────────────────────────────────────────────────────────

    /// The cell of the value of pin `index`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of part `part` (`PREVIOUS`, `CHANGED`, ...) of pin `index`.
    fn pin_part(index: u32, part: u32) -> Cell {
        Cell { kind: cell::PIN, index, part }
    }

    /// The cell of the value of term `index`.
    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The cell of the value of watch `index`.
    fn watch_cell(index: u32) -> Cell {
        Cell { kind: cell::WATCH, index, part: part::VALUE }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The statement that writes `len` bytes of pin `pin` with `flags` and no minimum delay.
    fn statement(pin: u32, len: u32, flags: u32) -> Statement {
        Statement { pin, offset: 0, len, flags, min_after: 0.0 }
    }

    /// The term spec named `name` of operator `operator`, between pins `plus` and `minus`.
    fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The `F64` watch named `name` of kind `kind`, firing the event pin `event`.
    fn watch_spec(name: &'static str, kind: u32, event: u32) -> WatchSpec<'static> {
        WatchSpec { name, origin: ORIGIN, kind, layout: &F64, event, ttol: DEFAULT, etol: DEFAULT }
    }

    /// `v` bytes of a `BIT`.
    fn bit(value: bool) -> [u8; 1] {
        [value as u8]
    }

    /// An `F64` carrier: its value bytes.
    pub fn real(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// The shape `build` makes, built the first time and kept in `slot` so every instance shares it.
    fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// The module of the device type `Kind` named `name`, built the first time and kept in `slot`.
    fn module_of<Kind: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
        slot.get_or_init(|| module::<Kind>(name))
    }

    /// The `u32` held in a `COUNT` pin's bytes.
    pub fn count_of(bytes: &[u8]) -> u32 {
        u32::from_le_bytes(bytes[..4].try_into().expect("a COUNT is four bytes"))
    }
}
