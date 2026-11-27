//! The devices of the solver target `digital_settle_subscriber`, one type per device.

use std::cell::RefCell;
use std::result::Result;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, HIGH, LOW};
use piperine_abi::raw::{Cell, Module, Shape, Statement, ambient, cell, global, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

thread_local! {
    /// `(time, value)` of every `SETTLE` the subscriber saw on this thread.
    pub static SETTLED: RefCell<Vec<(f64, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

/// Drives `a` from `LOW` to `HIGH` when `go` fires.
/// pins: a=0 (PORT BIT, init LOW); go=1 (PORT event); reaction 0: on [go] writes [S0 {a}]
pub struct InitialLowSetter;
static SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTER: OnceLock<Module> = OnceLock::new();

impl Device for InitialLowSetter {
    fn prototype() -> &'static Shape<'static> {
        SETTER_SHAPE.get_or_init(DigitalSettleSubscriberShapes::setter_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((InitialLowSetter, Arc::clone(SETTER_SHAPE.get_or_init(DigitalSettleSubscriberShapes::setter_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// An inverter one microstep late: a reaction writes `y <- NOT a` with no
/// delay (due the next microstep), so a reader of both `a` and `y` sees the
/// new `a` beside the old `y` for one microstep (a delta-cycle glitch).
/// pins: a=0 (PORT BIT); y=1 (PORT BIT, init HIGH); flip=2 (PRIVATE event)
/// fn 0 "edge": reads [a.CHANGED] writes [flip.CHANGED]
/// reaction 0: on [flip] reads [a] writes [S0 {y}]
pub struct LagInverter;
static LAG_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LAG: OnceLock<Module> = OnceLock::new();

impl Device for LagInverter {
    fn prototype() -> &'static Shape<'static> {
        LAG_SHAPE.get_or_init(DigitalSettleSubscriberShapes::lag_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LagInverter, Arc::clone(LAG_SHAPE.get_or_init(DigitalSettleSubscriberShapes::lag_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let changed = cx.bytes(0) == [1];
        cx.set(0, changed as u8);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let y = if cx.bytes(0) == HIGH { LOW } else { HIGH };
        cx.commit(0, &y, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// Records the value of `c` at every `SETTLE`.
/// pins: settle=0 (GLOBAL SETTLE event); c=1 (PORT BIT)
/// reaction 0 "settled": on [settle] reads [TIME, c]
pub struct Subscriber;
static SUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUB: OnceLock<Module> = OnceLock::new();

impl Device for Subscriber {
    fn prototype() -> &'static Shape<'static> {
        SUB_SHAPE.get_or_init(DigitalSettleSubscriberShapes::sub_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Subscriber, Arc::clone(SUB_SHAPE.get_or_init(DigitalSettleSubscriberShapes::sub_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let (t, c) = (cx.f64(0), cx.bytes(1).to_vec());
        SETTLED.with(|s| s.borrow_mut().push((t, c)));
        Ok(Flow::Continue)
    }
}
impl LagInverter {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        LAG.get_or_init(|| module::<LagInverter>("LagInverter"))
    }
}

impl InitialLowSetter {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SETTER.get_or_init(|| module::<InitialLowSetter>("Setter"))
    }
}

impl Subscriber {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SUB.get_or_init(|| module::<Subscriber>("Subscriber"))
    }
}

/// The shapes the `digital_settle_subscriber` devices are built from.
pub struct DigitalSettleSubscriberShapes;

impl DigitalSettleSubscriberShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The shape of `InitialLowSetter`, built once and shared by every instance.
    pub fn setter_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("a", role::PORT, &BIT) });
        let go = s.pin(PinSpec::event("go", role::PORT));
        s.reaction(ReactionSpec {
            name: "flip",
            origin: "digital_settle_subscriber.rs",
            on: vec![go],
            writes: vec![Statement { pin: a, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `LagInverter`, built once and shared by every instance.
    pub fn lag_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &HIGH, ..PinSpec::discrete("y", role::PORT, &BIT) });
        let flip = s.pin(PinSpec::event("flip", role::PRIVATE));
        s.function(FunctionSpec {
            name: "edge",
            origin: "digital_settle_subscriber.rs",
            reads: vec![Cell { kind: cell::PIN, index: a, part: part::CHANGED }],
            writes: vec![Cell { kind: cell::PIN, index: flip, part: part::CHANGED }],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "invert",
            origin: "digital_settle_subscriber.rs",
            on: vec![flip],
            reads: vec![DigitalSettleSubscriberShapes::pin(a)],
            writes: vec![Statement { pin: y, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `Subscriber`, built once and shared by every instance.
    pub fn sub_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        let c = s.pin(PinSpec::discrete("c", role::PORT, &BIT));
        s.reaction(ReactionSpec {
            name: "settled",
            origin: "digital_settle_subscriber.rs",
            on: vec![settle],
            reads: vec![Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }, DigitalSettleSubscriberShapes::pin(c)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }
}
