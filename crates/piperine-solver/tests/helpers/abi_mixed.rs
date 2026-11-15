//! Mixed-signal fixture devices on the `Device` trait for the protocol tests
//! (`solver-abi-integration` Phase 3): the comparator, D flip-flop and DAC of
//! abi.md §11.4 to §11.6, an inverter, a `SETTLE` counter, a probe of the
//! session globals, and the timer, recorder, stub and sampler of
//! `solver-abi-digital`'s fixtures. Each reaction bumps a per-thread counter
//! (`reactions`), so a test counts the calls its own run made.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW, REAL};
use piperine_abi::raw::{
    Cell, DEFAULT, Field, GROUND, Layout, List, Module, Partial, Shape, Statement, Str, ambient, cell, global, leaf, op, part, pin_flag,
    reaction_flag, role, statement_flag, watch,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "abi_mixed.rs";

// ─── Counting reactions ─────────────────────────────────────────────────────

thread_local! {
    static REACTIONS: RefCell<HashMap<&'static str, u32>> = RefCell::new(HashMap::new());
}

fn bump(device: &'static str) {
    REACTIONS.with(|c| *c.borrow_mut().entry(device).or_default() += 1);
}

/// The reactions this thread ran on devices called `device`.
pub fn reactions(device: &str) -> u32 {
    REACTIONS.with(|c| c.borrow().get(device).copied().unwrap_or_default())
}

// ─── Shape helpers ──────────────────────────────────────────────────────────

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}
fn pin_part(index: u32, part: u32) -> Cell {
    Cell { kind: cell::PIN, index, part }
}
fn term(index: u32) -> Cell {
    Cell { kind: cell::TERM, index, part: part::VALUE }
}
fn watch_cell(index: u32) -> Cell {
    Cell { kind: cell::WATCH, index, part: part::VALUE }
}
fn partial(write: u32, read: u32) -> Partial {
    Partial { write, to: 0, read, at: 0 }
}
fn statement(pin: u32, len: u32, flags: u32) -> Statement {
    Statement { pin, offset: 0, len, flags, min_after: 0.0 }
}
fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
    TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
}
fn watch_spec(name: &'static str, kind: u32, event: u32) -> WatchSpec<'static> {
    WatchSpec { name, origin: ORIGIN, kind, layout: &F64, event, ttol: DEFAULT, etol: DEFAULT }
}
/// `v` bytes of a `BIT`.
fn bit(value: bool) -> [u8; 2] {
    [value as u8, 1]
}
/// A `REAL`: value and known byte.
pub fn real(v: f64) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&v.to_le_bytes());
    b[8] = 1;
    b
}

fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
    slot.get_or_init(build)
}

fn module_of<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
    slot.get_or_init(|| module::<D>(name))
}

// ─── Comparator (abi.md §11.5) ──────────────────────────────────────────────

/// pins: inp=0, inn=1 (PORT continuous); out=2 (PORT BIT, init LOW); up=3, down=4 (PRIVATE event)
/// watches: W0 CROSS -> up, W1 CROSS -> down
/// fn 0: reads [inp, inn] writes [W0, W1] partials (0<-0) (0<-1) (1<-0) (1<-1)
/// reaction 0: on [up] writes [S0 {out}]; out <- HIGH.   reaction 1: on [down] writes [S0 {out}]; out <- LOW
pub struct Comparator;
static COMPARATOR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COMPARATOR: OnceLock<Module> = OnceLock::new();

fn comparator_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let inp = s.pin(PinSpec::continuous("inp", role::PORT));
    let inn = s.pin(PinSpec::continuous("inn", role::PORT));
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    let up = s.pin(PinSpec::event("up", role::PRIVATE));
    let down = s.pin(PinSpec::event("down", role::PRIVATE));
    let w0 = s.watch(watch_spec("rise", watch::CROSS, up));
    let w1 = s.watch(watch_spec("fall", watch::CROSS, down));
    s.function(FunctionSpec {
        name: "e",
        origin: ORIGIN,
        reads: vec![pin(inp), pin(inn)],
        writes: vec![watch_cell(w0), watch_cell(w1)],
        partials: vec![partial(0, 0), partial(0, 1), partial(1, 0), partial(1, 1)],
        ..FunctionSpec::default()
    });
    for (name, on) in [("up", up), ("down", down)] {
        s.reaction(ReactionSpec { name, origin: ORIGIN, on: vec![on], writes: vec![statement(out, 2, 0)], ..ReactionSpec::default() });
    }
    s.freeze()
}

impl Device for Comparator {
    fn prototype() -> &'static Shape<'static> {
        shared(&COMPARATOR_SHAPE, comparator_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Comparator, Arc::clone(shared(&COMPARATOR_SHAPE, comparator_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let e = cx.f64(0) - cx.f64(1);
        cx.set(0, e);
        cx.set(1, -e);
        cx.jac().copy_from_slice(&[1.0, -1.0, -1.0, 1.0]);
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("comparator");
        cx.commit(0, if k == 0 { &HIGH } else { &LOW }, 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn comparator() -> &'static Module {
    module_of::<Comparator>(&COMPARATOR, "Comparator")
}

// ─── Dff (abi.md §11.4) ─────────────────────────────────────────────────────

/// pins: clk=0, d=1 (PORT BIT); q=2 (PORT BIT, init LOW); rise=3 (PRIVATE event)
/// fn 0 "rise": reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED] writes [rise]
/// reaction 0: on [rise] reads [d] writes [S0 {q}]; q <- d, no delay
pub struct Dff;
static DFF_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DFF: OnceLock<Module> = OnceLock::new();

fn dff_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
    let d = s.pin(PinSpec::discrete("d", role::PORT, &BIT));
    let q = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("q", role::PORT, &BIT) });
    let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
    s.function(FunctionSpec {
        name: "rise",
        origin: ORIGIN,
        reads: vec![pin(clk), pin_part(clk, part::PREVIOUS), pin_part(clk, part::CHANGED)],
        writes: vec![pin_part(rise, part::CHANGED)],
        ..FunctionSpec::default()
    });
    s.reaction(ReactionSpec {
        name: "sample",
        origin: ORIGIN,
        on: vec![rise],
        reads: vec![pin(d)],
        writes: vec![statement(q, 2, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Dff {
    fn prototype() -> &'static Shape<'static> {
        shared(&DFF_SHAPE, dff_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Dff, Arc::clone(shared(&DFF_SHAPE, dff_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let rising = cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW;
        cx.set(0, rising as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("dff");
        cx.commit(0, cx.bytes(0), 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn dff() -> &'static Module {
    module_of::<Dff>(&DFF, "Dff")
}

// ─── Inverter ───────────────────────────────────────────────────────────────

/// pins: a=0, y=1 (PORT BIT).  fn 0 "inv": reads [a] writes [y]; y is unknown while a is.
pub struct Inverter;
static INVERTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static INVERTER: OnceLock<Module> = OnceLock::new();

fn inverter_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
    let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
    s.function(FunctionSpec { name: "inv", origin: ORIGIN, reads: vec![pin(a)], writes: vec![pin(y)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for Inverter {
    fn prototype() -> &'static Shape<'static> {
        shared(&INVERTER_SHAPE, inverter_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Inverter, Arc::clone(shared(&INVERTER_SHAPE, inverter_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let a = cx.bytes(0);
        let y = if a[1] != 0 { bit(a[0] == 0) } else { [0, 0] };
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn inverter() -> &'static Module {
    module_of::<Inverter>(&INVERTER, "Inverter")
}

// ─── Dac (abi.md §11.6, a one-bit code) ─────────────────────────────────────

/// abi.md §11.6 with `code` a `BIT` (k = its value, 0 or 1), so a flip-flop or a
/// timer drives it directly.
///
/// pins: vref=0 (PARAM F64, default 1.0); code=1 (PORT BIT); out=2 (PORT continuous);
///       f=3 (PRIVATE continuous: the branch flow)
/// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
/// fn 0: reads [code, out, f, vref] writes [T0, T1] partials (0<-f) (1<-out) (1<-vref)
pub struct Dac;
static DAC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DAC: OnceLock<Module> = OnceLock::new();
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();

fn dac_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let vref = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("vref", role::PARAM, &F64) });
    let code = s.pin(PinSpec::discrete("code", role::PORT, &BIT));
    let out = s.pin(PinSpec::continuous("out", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(flow("i", out, GROUND));
    let t1 = s.term(flow("eq", f, GROUND));
    s.function(FunctionSpec {
        name: "dac",
        origin: ORIGIN,
        reads: vec![pin(code), pin(out), pin(f), pin(vref)],
        writes: vec![term(t0), term(t1)],
        partials: vec![partial(0, 2), partial(1, 1), partial(1, 3)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Dac {
    fn prototype() -> &'static Shape<'static> {
        shared(&DAC_SHAPE, dac_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Dac, Arc::clone(shared(&DAC_SHAPE, dac_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let k = cx.bytes(0)[0] as f64;
        cx.set(0, cx.f64(2));
        cx.set(1, cx.f64(1) - k * cx.f64(3));
        cx.jac().copy_from_slice(&[1.0, 1.0, -k]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn dac() -> &'static Module {
    module_of::<Dac>(&DAC, "Dac")
}

// ─── SettleCount ────────────────────────────────────────────────────────────

/// Counts the `SETTLE` firings it sees (`reactions("settle")`).
/// pins: settle=0 (GLOBAL SETTLE event)   reaction 0: on [settle]
pub struct SettleCount;
static SETTLE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE: OnceLock<Module> = OnceLock::new();

fn settle_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
    s.reaction(ReactionSpec { name: "count", origin: ORIGIN, on: vec![settle], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for SettleCount {
    fn prototype() -> &'static Shape<'static> {
        shared(&SETTLE_SHAPE, settle_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleCount, Arc::clone(shared(&SETTLE_SHAPE, settle_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        bump("settle");
        Ok(Flow::Continue)
    }
}

pub fn settle_count() -> &'static Module {
    module_of::<SettleCount>(&SETTLE, "SettleCount")
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

fn globals_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let temp = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temp", role::GLOBAL, &F64) });
    let res = s.pin(PinSpec { global: global::RESOLUTION, ..PinSpec::discrete("res", role::GLOBAL, &F64) });
    let dc = s.pin(PinSpec { global: global::DC, ..PinSpec::discrete("dc", role::GLOBAL, &BIT) });
    let vdd = s.pin(PinSpec { global: global::NAMED, ..PinSpec::discrete("vdd", role::GLOBAL, &F64) });
    let watches: Vec<Cell> = ["temp", "res", "dc", "vdd"]
        .into_iter()
        .map(|name| watch_cell(s.watch(watch_spec(name, watch::VAR, DEFAULT))))
        .collect();
    s.function(FunctionSpec {
        name: "show",
        origin: ORIGIN,
        reads: vec![pin(temp), pin(res), pin(dc), pin(vdd)],
        writes: watches,
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Globals {
    fn prototype() -> &'static Shape<'static> {
        shared(&GLOBALS_SHAPE, globals_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Globals, Arc::clone(shared(&GLOBALS_SHAPE, globals_shape))))
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
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn globals() -> &'static Module {
    module_of::<Globals>(&GLOBALS, "Globals")
}

// ─── Timer (solver-abi-digital) ─────────────────────────────────────────────

/// A clock: `out` toggles every `half` seconds, each edge committed ahead of time.
/// pins: half=0 (PARAM F64, default 10 ns); out=1 (PORT BIT, init LOW); tick=2 (PRIVATE event);
///       start=3 (GLOBAL START event)
/// reaction 0 "run": on [start, tick] reads [out, half] writes [S0 {out}, S1 {tick, REPLACE}]
pub struct Timer;
static TIMER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TIMER: OnceLock<Module> = OnceLock::new();
const TEN_NS: [u8; 8] = 1e-8f64.to_le_bytes();

fn timer_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let half = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &TEN_NS, ..PinSpec::discrete("half", role::PARAM, &F64) });
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    let tick = s.pin(PinSpec::event("tick", role::PRIVATE));
    let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
    s.reaction(ReactionSpec {
        name: "run",
        origin: ORIGIN,
        on: vec![start, tick],
        reads: vec![pin(out), pin(half)],
        writes: vec![statement(out, 2, 0), statement(tick, 0, statement_flag::REPLACE)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Timer {
    fn prototype() -> &'static Shape<'static> {
        shared(&TIMER_SHAPE, timer_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Timer, Arc::clone(shared(&TIMER_SHAPE, timer_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("timer");
        let out = cx.bytes(0);
        let half = cx.f64(1);
        cx.commit(0, &bit(out[0] == 0), half)?;
        cx.commit(1, &[], half)?;
        Ok(Flow::Continue)
    }
}

pub fn timer() -> &'static Module {
    module_of::<Timer>(&TIMER, "Timer")
}

// ─── Recorder (solver-abi-digital) ──────────────────────────────────────────

/// Emits eight records through one statement when `go` fires.
/// pins: go=0 (PORT event); log=1 (PORT BYTE, RECORD)
/// reaction 0: on [go] writes [S0 {log}]; commits `[i, known]` for i = 1 to 8, no delay
pub struct Recorder;
static RECORDER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RECORDER: OnceLock<Module> = OnceLock::new();

fn recorder_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let log = s.pin(PinSpec { flags: pin_flag::RECORD, ..PinSpec::discrete("log", role::PORT, &BYTE) });
    s.reaction(ReactionSpec { name: "emit", origin: ORIGIN, on: vec![go], writes: vec![statement(log, 2, 0)], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for Recorder {
    fn prototype() -> &'static Shape<'static> {
        shared(&RECORDER_SHAPE, recorder_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Recorder, Arc::clone(shared(&RECORDER_SHAPE, recorder_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("recorder");
        for i in 1..=8u8 {
            cx.commit(0, &[i, 1], 0.0)?;
        }
        Ok(Flow::Continue)
    }
}

pub fn recorder() -> &'static Module {
    module_of::<Recorder>(&RECORDER, "Recorder")
}

// ─── Sampler (solver-abi-digital) ───────────────────────────────────────────

/// A reaction that reads a continuous pin: its sample becomes a `REAL` when `go` fires.
/// pins: v=0 (PORT continuous); go=1 (PORT event); s=2 (PORT REAL)
/// reaction 0: on [go] reads [v] writes [S0 {s}]; s <- the sample, no delay
pub struct Sampler;
static SAMPLER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SAMPLER: OnceLock<Module> = OnceLock::new();

fn sampler_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let v = s.pin(PinSpec::continuous("v", role::PORT));
    let go = s.pin(PinSpec::event("go", role::PORT));
    let out = s.pin(PinSpec::discrete("s", role::PORT, &REAL));
    s.reaction(ReactionSpec {
        name: "sample",
        origin: ORIGIN,
        on: vec![go],
        reads: vec![pin(v)],
        writes: vec![statement(out, 16, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Sampler {
    fn prototype() -> &'static Shape<'static> {
        shared(&SAMPLER_SHAPE, sampler_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Sampler, Arc::clone(shared(&SAMPLER_SHAPE, sampler_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("sampler");
        cx.commit(0, &real(cx.f64(0)), 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn sampler() -> &'static Module {
    module_of::<Sampler>(&SAMPLER, "Sampler")
}

// ─── Stub (solver-abi-digital) ──────────────────────────────────────────────

/// An emulator stub whose reaction is `IRREVERSIBLE` (abi.md §5 R6).
/// pins: go=0 (PORT event); out=1 (PORT BIT, init LOW)
/// reaction 0 "run" (IRREVERSIBLE): on [go] writes [S0 {out}]; out <- HIGH, no delay
pub struct Stub;
static STUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STUB: OnceLock<Module> = OnceLock::new();

fn stub_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    s.reaction(ReactionSpec {
        name: "run",
        origin: ORIGIN,
        flags: reaction_flag::IRREVERSIBLE,
        on: vec![go],
        writes: vec![statement(out, 2, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Stub {
    fn prototype() -> &'static Shape<'static> {
        shared(&STUB_SHAPE, stub_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Stub, Arc::clone(shared(&STUB_SHAPE, stub_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("stub");
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn stub() -> &'static Module {
    module_of::<Stub>(&STUB, "Stub")
}

// ─── Counter (solver-abi-digital) ───────────────────────────────────────────

static COUNT_FIELDS: [Field; 1] =
    [Field { name: Str::from_static("v"), leaf: leaf::U32, offset: 0, count: 1, flags: 0, known: 0 }];
/// One `U32` with no known byte.
pub static COUNT: Layout = Layout { size: 4, align: 4, fields: List::from_slice(&COUNT_FIELDS) };

/// The `u32` held in a `COUNT` pin's bytes.
pub fn count_of(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[..4].try_into().expect("a COUNT is four bytes"))
}

/// Counts rising edges in a `QUIET` state pin only its own reaction reads.
/// pins: clk=0 (PORT BIT); n=1 (PORT COUNT, QUIET); inc=2 (PRIVATE event)
/// fn 0 "rise": as the flip-flop's, writes [inc]
/// reaction 0: on [inc] reads [n] writes [S0 {n}]; n <- n + 1, no delay
pub struct Counter;
static COUNTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COUNTER: OnceLock<Module> = OnceLock::new();

fn counter_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
    let n = s.pin(PinSpec { flags: pin_flag::QUIET, ..PinSpec::discrete("n", role::PORT, &COUNT) });
    let inc = s.pin(PinSpec::event("inc", role::PRIVATE));
    s.function(FunctionSpec {
        name: "rise",
        origin: ORIGIN,
        reads: vec![pin(clk), pin_part(clk, part::PREVIOUS), pin_part(clk, part::CHANGED)],
        writes: vec![pin_part(inc, part::CHANGED)],
        ..FunctionSpec::default()
    });
    s.reaction(ReactionSpec {
        name: "count",
        origin: ORIGIN,
        on: vec![inc],
        reads: vec![pin(n)],
        writes: vec![statement(n, 4, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Counter {
    fn prototype() -> &'static Shape<'static> {
        shared(&COUNTER_SHAPE, counter_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Counter, Arc::clone(shared(&COUNTER_SHAPE, counter_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let rising = cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW;
        cx.set(0, rising as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("counter");
        let n = count_of(cx.bytes(0)) + 1;
        cx.commit(0, &n.to_le_bytes(), 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn counter() -> &'static Module {
    module_of::<Counter>(&COUNTER, "Counter")
}

// ─── SettleTime ─────────────────────────────────────────────────────────────

thread_local! {
    static SETTLES: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

/// The times of the `SETTLE` firings this thread's `SettleTime` devices saw.
pub fn settle_times() -> Vec<f64> {
    SETTLES.with(|s| s.borrow().clone())
}

/// Writes down, per thread, the time of each `SETTLE` it sees (`settle_times`).
/// pins: settle=0 (GLOBAL SETTLE event)
/// reaction 0: on [settle] reads [TIME]
pub struct SettleTime;
static SETTLE_TIME_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_TIME: OnceLock<Module> = OnceLock::new();

fn settle_time_shape() -> ShapeRef {
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

impl Device for SettleTime {
    fn prototype() -> &'static Shape<'static> {
        shared(&SETTLE_TIME_SHAPE, settle_time_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleTime, Arc::clone(shared(&SETTLE_TIME_SHAPE, settle_time_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("settle_time");
        let t = cx.f64(0);
        SETTLES.with(|s| s.borrow_mut().push(t));
        Ok(Flow::Continue)
    }
}

pub fn settle_time() -> &'static Module {
    module_of::<SettleTime>(&SETTLE_TIME, "SettleTime")
}
