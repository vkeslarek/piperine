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
    Cell, Field, Layout, List, Module, Shape, Statement, Str, cell, global, kind, leaf, part, pin_flag, reaction_flag,
    role, statement_flag,
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
/// pins: d0=0, d1=1 (PORT BIT, RELAX); v=2 (PORT BIT)
/// fn 0 "resolve": reads [d0, d1] writes [v]
pub struct Resolver;
static RESOLVER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RESOLVER: OnceLock<Module> = OnceLock::new();

fn resolver_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let d0 = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("d0", role::PORT, &BIT) });
    let d1 = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("d1", role::PORT, &BIT) });
    let v = s.pin(PinSpec::discrete("v", role::PORT, &BIT));
    s.function(FunctionSpec { name: "resolve", reads: vec![pin(d0), pin(d1)], writes: vec![pin(v)], ..FunctionSpec::default() });
    s.freeze()
}

impl Device for Resolver {
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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
    fn prototype() -> &'static Shape {
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

// ─── Binding by name ────────────────────────────────────────────────────────

/// One device to place: its path, the live instance and `(pin name, net name)` wires.
pub type Placed<'a> = (&'a str, Handle, Vec<(&'a str, &'a str)>);

/// Binds `devices` by net name. A wired pin goes to the named net, created on first use from the
/// pin's layout and flags (`QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` of any pin on it, and the
/// first `init`). A pin with no wire gets a net of its own, named `<path>.<pin>`; an unwired
/// `GLOBAL` pin goes to the design-wide net `global:<constant>` (an unwired `OPTIONAL` one to
/// nothing). A continuous pin binds to the sample slot of its wire's name, slots numbered in the
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
            if wired.is_none() && p.role == role::GLOBAL && p.flags & pin_flag::OPTIONAL != 0 {
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
                spec.init = p.init.as_slice().to_vec();
                nets.push(spec);
                NetId(nets.len() as u32 - 1)
            });
            let spec = &mut nets[id.0 as usize];
            spec.quiet |= p.flags & pin_flag::QUIET != 0;
            spec.record |= p.flags & pin_flag::RECORD != 0;
            spec.relax |= p.flags & pin_flag::RELAX != 0;
            spec.structural |= p.flags & pin_flag::STRUCTURAL != 0;
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
