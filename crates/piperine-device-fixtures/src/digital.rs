//! Hand-written devices for the ABI engine's tests (feature `solver-abi-digital`, SAD-06 and SAD-08;
//! abi.md §11.4 and §11.7).
//!
//! Every device is a plain `impl Device` over a shape built once. Pin names and cells are listed above
//! each device. Each `eval` and `react` bumps a per-thread call counter (`CallCounts`), so a test can
//! count the calls its own engine made: a test runs on its own thread, so counts never mix.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use piperine_abi::caller::Handle;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW};
use piperine_abi::raw::{
    Cell, Created, Eval as RawEval, Field, Instance, Layout, List, Module, Note, React as RawReact, Shape, Statement, Str,
    Value, cell, global, leaf, part, pin_flag, reaction_flag, role, statement_flag, status,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

// ─── Counting calls ─────────────────────────────────────────────────────────

thread_local! {
    static CALLS: RefCell<HashMap<&'static str, (u32, u32)>> = RefCell::new(HashMap::new());
}

static COUNT_FIELDS: [Field; 1] =
    [Field { name: Str::from_static("v"), leaf: leaf::U32, offset: 0, count: 1 }];
/// One `U32`: a field that can move with no "unknown" to fall back to.
pub static COUNT: Layout = Layout { size: 4, align: 4, fields: List::from_slice(&COUNT_FIELDS) };

// ─── Inverter ───────────────────────────────────────────────────────────────

/// pins: a=0, y=1 (PORT BIT).  fn 0 "inv": reads [a] writes [y]; y is the negation of a.
pub struct Inverter;
static INVERTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static INVERTER: OnceLock<Module> = OnceLock::new();

impl Inverter {
    /// The shape of `Inverter`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec { name: "inv", reads: vec![DeviceParts::pin(a)], writes: vec![DeviceParts::pin(y)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Inverter>(&INVERTER, "inverter")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Inverter>(&INVERTER, "inverter")
    }
}

impl Device for Inverter {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&INVERTER_SHAPE, Inverter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Inverter, Arc::clone(DeviceParts::shared(&INVERTER_SHAPE, Inverter::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("inverter", false);
        let a = cx.bytes(0);
        let y = DeviceParts::bit(a[0] == 0);
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
}

// ─── And2 ───────────────────────────────────────────────────────────────────

/// pins: a=0, b=1, y=2 (PORT BIT).  fn 0 "and": reads [a, b] writes [y].
pub struct And2;
static AND2_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static AND2: OnceLock<Module> = OnceLock::new();

impl And2 {
    /// The shape of `And2`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let b = s.pin(PinSpec::discrete("b", role::PORT, &BIT));
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec { name: "and", reads: vec![DeviceParts::pin(a), DeviceParts::pin(b)], writes: vec![DeviceParts::pin(y)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<And2>(&AND2, "and2")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<And2>(&AND2, "and2")
    }
}

impl Device for And2 {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&AND2_SHAPE, And2::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((And2, Arc::clone(DeviceParts::shared(&AND2_SHAPE, And2::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("and2", false);
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let y = DeviceParts::bit(a[0] != 0 && b[0] != 0);
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
}

// ─── Or2 ───────────────────────────────────────────────────────────────────

/// pins: a=0, b=1, y=2 (PORT BIT).  fn 0 "or": reads [a, b] writes [y].
pub struct Or2;
static OR2_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OR2: OnceLock<Module> = OnceLock::new();

impl Or2 {
    /// The shape of `Or2`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let b = s.pin(PinSpec::discrete("b", role::PORT, &BIT));
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec { name: "or", reads: vec![DeviceParts::pin(a), DeviceParts::pin(b)], writes: vec![DeviceParts::pin(y)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Or2>(&OR2, "or2")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Or2>(&OR2, "or2")
    }
}

impl Device for Or2 {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&OR2_SHAPE, Or2::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Or2, Arc::clone(DeviceParts::shared(&OR2_SHAPE, Or2::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("or2", false);
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let y = DeviceParts::bit(a[0] != 0 || b[0] != 0);
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
}

// ─── Dff (abi.md §11.4) ─────────────────────────────────────────────────────

/// pins: clk=0, d=1 (PORT BIT); q=2 (PORT BIT, init LOW); rise=3 (PRIVATE event)
/// fn 0 "rise":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]
/// reaction 0:   on [rise]   reads [d]   writes [S0 {q}]; q <- d, no delay
pub struct DataFlipFlop;
static DFF_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DFF: OnceLock<Module> = OnceLock::new();

impl DataFlipFlop {
    /// The shape of `DataFlipFlop`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
        let d = s.pin(PinSpec::discrete("d", role::PORT, &BIT));
        let q = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("q", role::PORT, &BIT) });
        let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
        s.function(DeviceParts::edge_function(clk, rise));
        s.reaction(ReactionSpec {
            name: "sample",
            on: vec![rise],
            reads: vec![DeviceParts::pin(d)],
            writes: vec![DeviceParts::statement(q, 1, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<DataFlipFlop>(&DFF, "dff")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<DataFlipFlop>(&DFF, "dff")
    }
}

impl Device for DataFlipFlop {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&DFF_SHAPE, DataFlipFlop::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((DataFlipFlop, Arc::clone(DeviceParts::shared(&DFF_SHAPE, DataFlipFlop::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("dff", false);
        cx.set(0, DeviceParts::rising(cx) as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("dff", true);
        cx.commit(0, cx.bytes(0), 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Timer ──────────────────────────────────────────────────────────────────

/// A clock: `out` toggles every `half` seconds, each edge committed ahead of time so it
/// applies at microstep 0 of its instant (the clock source of `docs/spec/part_v_execution.md` §15.1).
///
/// pins: half=0 (PARAM F64, default 10 ns); out=1 (PORT BIT, init LOW); tick=2 (PRIVATE event);
///       start=3 (GLOBAL START event)
/// reaction 0 "run": on [start, tick]  reads [out, half]
///                   writes [S0 {out}, S1 {tick, REPLACE}]
///   commits out <- not out and a `tick` event, both after `half`. A `half` of zero or less
///   re-arms at the next microstep forever: the engine's `E0501` case.
pub struct Timer;
static TIMER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TIMER: OnceLock<Module> = OnceLock::new();
const TEN_NS: [u8; 8] = 1e-8f64.to_le_bytes();

impl Timer {
    /// The shape of `Timer`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let half = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &TEN_NS, ..PinSpec::discrete("half", role::PARAM, &F64) });
        let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        let tick = s.pin(PinSpec::event("tick", role::PRIVATE));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        s.reaction(ReactionSpec {
            name: "run",
            on: vec![start, tick],
            reads: vec![DeviceParts::pin(out), DeviceParts::pin(half)],
            writes: vec![DeviceParts::statement(out, 1, 0), DeviceParts::statement(tick, 0, statement_flag::REPLACE)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Timer>(&TIMER, "timer")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Timer>(&TIMER, "timer")
    }
}

impl Device for Timer {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&TIMER_SHAPE, Timer::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Timer, Arc::clone(DeviceParts::shared(&TIMER_SHAPE, Timer::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("timer", true);
        let out = cx.bytes(0);
        let half = cx.f64(1);
        cx.commit(0, &DeviceParts::bit(out[0] == 0), half)?;
        cx.commit(1, &[], half)?;
        Ok(Flow::Continue)
    }
}

// ─── Counter ────────────────────────────────────────────────────────────────

/// Counts rising edges in a `QUIET` state pin only its own reaction reads.
///
/// pins: clk=0 (PORT BIT); n=1 (PORT COUNT, QUIET); inc=2 (PRIVATE event)
/// fn 0 "rise": as the flip-flop's, writes [inc]
/// reaction 0:  on [inc]  reads [n]  writes [S0 {n}]; n <- n + 1, no delay
pub struct Counter;
static COUNTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COUNTER: OnceLock<Module> = OnceLock::new();

impl Counter {
    /// The shape of `Counter`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
        let n = s.pin(PinSpec { flags: pin_flag::QUIET, ..PinSpec::discrete("n", role::PORT, &COUNT) });
        let inc = s.pin(PinSpec::event("inc", role::PRIVATE));
        s.function(DeviceParts::edge_function(clk, inc));
        s.reaction(ReactionSpec {
            name: "count",
            on: vec![inc],
            reads: vec![DeviceParts::pin(n)],
            writes: vec![DeviceParts::statement(n, 4, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Counter>(&COUNTER, "counter")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Counter>(&COUNTER, "counter")
    }
}

impl Device for Counter {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&COUNTER_SHAPE, Counter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Counter, Arc::clone(DeviceParts::shared(&COUNTER_SHAPE, Counter::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("counter", false);
        cx.set(0, DeviceParts::rising(cx) as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("counter", true);
        let n = DeviceParts::count_of(cx.bytes(0)).wrapping_add(1);
        cx.commit(0, &n.to_le_bytes(), 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Resolver ───────────────────────────────────────────────────────────────

/// A wired-or over two drives (abi.md §9 D1).
///
/// pins: d0=0 (PORT BIT, RELAX); d1=1 (PORT BIT, RELAX, OPTIONAL: unbound it reads as zero); v=2 (PORT BIT)
/// fn 0 "resolve": reads [d0, d1] writes [v]
pub struct Resolver;
static RESOLVER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RESOLVER: OnceLock<Module> = OnceLock::new();

impl Resolver {
    /// The shape of `Resolver`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let d0 = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("d0", role::PORT, &BIT) });
        let d1 = s.pin(PinSpec { flags: pin_flag::RELAX | pin_flag::OPTIONAL, ..PinSpec::discrete("d1", role::PORT, &BIT) });
        let v = s.pin(PinSpec::discrete("v", role::PORT, &BIT));
        s.function(FunctionSpec { name: "resolve", reads: vec![DeviceParts::pin(d0), DeviceParts::pin(d1)], writes: vec![DeviceParts::pin(v)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Resolver>(&RESOLVER, "resolver")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Resolver>(&RESOLVER, "resolver")
    }
}

impl Device for Resolver {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&RESOLVER_SHAPE, Resolver::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resolver, Arc::clone(DeviceParts::shared(&RESOLVER_SHAPE, Resolver::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("resolver", false);
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let value = a[0] != 0 || b[0] != 0;
        cx.out_bytes(0).copy_from_slice(&DeviceParts::bit(value));
        Ok(())
    }
}

// ─── Succ ───────────────────────────────────────────────────────────────────

/// A function whose output always moves: `y = a + 1`. Bound with `a` and `y` on one net it is a
/// relaxed self-loop that never settles (abi.md §9 D1: a net that still moves is a fault).
///
/// pins: a=0 (PORT COUNT); y=1 (PORT COUNT, RELAX)    fn 0 "succ": reads [a] writes [y]
pub struct Successor;
static SUCC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUCC: OnceLock<Module> = OnceLock::new();

impl Successor {
    /// The shape of `Successor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &COUNT));
        let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &COUNT) });
        s.function(FunctionSpec { name: "succ", reads: vec![DeviceParts::pin(a)], writes: vec![DeviceParts::pin(y)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Successor>(&SUCC, "succ")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Successor>(&SUCC, "succ")
    }
}

impl Device for Successor {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&SUCC_SHAPE, Successor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Successor, Arc::clone(DeviceParts::shared(&SUCC_SHAPE, Successor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("succ", false);
        let next = DeviceParts::count_of(cx.bytes(0)).wrapping_add(1);
        cx.set(0, next);
        Ok(())
    }
}

// ─── MultiRecorder ───────────────────────────────────────────────────────────────

/// Emits eight records through one statement when `go` fires.
///
/// pins: go=0 (PORT event); log=1 (PORT BYTE, RECORD)
/// reaction 0: on [go]  writes [S0 {log}]; commits `[i]` for i = 1 to 8, no delay
pub struct MultiRecorder;
static RECORDER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RECORDER: OnceLock<Module> = OnceLock::new();

impl MultiRecorder {
    /// The shape of `MultiRecorder`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let go = s.pin(PinSpec::event("go", role::PORT));
        let log = s.pin(PinSpec { flags: pin_flag::RECORD, ..PinSpec::discrete("log", role::PORT, &BYTE) });
        s.reaction(ReactionSpec { name: "emit", on: vec![go], writes: vec![DeviceParts::statement(log, 1, 0)], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<MultiRecorder>(&RECORDER, "recorder")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<MultiRecorder>(&RECORDER, "recorder")
    }
}

impl Device for MultiRecorder {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&RECORDER_SHAPE, MultiRecorder::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((MultiRecorder, Arc::clone(DeviceParts::shared(&RECORDER_SHAPE, MultiRecorder::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("recorder", true);
        for i in 1..=8u8 {
            cx.commit(0, &[i], 0.0)?;
        }
        Ok(Flow::Continue)
    }
}

// ─── Settle recorder ────────────────────────────────────────────────────────

/// `on settle { emit!(..) }`: one record when the settle phase runs.
///
/// pins: log=0 (PORT BYTE, RECORD); settle=1 (GLOBAL SETTLE event)
/// reaction 0: on [settle] writes [S0 {log}]; commits `[7]`, no delay
pub struct SettleRecorder;
static SETTLE_RECORDER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_RECORDER: OnceLock<Module> = OnceLock::new();

impl SettleRecorder {
    /// The shape of `SettleRecorder`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let log = s.pin(PinSpec { flags: pin_flag::RECORD, ..PinSpec::discrete("log", role::PORT, &BYTE) });
        let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        s.reaction(ReactionSpec { name: "emit", on: vec![settle], writes: vec![DeviceParts::statement(log, 1, 0)], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<SettleRecorder>(&SETTLE_RECORDER, "settle_recorder")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<SettleRecorder>(&SETTLE_RECORDER, "settle_recorder")
    }
}

impl Device for SettleRecorder {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&SETTLE_RECORDER_SHAPE, SettleRecorder::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleRecorder, Arc::clone(DeviceParts::shared(&SETTLE_RECORDER_SHAPE, SettleRecorder::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("settle_recorder", true);
        cx.commit(0, &[7], 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── CheckedParameter ────────────────────────────────────────────────────────────────

/// A parameter with a `where r > 0` check, and a setup function that asks for another
/// shape above 100 (abi.md §3.1 P6, §9).
///
/// pins: r=0 (PARAM F64, default 1.0, check -> ok); ok=1 (PRIVATE BIT)
/// fn 0 "setup": reads [r] writes [ok]; `RESHAPE` when r > 100
pub struct CheckedParameter;
static CHECKED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CHECKED: OnceLock<Module> = OnceLock::new();
const ONE: [u8; 8] = 1.0f64.to_le_bytes();

impl CheckedParameter {
    /// The shape of `CheckedParameter`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec {
            flags: pin_flag::HAS_INIT | pin_flag::HAS_CHECK,
            init: &ONE,
            check: 1,
            ..PinSpec::discrete("r", role::PARAM, &F64)
        });
        let ok = s.pin(PinSpec::discrete("ok", role::PRIVATE, &BIT));
        s.function(FunctionSpec { name: "setup", reads: vec![DeviceParts::pin(0)], writes: vec![DeviceParts::pin(ok)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<CheckedParameter>(&CHECKED, "checked")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<CheckedParameter>(&CHECKED, "checked")
    }
}

impl Device for CheckedParameter {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&CHECKED_SHAPE, CheckedParameter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CheckedParameter, Arc::clone(DeviceParts::shared(&CHECKED_SHAPE, CheckedParameter::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("checked", false);
        let r = cx.f64(0);
        if r > 100.0 {
            return Err(Fault::reshape("r above 100 needs another shape"));
        }
        cx.out_bytes(0).copy_from_slice(&DeviceParts::bit(r > 0.0));
        Ok(())
    }
}

// ─── BitFaulty ─────────────────────────────────────────────────────────────────

/// A function that fails on demand: a known high `a` is a `FAULT` (code 7, "boom"), a known low
/// `a` is a `RESHAPE`. Used outside the parameter cone, where both are faults.
///
/// pins: a=0, y=1 (PORT BIT)   fn 0 "fail": reads [a] writes [y]
pub struct BitFaulty;
static FAULTY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FAULTY: OnceLock<Module> = OnceLock::new();

impl BitFaulty {
    /// The shape of `BitFaulty`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec { name: "fail", reads: vec![DeviceParts::pin(a)], writes: vec![DeviceParts::pin(y)], ..FunctionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<BitFaulty>(&FAULTY, "faulty")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<BitFaulty>(&FAULTY, "faulty")
    }
}

impl Device for BitFaulty {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&FAULTY_SHAPE, BitFaulty::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((BitFaulty, Arc::clone(DeviceParts::shared(&FAULTY_SHAPE, BitFaulty::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("faulty", false);
        match cx.bytes(0) {
            [1] => Err(Fault::new(7, "boom")),
            [0] => Err(Fault::reshape("another shape")),
            _ => Ok(()),
        }
    }
}

// ─── Sampler ────────────────────────────────────────────────────────────────

/// A reaction that reads a continuous pin: its sample becomes an `F64` when `go` fires.
///
/// pins: v=0 (PORT continuous); go=1 (PORT event); s=2 (PORT F64)
/// reaction 0: on [go]  reads [v]  writes [S0 {s}]; s <- the sample, no delay
pub struct Sampler;
static SAMPLER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SAMPLER: OnceLock<Module> = OnceLock::new();

impl Sampler {
    /// The shape of `Sampler`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let v = s.pin(PinSpec::continuous("v", role::PORT));
        let go = s.pin(PinSpec::event("go", role::PORT));
        let out = s.pin(PinSpec::discrete("s", role::PORT, &F64));
        s.reaction(ReactionSpec {
            name: "sample",
            on: vec![go],
            reads: vec![DeviceParts::pin(v)],
            writes: vec![DeviceParts::statement(out, 8, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Sampler>(&SAMPLER, "sampler")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Sampler>(&SAMPLER, "sampler")
    }
}

impl Device for Sampler {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&SAMPLER_SHAPE, Sampler::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Sampler, Arc::clone(DeviceParts::shared(&SAMPLER_SHAPE, Sampler::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("sampler", true);
        cx.commit(0, &DeviceParts::real(cx.f64(0)), 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Stub ───────────────────────────────────────────────────────────────────

/// An emulator stub whose reaction is `IRREVERSIBLE` (abi.md §5 R6): it cannot be undone.
///
/// pins: go=0 (PORT event); out=1 (PORT BIT, init LOW)
/// reaction 0 "run" (IRREVERSIBLE): on [go]  writes [S0 {out}]; out <- HIGH, no delay
pub struct Stub;
static STUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STUB: OnceLock<Module> = OnceLock::new();

impl Stub {
    /// The shape of `Stub`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let go = s.pin(PinSpec::event("go", role::PORT));
        let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        s.reaction(ReactionSpec {
            name: "run",
            flags: reaction_flag::IRREVERSIBLE,
            on: vec![go],
            writes: vec![DeviceParts::statement(out, 1, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Stub>(&STUB, "stub")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Stub>(&STUB, "stub")
    }
}

impl Device for Stub {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&STUB_SHAPE, Stub::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Stub, Arc::clone(DeviceParts::shared(&STUB_SHAPE, Stub::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("stub", true);
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Odd ────────────────────────────────────────────────────────────────────

/// A reaction that misbehaves in a way each event picks.
///
/// pins: out=0 (PORT BIT); e0..e2=1..3 (PORT event)
/// reaction k on [e_k], statements [S0 {out}] (reaction 0: S0 {out, LATER, min_after 1 ns}):
///   0  commits after 0.5 ns through the LATER statement above
///   1  returns FINISH
///   2  returns a FAULT, code 9, "bang"
pub struct Odd;
static ODD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static ODD: OnceLock<Module> = OnceLock::new();

impl Odd {
    /// The shape of `Odd`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        let events: Vec<u32> = (0..3).map(|k| s.pin(PinSpec::event(&format!("e{k}"), role::PORT))).collect();
        for (k, e) in events.iter().enumerate() {
            let later = if k == 0 {
                Statement { flags: statement_flag::LATER, min_after: 1e-9, ..DeviceParts::statement(out, 1, 0) }
            } else {
                DeviceParts::statement(out, 1, 0)
            };
            s.reaction(ReactionSpec { name: "odd", on: vec![*e], writes: vec![later], ..ReactionSpec::default() });
        }
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Odd>(&ODD, "odd")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Odd>(&ODD, "odd")
    }
}

impl Device for Odd {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&ODD_SHAPE, Odd::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Odd, Arc::clone(DeviceParts::shared(&ODD_SHAPE, Odd::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("odd", true);
        match k {
            0 => cx.commit(0, &[1], 0.5e-9)?,
            1 => return Ok(Flow::Finish),
            _ => return Err(Fault::new(9, "bang")),
        }
        Ok(Flow::Continue)
    }
}

// ─── Rogue (a foreign device) ───────────────────────────────────────────────

/// A device written against the C contract directly, so nothing between it and the engine checks
/// its commits (the Rust `Device` path refuses a commit longer than its statement before the host
/// sees it). Its commits break the statement's range.
///
/// pins: out=0 (PORT BIT); go0=1, go1=2 (PORT event)
/// reaction 0 on [go0] commits three bytes through S0 {out, 2 bytes}
/// reaction 1 on [go1] commits through statement 9, which does not exist
pub struct Rogue;
static ROGUE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static ROGUE: OnceLock<Module> = OnceLock::new();

impl Rogue {
    /// The shape of the device.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        for k in 0..2 {
            let go = s.pin(PinSpec::event(&format!("go{k}"), role::PORT));
            s.reaction(ReactionSpec { name: "rogue", on: vec![go], writes: vec![DeviceParts::statement(out, 1, 0)], ..ReactionSpec::default() });
        }
        s.freeze()
    }

    /// `create`: the shape and a never-dereferenced instance.
    unsafe extern "C" fn create(_: *const Module, _: List<Value>, _: *mut Note) -> Created {
        let shape = DeviceParts::shared(&ROGUE_SHAPE, Rogue::shape).as_raw();
        Created { status: status::OK, instance: std::ptr::NonNull::<Instance>::dangling().as_ptr(), shape }
    }

    /// `destroy`: frees nothing.
    unsafe extern "C" fn destroy(_: *mut Instance) {}

    /// `eval`: no function.
    unsafe extern "C" fn eval(_: *mut Instance, _: u32, _: *const RawEval) -> u32 {
        status::OK
    }

    /// `react`: commits through a range the shape does not declare.
    unsafe extern "C" fn react(_: *mut Instance, k: u32, frame: *const RawReact) -> u32 {
        // SAFETY: abi.md §3 React: `frame` and its `host` are valid for this call.
        let (host, data) = unsafe { (&*(*frame).host, (*frame).host_data) };
        let bytes = [1u8, 1, 1];
        let (statement, len) = if k == 0 { (0, 3) } else { (9, 2) };
        // SAFETY: abi.md §2 entry 6: `commit` is called inside `react` with the frame's own `host_data`.
        unsafe { (host.commit)(data, statement, 0, len, bytes.as_ptr(), 0.0) }
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        let module = ROGUE.get_or_init(|| {
            // SAFETY: `Rogue::shape` is a valid shape that lives for the process (a `OnceLock` static), and the four
            // functions above keep the contracts of abi.md §2.
            unsafe {
                Module::from_raw_parts(
                    Str::from_static("rogue"),
                    DeviceParts::shared(&ROGUE_SHAPE, Rogue::shape).as_raw(),
                    std::ptr::null(),
                    Rogue::create,
                    Rogue::destroy,
                    Rogue::eval,
                    Rogue::react,
                )
            }
        });
        Handle::create(module, &[]).expect("a fixture device creates")
    }
}

// ─── Setter ─────────────────────────────────────────────────────────────────

/// A comparator's digital half: `out` goes high one microstep after `up` fires.
///
/// pins: up=0 (PORT event); out=1 (PORT BIT)   reaction 0: on [up] writes [S0 {out}]; out <- HIGH, no delay
pub struct Setter;
static SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTER: OnceLock<Module> = OnceLock::new();

impl Setter {
    /// The shape of `Setter`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let up = s.pin(PinSpec::event("up", role::PORT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        s.reaction(ReactionSpec { name: "set", on: vec![up], writes: vec![DeviceParts::statement(out, 1, 0)], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Setter>(&SETTER, "setter")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Setter>(&SETTER, "setter")
    }
}

impl Device for Setter {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&SETTER_SHAPE, Setter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Setter, Arc::clone(DeviceParts::shared(&SETTER_SHAPE, Setter::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("setter", true);
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Pulser ─────────────────────────────────────────────────────────────────

/// A pulse of one microstep, as `idtmod`'s `seat` makes one (docs/spec §22.2.9 rule 3): `out` goes
/// high one microstep after `up` fires and low again at the microstep after that.
///
/// pins: up=0 (PORT event); out=1 (PORT BIT, init LOW); rise=2 (PRIVATE event)
/// fn 0 "rise": reads [out, out.PREVIOUS, out.CHANGED] writes [rise.CHANGED]
/// reaction 0 "raise": on [up] writes [S0 {out}]; out <- HIGH, no delay
/// reaction 1 "lower": on [rise] writes [S0 {out}]; out <- LOW, no delay
pub struct PulseGenerator;
static PULSER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static PULSER: OnceLock<Module> = OnceLock::new();

impl PulseGenerator {
    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<PulseGenerator>(&PULSER, "pulser")
    }

    /// The device, bound by name like the others.
    pub fn handle() -> Handle {
        DeviceParts::make::<PulseGenerator>(&PULSER, "pulser")
    }

    /// Its shape: the edge function of `out` and the two reactions.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let up = s.pin(PinSpec::event("up", role::PORT));
        let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
        s.function(DeviceParts::edge_function(out, rise));
        s.reaction(ReactionSpec { name: "raise", on: vec![up], writes: vec![DeviceParts::statement(out, 1, 0)], ..ReactionSpec::default() });
        s.reaction(ReactionSpec { name: "lower", on: vec![rise], writes: vec![DeviceParts::statement(out, 1, 0)], ..ReactionSpec::default() });
        s.freeze()
    }
}

impl Device for PulseGenerator {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&PULSER_SHAPE, PulseGenerator::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((PulseGenerator, Arc::clone(DeviceParts::shared(&PULSER_SHAPE, PulseGenerator::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        CallCounts::bump("pulser", false);
        cx.set(0, DeviceParts::rising(cx) as u8);
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("pulser", true);
        cx.commit(0, if k == 0 { &HIGH } else { &LOW }, 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Settler ────────────────────────────────────────────────────────────────

/// A reaction on `SETTLE` that commits after `delay` seconds.
///
/// pins: delay=0 (PARAM F64, default 0); out=1 (PORT BIT); settle=2 (GLOBAL SETTLE event)
/// reaction 0: on [settle] reads [delay] writes [S0 {out}]; out <- HIGH after delay
pub struct Settler;
static SETTLER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLER: OnceLock<Module> = OnceLock::new();

impl Settler {
    /// The shape of `Settler`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let delay = s.pin(PinSpec::discrete("delay", role::PARAM, &F64));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        s.reaction(ReactionSpec {
            name: "settle",
            on: vec![settle],
            reads: vec![DeviceParts::pin(delay)],
            writes: vec![DeviceParts::statement(out, 1, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Settler>(&SETTLER, "settler")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Settler>(&SETTLER, "settler")
    }
}

impl Device for Settler {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&SETTLER_SHAPE, Settler::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Settler, Arc::clone(DeviceParts::shared(&SETTLER_SHAPE, Settler::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("settler", true);
        cx.commit(0, &HIGH, cx.f64(0))?;
        Ok(Flow::Continue)
    }
}

// ─── Settle event ───────────────────────────────────────────────────────────

/// `on settle && en.high() { out <- HIGH after delay }` as the plan builds it (§13 rule 17): an
/// event function reads the `SETTLE` pin's `CHANGED` byte and the guard, and fires a private event
/// pin the reaction listens to; the reaction never names `SETTLE`.
///
/// pins: delay=0 (PARAM F64, default 0); en=1 (PORT BIT); out=2 (PORT BIT); settle=3 (GLOBAL SETTLE
///       event); seen=4 (PRIVATE event)
/// fn 0 "settle_event": reads [en.VALUE, settle.CHANGED] writes [seen]; fires when both hold
/// reaction 0: on [seen] reads [delay] writes [S0 {out}]; out <- HIGH after delay
pub struct SettleEvent;
static SETTLE_EVENT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_EVENT: OnceLock<Module> = OnceLock::new();

impl SettleEvent {
    /// The shape of `SettleEvent`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let delay = s.pin(PinSpec::discrete("delay", role::PARAM, &F64));
        let en = s.pin(PinSpec::discrete("en", role::PORT, &BIT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        let seen = s.pin(PinSpec::event("seen", role::PRIVATE));
        s.function(FunctionSpec {
            name: "settle_event",
            reads: vec![DeviceParts::pin(en), DeviceParts::pin_part(settle, part::CHANGED)],
            writes: vec![DeviceParts::pin_part(seen, part::CHANGED)],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "on_seen",
            on: vec![seen],
            reads: vec![DeviceParts::pin(delay)],
            writes: vec![DeviceParts::statement(out, 1, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<SettleEvent>(&SETTLE_EVENT, "settle_event")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<SettleEvent>(&SETTLE_EVENT, "settle_event")
    }
}

impl Device for SettleEvent {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&SETTLE_EVENT_SHAPE, SettleEvent::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleEvent, Arc::clone(DeviceParts::shared(&SETTLE_EVENT_SHAPE, SettleEvent::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let guard_holds = cx.bytes(0) == HIGH;
        let settling = cx.bytes(1) == [1];
        cx.set(0, (guard_holds && settling) as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("settle_event", true);
        cx.commit(0, &HIGH, cx.f64(0))?;
        Ok(Flow::Continue)
    }
}

// ─── LaterDelayed ────────────────────────────────────────────────────────────────

/// A statement that promises a delay of at least 1 ns (`LATER`) and commits after 2 ns.
///
/// pins: go=0 (PORT event); out=1 (PORT BIT)
/// reaction 0: on [go] writes [S0 {out, LATER, min_after 1 ns}]; out <- HIGH after 2 ns
pub struct LaterDelayed;
static DELAYED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DELAYED: OnceLock<Module> = OnceLock::new();

impl LaterDelayed {
    /// The shape of `LaterDelayed`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let go = s.pin(PinSpec::event("go", role::PORT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        s.reaction(ReactionSpec {
            name: "later",
            on: vec![go],
            writes: vec![Statement { flags: statement_flag::LATER, min_after: 1e-9, ..DeviceParts::statement(out, 1, 0) }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<LaterDelayed>(&DELAYED, "delayed")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<LaterDelayed>(&DELAYED, "delayed")
    }
}

impl Device for LaterDelayed {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&DELAYED_SHAPE, LaterDelayed::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LaterDelayed, Arc::clone(DeviceParts::shared(&DELAYED_SHAPE, LaterDelayed::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("delayed", true);
        cx.commit(0, &HIGH, 2e-9)?;
        Ok(Flow::Continue)
    }
}

// ─── Lagged ─────────────────────────────────────────────────────────────────

/// A statement that commits after 2 ns and promises nothing: its delay comes from an expression
/// the shape cannot bound (the buck's `Osc`, whose delay is `0.5 / f`), so it carries no `LATER`.
///
/// pins: go=0 (PORT event); out=1 (PORT BIT)
/// reaction 0: on [go] writes [S0 {out}]; out <- HIGH after 2 ns
pub struct Lagged;
static LAGGED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LAGGED: OnceLock<Module> = OnceLock::new();

impl Lagged {
    /// The shape of `Lagged`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let go = s.pin(PinSpec::event("go", role::PORT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        s.reaction(ReactionSpec { name: "lagged", on: vec![go], writes: vec![DeviceParts::statement(out, 1, 0)], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Lagged>(&LAGGED, "lagged")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Lagged>(&LAGGED, "lagged")
    }
}

impl Device for Lagged {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&LAGGED_SHAPE, Lagged::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Lagged, Arc::clone(DeviceParts::shared(&LAGGED_SHAPE, Lagged::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("lagged", true);
        cx.commit(0, &HIGH, 2e-9)?;
        Ok(Flow::Continue)
    }
}

// ─── Clockwatch ─────────────────────────────────────────────────────────────

static TICKS_FIELDS: [Field; 2] = [
    Field { name: Str::from_static("tick"), leaf: leaf::I64, offset: 0, count: 1 },
    Field { name: Str::from_static("horizon"), leaf: leaf::I64, offset: 8, count: 1 },
];
/// Two `I64`s: the `TICK` and `HORIZON` a reaction read.
pub static TICKS: Layout = Layout { size: 16, align: 8, fields: List::from_slice(&TICKS_FIELDS) };

/// Writes down the `TICK` and `HORIZON` it was given when `go` fires.
///
/// pins: go=0 (PORT event); seen=1 (PORT TICKS)
/// reaction 0: on [go] reads [TICK, HORIZON] writes [S0 {seen}]; seen <- (tick, horizon), no delay
pub struct ClockWatch;
static CLOCKWATCH_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CLOCKWATCH: OnceLock<Module> = OnceLock::new();

impl ClockWatch {
    /// The shape of `ClockWatch`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let go = s.pin(PinSpec::event("go", role::PORT));
        let seen = s.pin(PinSpec::discrete("seen", role::PORT, &TICKS));
        let ambient = |index| Cell { kind: cell::AMBIENT, index, part: part::VALUE };
        s.reaction(ReactionSpec {
            name: "watch",
            on: vec![go],
            reads: vec![ambient(piperine_abi::raw::ambient::TICK), ambient(piperine_abi::raw::ambient::HORIZON)],
            writes: vec![DeviceParts::statement(seen, 16, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<ClockWatch>(&CLOCKWATCH, "clockwatch")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<ClockWatch>(&CLOCKWATCH, "clockwatch")
    }
}

impl Device for ClockWatch {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&CLOCKWATCH_SHAPE, ClockWatch::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ClockWatch, Arc::clone(DeviceParts::shared(&CLOCKWATCH_SHAPE, ClockWatch::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("clockwatch", true);
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&cx.i64(0).to_le_bytes());
        bytes[8..].copy_from_slice(&cx.i64(1).to_le_bytes());
        cx.commit(0, &bytes, 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── Rearm ──────────────────────────────────────────────────────────────────

/// An `IRREVERSIBLE` reaction that fires its own event again 5 fs later.
///
/// pins: a=0 (PORT event)   reaction 0 (IRREVERSIBLE): on [a] writes [S0 {a}]; fires `a` after 5 fs
pub struct Rearm;
static REARM_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static REARM: OnceLock<Module> = OnceLock::new();

impl Rearm {
    /// The shape of `Rearm`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::event("a", role::PORT));
        s.reaction(ReactionSpec {
            name: "rearm",
            flags: reaction_flag::IRREVERSIBLE,
            on: vec![a],
            writes: vec![DeviceParts::statement(a, 0, 0)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The module of the device.
    pub fn module() -> &'static Module {
        DeviceParts::module_of::<Rearm>(&REARM, "rearm")
    }

    /// An instance of the device.
    pub fn handle() -> Handle {
        DeviceParts::make::<Rearm>(&REARM, "rearm")
    }
}

impl Device for Rearm {
    fn prototype() -> &'static Shape<'static> {
        DeviceParts::shared(&REARM_SHAPE, Rearm::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Rearm, Arc::clone(DeviceParts::shared(&REARM_SHAPE, Rearm::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        CallCounts::bump("rearm", true);
        cx.commit(0, &[], 5e-15)?;
        Ok(Flow::Continue)
    }
}

/// The per-thread count of the `eval` and `react` calls each fixture device received.
pub struct CallCounts;

impl CallCounts {
    /// Counts one `eval` (or, when `react` is set, one reaction) of the device called `device` on this thread.
    pub(crate) fn bump(device: &'static str, react: bool) {
        CALLS.with(|c| {
            let mut c = c.borrow_mut();
            let e = c.entry(device).or_default();
            if react { e.1 += 1 } else { e.0 += 1 }
        });
    }

    /// The `(eval, react)` calls this thread made on devices called `device`.
    pub fn of(device: &str) -> (u32, u32) {
        CALLS.with(|c| c.borrow().get(device).copied().unwrap_or_default())
    }

    /// Forgets every count of this thread.
    pub fn reset() {
        CALLS.with(|c| c.borrow_mut().clear());
    }
}

/// The cells, bytes and shared shapes every digital fixture is built from.
pub struct DeviceParts;

impl DeviceParts {
    // ─── Shape helpers ──────────────────────────────────────────────────────────

    /// The cell of the value of pin `index`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of part `part` (`PREVIOUS`, `CHANGED`, ...) of pin `index`.
    fn pin_part(index: u32, part: u32) -> Cell {
        Cell { kind: cell::PIN, index, part }
    }

    /// `v` bytes of a `BIT`: the value.
    fn bit(value: bool) -> [u8; 1] {
        [value as u8]
    }

    /// An `F64` carrier: its value bytes.
    fn real(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// The statement that writes `len` bytes of pin `pin` with `flags` and no minimum delay.
    fn statement(pin: u32, len: u32, flags: u32) -> Statement {
        Statement { pin, offset: 0, len, flags, min_after: 0.0 }
    }

    /// The `u32` held in a `COUNT` pin's bytes.
    fn count_of(bytes: &[u8]) -> u32 {
        u32::from_le_bytes(bytes[..4].try_into().expect("a COUNT is four bytes"))
    }

    /// A shape built once and shared by every instance (abi.md §9).
    fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// Both halves of a device, per type: the shape and the module.
    fn module_of<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
        slot.get_or_init(|| module::<D>(name))
    }

    /// An instance of `D` with no param values.
    fn make<D: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> Handle {
        Handle::create(DeviceParts::module_of::<D>(slot, name), &[]).expect("a fixture device creates")
    }

    /// A function that fires event `rise` on a rising edge of `clk` (abi.md §11.4).
    fn edge_function(clk: u32, rise: u32) -> FunctionSpec<'static> {
        FunctionSpec {
            name: "rise",
            reads: vec![DeviceParts::pin(clk), DeviceParts::pin_part(clk, part::PREVIOUS), DeviceParts::pin_part(clk, part::CHANGED)],
            writes: vec![DeviceParts::pin_part(rise, part::CHANGED)],
            ..FunctionSpec::default()
        }
    }

    /// The edge test of `edge_function`'s frame: value, previous, changed.
    fn rising(cx: &Eval<'_>) -> bool {
        cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW
    }
}
