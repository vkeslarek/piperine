//! Hand-written devices for the ABI engine's tests (feature
//! `solver-abi-digital`, SAD-06 and SAD-08; abi.md §11.4 and §11.7), and
//! `bind_by_name`, which binds a list of them by net name.
//!
//! Every device is a plain `impl Device` over a shape built once. Pin names
//! and cells are listed above each device. Each `eval` and `react` bumps a
//! per-thread call counter (`calls`), so a test can count the calls its own
//! engine made: a test runs on its own thread, so counts never mix.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use piperine_abi::caller::Handle;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW, REAL};
use piperine_abi::raw::{
    Cell, Created, Eval as RawEval, Field, Instance, Layout, List, Module, Note, React as RawReact, Shape, Statement, Str,
    Value, cell, global, kind, leaf, part, pin_flag, reaction_flag, role, statement_flag, status,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_digital_solver::abi::net::{NetId, NetKind, NetSpec};
use piperine_digital_solver::abi::plan::{Binding, DigitalPlan, InstancePlan};
use piperine_digital_solver::abi::tick::Resolution;
use piperine_digital_solver::error::DigitalError;

// ─── Counting calls ─────────────────────────────────────────────────────────

thread_local! {
    static CALLS: RefCell<HashMap<&'static str, (u32, u32)>> = RefCell::new(HashMap::new());
}

fn bump(device: &'static str, react: bool) {
    CALLS.with(|c| {
        let mut c = c.borrow_mut();
        let e = c.entry(device).or_default();
        if react { e.1 += 1 } else { e.0 += 1 }
    });
}

/// The `(eval, react)` calls this thread made on devices called `device`.
pub fn calls(device: &str) -> (u32, u32) {
    CALLS.with(|c| c.borrow().get(device).copied().unwrap_or_default())
}

/// Forgets every count of this thread.
pub fn reset_calls() {
    CALLS.with(|c| c.borrow_mut().clear());
}

// ─── Shape helpers ──────────────────────────────────────────────────────────

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}
fn pin_part(index: u32, part: u32) -> Cell {
    Cell { kind: cell::PIN, index, part }
}
/// `v` bytes of a `BIT`: the value and its known byte.
fn bit(value: bool, known: bool) -> [u8; 2] {
    if known { [value as u8, 1] } else { [0, 0] }
}
/// A `REAL`: value and known byte.
fn real(v: f64) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&v.to_le_bytes());
    b[8] = 1;
    b
}
fn statement(pin: u32, len: u32, flags: u32) -> Statement {
    Statement { pin, offset: 0, len, flags, min_after: 0.0 }
}

/// The `u32` held in a `COUNT` pin's bytes.
fn count_of(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[..4].try_into().expect("a COUNT is four bytes"))
}

static COUNT_FIELDS: [Field; 1] =
    [Field { name: Str::from_static("v"), leaf: leaf::U32, offset: 0, count: 1, flags: 0, known: 0 }];
/// One `U32` with no known byte: a field that can move with no "unknown" to fall back to.
pub static COUNT: Layout = Layout { size: 4, align: 4, fields: List::from_slice(&COUNT_FIELDS) };

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
    Handle::create(module_of::<D>(slot, name), &[]).expect("a fixture device creates")
}

/// A function that fires event `rise` on a rising edge of `clk` (abi.md §11.4).
fn edge_function(clk: u32, rise: u32) -> FunctionSpec<'static> {
    FunctionSpec {
        name: "rise",
        reads: vec![pin(clk), pin_part(clk, part::PREVIOUS), pin_part(clk, part::CHANGED)],
        writes: vec![pin_part(rise, part::CHANGED)],
        ..FunctionSpec::default()
    }
}

/// The edge test of `edge_function`'s frame: value, previous, changed.
fn rising(cx: &Eval<'_>) -> bool {
    cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW
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
    s.function(FunctionSpec { name: "inv", reads: vec![pin(a)], writes: vec![pin(y)], ..FunctionSpec::default() });
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
        bump("inverter", false);
        let a = cx.bytes(0);
        let y = bit(a[0] == 0, a[1] != 0);
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn inverter() -> Handle {
    make::<Inverter>(&INVERTER, "inverter")
}

// ─── And2 ───────────────────────────────────────────────────────────────────

/// pins: a=0, b=1, y=2 (PORT BIT).  fn 0 "and": reads [a, b] writes [y]; known when both are.
pub struct And2;
static AND2_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static AND2: OnceLock<Module> = OnceLock::new();

fn and2_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
    let b = s.pin(PinSpec::discrete("b", role::PORT, &BIT));
    let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
    s.function(FunctionSpec { name: "and", reads: vec![pin(a), pin(b)], writes: vec![pin(y)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for And2 {
    fn prototype() -> &'static Shape<'static> {
        shared(&AND2_SHAPE, and2_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((And2, Arc::clone(shared(&AND2_SHAPE, and2_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        bump("and2", false);
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let y = bit(a[0] != 0 && b[0] != 0, a[1] != 0 && b[1] != 0);
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn and2() -> Handle {
    make::<And2>(&AND2, "and2")
}

// ─── Dff (abi.md §11.4) ─────────────────────────────────────────────────────

/// pins: clk=0, d=1 (PORT BIT); q=2 (PORT BIT, init LOW); rise=3 (PRIVATE event)
/// fn 0 "rise":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]
/// reaction 0:   on [rise]   reads [d]   writes [S0 {q}]; q <- d, no delay
pub struct Dff;
static DFF_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DFF: OnceLock<Module> = OnceLock::new();

fn dff_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
    let d = s.pin(PinSpec::discrete("d", role::PORT, &BIT));
    let q = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("q", role::PORT, &BIT) });
    let rise = s.pin(PinSpec::event("rise", role::PRIVATE));
    s.function(edge_function(clk, rise));
    s.reaction(ReactionSpec {
        name: "sample",
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
        bump("dff", false);
        cx.set(0, rising(cx) as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("dff", true);
        cx.commit(0, cx.bytes(0), 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn dff() -> Handle {
    make::<Dff>(&DFF, "dff")
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

fn timer_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let half = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &TEN_NS, ..PinSpec::discrete("half", role::PARAM, &F64) });
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    let tick = s.pin(PinSpec::event("tick", role::PRIVATE));
    let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
    s.reaction(ReactionSpec {
        name: "run",
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
        bump("timer", true);
        let out = cx.bytes(0);
        let half = cx.f64(1);
        cx.commit(0, &bit(out[0] == 0, true), half)?;
        cx.commit(1, &[], half)?;
        Ok(Flow::Continue)
    }
}

pub fn timer() -> Handle {
    make::<Timer>(&TIMER, "timer")
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

fn counter_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let clk = s.pin(PinSpec::discrete("clk", role::PORT, &BIT));
    let n = s.pin(PinSpec { flags: pin_flag::QUIET, ..PinSpec::discrete("n", role::PORT, &COUNT) });
    let inc = s.pin(PinSpec::event("inc", role::PRIVATE));
    s.function(edge_function(clk, inc));
    s.reaction(ReactionSpec {
        name: "count",
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
        bump("counter", false);
        cx.set(0, rising(cx) as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("counter", true);
        let n = count_of(cx.bytes(0)).wrapping_add(1);
        cx.commit(0, &n.to_le_bytes(), 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn counter() -> Handle {
    make::<Counter>(&COUNTER, "counter")
}

// ─── Resolver ───────────────────────────────────────────────────────────────

/// A wired-or over two drives, `unknown` when neither is known (abi.md §9 D1).
///
/// pins: d0=0 (PORT BIT, RELAX); d1=1 (PORT BIT, RELAX, OPTIONAL: unbound it reads as unknown); v=2 (PORT BIT)
/// fn 0 "resolve": reads [d0, d1] writes [v]
pub struct Resolver;
static RESOLVER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RESOLVER: OnceLock<Module> = OnceLock::new();

fn resolver_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let d0 = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("d0", role::PORT, &BIT) });
    let d1 = s.pin(PinSpec { flags: pin_flag::RELAX | pin_flag::OPTIONAL, ..PinSpec::discrete("d1", role::PORT, &BIT) });
    let v = s.pin(PinSpec::discrete("v", role::PORT, &BIT));
    s.function(FunctionSpec { name: "resolve", reads: vec![pin(d0), pin(d1)], writes: vec![pin(v)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for Resolver {
    fn prototype() -> &'static Shape<'static> {
        shared(&RESOLVER_SHAPE, resolver_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resolver, Arc::clone(shared(&RESOLVER_SHAPE, resolver_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        bump("resolver", false);
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let known = a[1] != 0 || b[1] != 0;
        let value = (a[1] != 0 && a[0] != 0) || (b[1] != 0 && b[0] != 0);
        cx.out_bytes(0).copy_from_slice(&bit(value, known));
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn resolver() -> Handle {
    make::<Resolver>(&RESOLVER, "resolver")
}

// ─── Succ ───────────────────────────────────────────────────────────────────

/// A function whose output has no "unknown": `y = a + 1`. Bound with `a` and `y` on one net it is a
/// relaxed self-loop whose field always moves (abi.md §9 D1: a field without a known byte is a fault).
///
/// pins: a=0 (PORT COUNT); y=1 (PORT COUNT, RELAX)    fn 0 "succ": reads [a] writes [y]
pub struct Succ;
static SUCC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUCC: OnceLock<Module> = OnceLock::new();

fn succ_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::discrete("a", role::PORT, &COUNT));
    let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &COUNT) });
    s.function(FunctionSpec { name: "succ", reads: vec![pin(a)], writes: vec![pin(y)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for Succ {
    fn prototype() -> &'static Shape<'static> {
        shared(&SUCC_SHAPE, succ_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Succ, Arc::clone(shared(&SUCC_SHAPE, succ_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        bump("succ", false);
        let next = count_of(cx.bytes(0)).wrapping_add(1);
        cx.set(0, next);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn succ() -> Handle {
    make::<Succ>(&SUCC, "succ")
}

// ─── Recorder ───────────────────────────────────────────────────────────────

/// Emits eight records through one statement when `go` fires.
///
/// pins: go=0 (PORT event); log=1 (PORT BYTE, RECORD)
/// reaction 0: on [go]  writes [S0 {log}]; commits `[i, known]` for i = 1 to 8, no delay
pub struct Recorder;
static RECORDER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RECORDER: OnceLock<Module> = OnceLock::new();

fn recorder_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let log = s.pin(PinSpec { flags: pin_flag::RECORD, ..PinSpec::discrete("log", role::PORT, &BYTE) });
    s.reaction(ReactionSpec { name: "emit", on: vec![go], writes: vec![statement(log, 2, 0)], ..ReactionSpec::default() });
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
        bump("recorder", true);
        for i in 1..=8u8 {
            cx.commit(0, &[i, 1], 0.0)?;
        }
        Ok(Flow::Continue)
    }
}

pub fn recorder() -> Handle {
    make::<Recorder>(&RECORDER, "recorder")
}

// ─── Checked ────────────────────────────────────────────────────────────────

/// A parameter with a `where r > 0` check, and a setup function that asks for another
/// shape above 100 (abi.md §3.1 P6, §9).
///
/// pins: r=0 (PARAM F64, default 1.0, check -> ok); ok=1 (PRIVATE BIT)
/// fn 0 "setup": reads [r] writes [ok]; `RESHAPE` when r > 100
pub struct Checked;
static CHECKED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CHECKED: OnceLock<Module> = OnceLock::new();
const ONE: [u8; 8] = 1.0f64.to_le_bytes();

fn checked_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec {
        flags: pin_flag::HAS_INIT | pin_flag::HAS_CHECK,
        init: &ONE,
        check: 1,
        ..PinSpec::discrete("r", role::PARAM, &F64)
    });
    let ok = s.pin(PinSpec::discrete("ok", role::PRIVATE, &BIT));
    s.function(FunctionSpec { name: "setup", reads: vec![pin(0)], writes: vec![pin(ok)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for Checked {
    fn prototype() -> &'static Shape<'static> {
        shared(&CHECKED_SHAPE, checked_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Checked, Arc::clone(shared(&CHECKED_SHAPE, checked_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        bump("checked", false);
        let r = cx.f64(0);
        if r > 100.0 {
            return Err(Fault::reshape("r above 100 needs another shape"));
        }
        cx.out_bytes(0).copy_from_slice(&bit(r > 0.0, true));
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn checked() -> Handle {
    make::<Checked>(&CHECKED, "checked")
}

// ─── Faulty ─────────────────────────────────────────────────────────────────

/// A function that fails on demand: a known high `a` is a `FAULT` (code 7, "boom"), a known low
/// `a` is a `RESHAPE`. Used outside the parameter cone, where both are faults.
///
/// pins: a=0, y=1 (PORT BIT)   fn 0 "fail": reads [a] writes [y]
pub struct Faulty;
static FAULTY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FAULTY: OnceLock<Module> = OnceLock::new();

fn faulty_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
    let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
    s.function(FunctionSpec { name: "fail", reads: vec![pin(a)], writes: vec![pin(y)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for Faulty {
    fn prototype() -> &'static Shape<'static> {
        shared(&FAULTY_SHAPE, faulty_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Faulty, Arc::clone(shared(&FAULTY_SHAPE, faulty_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        bump("faulty", false);
        match cx.bytes(0) {
            [1, 1] => Err(Fault::new(7, "boom")),
            [0, 1] => Err(Fault::reshape("another shape")),
            _ => Ok(()),
        }
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

pub fn faulty() -> Handle {
    make::<Faulty>(&FAULTY, "faulty")
}

// ─── Sampler ────────────────────────────────────────────────────────────────

/// A reaction that reads a continuous pin: its sample becomes a `REAL` when `go` fires.
///
/// pins: v=0 (PORT continuous); go=1 (PORT event); s=2 (PORT REAL)
/// reaction 0: on [go]  reads [v]  writes [S0 {s}]; s <- the sample, no delay
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
        bump("sampler", true);
        cx.commit(0, &real(cx.f64(0)), 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn sampler() -> Handle {
    make::<Sampler>(&SAMPLER, "sampler")
}

// ─── Stub ───────────────────────────────────────────────────────────────────

/// An emulator stub whose reaction is `IRREVERSIBLE` (abi.md §5 R6): it cannot be undone.
///
/// pins: go=0 (PORT event); out=1 (PORT BIT, init LOW)
/// reaction 0 "run" (IRREVERSIBLE): on [go]  writes [S0 {out}]; out <- HIGH, no delay
pub struct Stub;
static STUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STUB: OnceLock<Module> = OnceLock::new();

fn stub_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
    s.reaction(ReactionSpec {
        name: "run",
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
        bump("stub", true);
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn stub() -> Handle {
    make::<Stub>(&STUB, "stub")
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

fn odd_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
    let events: Vec<u32> = (0..3).map(|k| s.pin(PinSpec::event(&format!("e{k}"), role::PORT))).collect();
    for (k, e) in events.iter().enumerate() {
        let later = if k == 0 {
            Statement { flags: statement_flag::LATER, min_after: 1e-9, ..statement(out, 2, 0) }
        } else {
            statement(out, 2, 0)
        };
        s.reaction(ReactionSpec { name: "odd", on: vec![*e], writes: vec![later], ..ReactionSpec::default() });
    }
    s.freeze()
}

impl Device for Odd {
    fn prototype() -> &'static Shape<'static> {
        shared(&ODD_SHAPE, odd_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Odd, Arc::clone(shared(&ODD_SHAPE, odd_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("odd", true);
        match k {
            0 => cx.commit(0, &[1, 1], 0.5e-9)?,
            1 => return Ok(Flow::Finish),
            _ => return Err(Fault::new(9, "bang")),
        }
        Ok(Flow::Continue)
    }
}

pub fn odd() -> Handle {
    make::<Odd>(&ODD, "odd")
}

// ─── Rogue (a foreign device) ───────────────────────────────────────────────

/// A device written against the C contract directly, so nothing between it and the engine checks
/// its commits (the Rust `Device` path refuses a commit longer than its statement before the host
/// sees it). Its commits break the statement's range.
///
/// pins: out=0 (PORT BIT); go0=1, go1=2 (PORT event)
/// reaction 0 on [go0] commits three bytes through S0 {out, 2 bytes}
/// reaction 1 on [go1] commits through statement 9, which does not exist
static ROGUE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static ROGUE: OnceLock<Module> = OnceLock::new();

fn rogue_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
    for k in 0..2 {
        let go = s.pin(PinSpec::event(&format!("go{k}"), role::PORT));
        s.reaction(ReactionSpec { name: "rogue", on: vec![go], writes: vec![statement(out, 2, 0)], ..ReactionSpec::default() });
    }
    s.freeze()
}

unsafe extern "C" fn rogue_create(_: *const Module, _: List<Value>, _: *mut Note) -> Created {
    let shape = shared(&ROGUE_SHAPE, rogue_shape).as_raw();
    Created { status: status::OK, instance: std::ptr::NonNull::<Instance>::dangling().as_ptr(), shape }
}
unsafe extern "C" fn rogue_destroy(_: *mut Instance) {}
unsafe extern "C" fn rogue_eval(_: *mut Instance, _: u32, _: *const RawEval) -> u32 {
    status::OK
}
unsafe extern "C" fn rogue_react(_: *mut Instance, k: u32, frame: *const RawReact) -> u32 {
    // SAFETY: abi.md §3 React: `frame` and its `host` are valid for this call.
    let (host, data) = unsafe { (&*(*frame).host, (*frame).host_data) };
    let bytes = [1u8, 1, 1];
    let (statement, len) = if k == 0 { (0, 3) } else { (9, 2) };
    // SAFETY: abi.md §2 entry 6: `commit` is called inside `react` with the frame's own `host_data`.
    unsafe { (host.commit)(data, statement, 0, len, bytes.as_ptr(), 0.0) }
}

pub fn rogue() -> Handle {
    let module = ROGUE.get_or_init(|| {
        // SAFETY: `rogue_shape` is a valid shape that lives for the process (a `OnceLock` static), and the four
        // functions above keep the contracts of abi.md §2.
        unsafe {
            Module::from_raw_parts(
                Str::from_static("rogue"),
                shared(&ROGUE_SHAPE, rogue_shape).as_raw(),
                std::ptr::null(),
                rogue_create,
                rogue_destroy,
                rogue_eval,
                rogue_react,
            )
        }
    });
    Handle::create(module, &[]).expect("a fixture device creates")
}

// ─── Setter ─────────────────────────────────────────────────────────────────

/// A comparator's digital half: `out` goes high one microstep after `up` fires.
///
/// pins: up=0 (PORT event); out=1 (PORT BIT)   reaction 0: on [up] writes [S0 {out}]; out <- HIGH, no delay
pub struct Setter;
static SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTER: OnceLock<Module> = OnceLock::new();

fn setter_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let up = s.pin(PinSpec::event("up", role::PORT));
    let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
    s.reaction(ReactionSpec { name: "set", on: vec![up], writes: vec![statement(out, 2, 0)], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for Setter {
    fn prototype() -> &'static Shape<'static> {
        shared(&SETTER_SHAPE, setter_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Setter, Arc::clone(shared(&SETTER_SHAPE, setter_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("setter", true);
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn setter() -> Handle {
    make::<Setter>(&SETTER, "setter")
}

// ─── Settler ────────────────────────────────────────────────────────────────

/// A reaction on `SETTLE` that commits after `delay` seconds.
///
/// pins: delay=0 (PARAM F64, default 0); out=1 (PORT BIT); settle=2 (GLOBAL SETTLE event)
/// reaction 0: on [settle] reads [delay] writes [S0 {out}]; out <- HIGH after delay
pub struct Settler;
static SETTLER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLER: OnceLock<Module> = OnceLock::new();

fn settler_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let delay = s.pin(PinSpec::discrete("delay", role::PARAM, &F64));
    let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
    let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
    s.reaction(ReactionSpec {
        name: "settle",
        on: vec![settle],
        reads: vec![pin(delay)],
        writes: vec![statement(out, 2, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Settler {
    fn prototype() -> &'static Shape<'static> {
        shared(&SETTLER_SHAPE, settler_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Settler, Arc::clone(shared(&SETTLER_SHAPE, settler_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("settler", true);
        cx.commit(0, &HIGH, cx.f64(0))?;
        Ok(Flow::Continue)
    }
}

pub fn settler() -> Handle {
    make::<Settler>(&SETTLER, "settler")
}

// ─── Delayed ────────────────────────────────────────────────────────────────

/// A statement that promises a delay of at least 1 ns (`LATER`) and commits after 2 ns.
///
/// pins: go=0 (PORT event); out=1 (PORT BIT)
/// reaction 0: on [go] writes [S0 {out, LATER, min_after 1 ns}]; out <- HIGH after 2 ns
pub struct Delayed;
static DELAYED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DELAYED: OnceLock<Module> = OnceLock::new();

fn delayed_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
    s.reaction(ReactionSpec {
        name: "later",
        on: vec![go],
        writes: vec![Statement { flags: statement_flag::LATER, min_after: 1e-9, ..statement(out, 2, 0) }],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Delayed {
    fn prototype() -> &'static Shape<'static> {
        shared(&DELAYED_SHAPE, delayed_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Delayed, Arc::clone(shared(&DELAYED_SHAPE, delayed_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("delayed", true);
        cx.commit(0, &HIGH, 2e-9)?;
        Ok(Flow::Continue)
    }
}

pub fn delayed() -> Handle {
    make::<Delayed>(&DELAYED, "delayed")
}

// ─── Clockwatch ─────────────────────────────────────────────────────────────

static TICKS_FIELDS: [Field; 2] = [
    Field { name: Str::from_static("tick"), leaf: leaf::I64, offset: 0, count: 1, flags: 0, known: 0 },
    Field { name: Str::from_static("horizon"), leaf: leaf::I64, offset: 8, count: 1, flags: 0, known: 0 },
];
/// Two `I64`s with no known byte: the `TICK` and `HORIZON` a reaction read.
pub static TICKS: Layout = Layout { size: 16, align: 8, fields: List::from_slice(&TICKS_FIELDS) };

/// Writes down the `TICK` and `HORIZON` it was given when `go` fires.
///
/// pins: go=0 (PORT event); seen=1 (PORT TICKS)
/// reaction 0: on [go] reads [TICK, HORIZON] writes [S0 {seen}]; seen <- (tick, horizon), no delay
pub struct Clockwatch;
static CLOCKWATCH_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CLOCKWATCH: OnceLock<Module> = OnceLock::new();

fn clockwatch_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let go = s.pin(PinSpec::event("go", role::PORT));
    let seen = s.pin(PinSpec::discrete("seen", role::PORT, &TICKS));
    let ambient = |index| Cell { kind: cell::AMBIENT, index, part: part::VALUE };
    s.reaction(ReactionSpec {
        name: "watch",
        on: vec![go],
        reads: vec![ambient(piperine_abi::raw::ambient::TICK), ambient(piperine_abi::raw::ambient::HORIZON)],
        writes: vec![statement(seen, 16, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Clockwatch {
    fn prototype() -> &'static Shape<'static> {
        shared(&CLOCKWATCH_SHAPE, clockwatch_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Clockwatch, Arc::clone(shared(&CLOCKWATCH_SHAPE, clockwatch_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("clockwatch", true);
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&cx.i64(0).to_le_bytes());
        bytes[8..].copy_from_slice(&cx.i64(1).to_le_bytes());
        cx.commit(0, &bytes, 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn clockwatch() -> Handle {
    make::<Clockwatch>(&CLOCKWATCH, "clockwatch")
}

// ─── Sloppy ─────────────────────────────────────────────────────────────────

/// Writes a value with its known byte clear (`[1, 0]`: abi.md P3 says such a field is all zero), once
/// through a function and once through a commit.
///
/// pins: a=0, y=1, z=2 (PORT BIT); go=3 (PORT event)
/// fn 0 "bad": reads [a] writes [y]; y <- [1, 0] when a is high     reaction 0: on [go] writes [S0 {z}]; z <- [1, 0]
pub struct Sloppy;
static SLOPPY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SLOPPY: OnceLock<Module> = OnceLock::new();

fn sloppy_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
    let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
    let z = s.pin(PinSpec::discrete("z", role::PORT, &BIT));
    let go = s.pin(PinSpec::event("go", role::PORT));
    s.function(FunctionSpec { name: "bad", reads: vec![pin(a)], writes: vec![pin(y)], ..FunctionSpec::default() });
    s.reaction(ReactionSpec { name: "bad", on: vec![go], writes: vec![statement(z, 2, 0)], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for Sloppy {
    fn prototype() -> &'static Shape<'static> {
        shared(&SLOPPY_SHAPE, sloppy_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Sloppy, Arc::clone(shared(&SLOPPY_SHAPE, sloppy_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.bytes(0) == HIGH {
            cx.out_bytes(0).copy_from_slice(&[1, 0]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &[1, 0], 0.0)?;
        Ok(Flow::Continue)
    }
}

pub fn sloppy() -> Handle {
    make::<Sloppy>(&SLOPPY, "sloppy")
}

// ─── Rearm ──────────────────────────────────────────────────────────────────

/// An `IRREVERSIBLE` reaction that fires its own event again 5 fs later.
///
/// pins: a=0 (PORT event)   reaction 0 (IRREVERSIBLE): on [a] writes [S0 {a}]; fires `a` after 5 fs
pub struct Rearm;
static REARM_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static REARM: OnceLock<Module> = OnceLock::new();

fn rearm_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::event("a", role::PORT));
    s.reaction(ReactionSpec {
        name: "rearm",
        flags: reaction_flag::IRREVERSIBLE,
        on: vec![a],
        writes: vec![statement(a, 0, 0)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Rearm {
    fn prototype() -> &'static Shape<'static> {
        shared(&REARM_SHAPE, rearm_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Rearm, Arc::clone(shared(&REARM_SHAPE, rearm_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        bump("rearm", true);
        cx.commit(0, &[], 5e-15)?;
        Ok(Flow::Continue)
    }
}

pub fn rearm() -> Handle {
    make::<Rearm>(&REARM, "rearm")
}

// ─── Binding by name ────────────────────────────────────────────────────────

/// One device to place: its path, the live instance and `(pin name, net name)` wires.
pub type Placed<'a> = (&'a str, Handle, Vec<(&'a str, &'a str)>);

/// Binds `devices` by net name. A wired pin goes to the named net, created on first use from the
/// pin's layout and flags (`QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` of any pin on it, and the
/// the `init` of any pin that has one). A pin with no wire gets a net of its own, named `<path>.<pin>`; an unwired
/// `GLOBAL` pin goes to the design-wide net `global:<constant>`; an unwired `OPTIONAL` pin goes to
/// nothing. A continuous pin binds to the sample slot of its wire's name, slots numbered in the
/// order the names first appear.
pub fn bind_by_name(devices: Vec<Placed<'_>>, resolution: Resolution) -> Result<DigitalPlan, DigitalError> {
    bind_by_name_with(devices, resolution, |_| {})
}

/// `bind_by_name`, letting the test adjust the nets (`analog_reads`, say) before bind.
pub fn bind_by_name_with(
    devices: Vec<Placed<'_>>,
    resolution: Resolution,
    adjust: impl FnOnce(&mut Vec<NetSpec>),
) -> Result<DigitalPlan, DigitalError> {
    let mut nets: Vec<NetSpec> = Vec::new();
    let mut by_name: HashMap<String, NetId> = HashMap::new();
    let mut samples: HashMap<String, u32> = HashMap::new();
    let mut instances = Vec::new();
    for (path, handle, wires) in devices {
        let shape = handle.shape();
        let mut pins = Vec::new();
        for p in shape.pins.iter() {
            let name = p.name.as_str();
            let wired = wires.iter().find(|(pin, _)| *pin == name).map(|(_, net)| net.to_string());
            if p.kind == kind::CONTINUOUS {
                let key = wired.unwrap_or_else(|| format!("{path}.{name}"));
                let next = samples.len() as u32;
                pins.push(Binding::Sample(*samples.entry(key).or_insert(next)));
                continue;
            }
            if wired.is_none() && p.flags & pin_flag::OPTIONAL != 0 {
                pins.push(Binding::Absent);
                continue;
            }
            let net = wired.unwrap_or_else(|| match p.role {
                role::GLOBAL => format!("global:{}", p.global),
                _ => format!("{path}.{name}"),
            });
            let id = *by_name.entry(net.clone()).or_insert_with(|| {
                let mut spec = NetSpec::discrete(&net, p.layout.size, p.layout.align);
                if p.kind == kind::EVENT {
                    spec.kind = NetKind::Event;
                }
                nets.push(spec);
                NetId(nets.len() as u32 - 1)
            });
            let spec = &mut nets[id.0 as usize];
            spec.quiet |= p.flags & pin_flag::QUIET != 0;
            spec.record |= p.flags & pin_flag::RECORD != 0;
            spec.relax |= p.flags & pin_flag::RELAX != 0;
            spec.structural |= p.flags & pin_flag::STRUCTURAL != 0;
            if spec.init.is_empty() && p.flags & pin_flag::HAS_INIT != 0 {
                spec.init = p.init.as_slice().to_vec();
            }
            pins.push(Binding::Net(id));
        }
        instances.push(InstancePlan { path: path.to_string(), handle, pins });
    }
    adjust(&mut nets);
    DigitalPlan::bind(nets, instances, resolution)
}

/// The id of the net called `name` in a bound plan.
pub fn net_named(plan: &DigitalPlan, name: &str) -> NetId {
    let i = plan.nets.iter().position(|n| n.name == name).unwrap_or_else(|| panic!("no net called {name}"));
    NetId(i as u32)
}
