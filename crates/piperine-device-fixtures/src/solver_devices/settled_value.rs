//! The devices of the solver target `settled_value`, one type per device.

use std::cell::Cell as StdCell;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, HIGH, LOW};
use piperine_abi::raw::{Cell, Module, Shape, Statement, cell, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

const ORIGIN: &str = "settled_value.rs";

thread_local! {
    /// How many times `SettledWatch` has reacted in this thread.
    pub static FIRED: StdCell<u32> = const { StdCell::new(0) };
}

/// pins: x=0 (PORT BIT, init LOW); go=1 (PORT event)
/// reaction 0: on [go] writes [S0 {x}, S1 {x}]; x <- HIGH; x <- LOW, no delay
pub struct TwoWrites;
static TW_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TW: OnceLock<Module> = OnceLock::new();

impl Device for TwoWrites {
    fn prototype() -> &'static Shape<'static> {
        TW_SHAPE.get_or_init(SettledValueShapes::tw_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TwoWrites, Arc::clone(TW_SHAPE.get_or_init(SettledValueShapes::tw_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        cx.commit(1, &LOW, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// pins: x=0 (PORT BIT); flip=1 (PRIVATE event)
/// fn 0: reads [x.CHANGED] writes [flip.CHANGED];  reaction 0: on [flip] counts
pub struct SettledWatch;
static W_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static W: OnceLock<Module> = OnceLock::new();

impl Device for SettledWatch {
    fn prototype() -> &'static Shape<'static> {
        W_SHAPE.get_or_init(SettledValueShapes::w_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettledWatch, Arc::clone(W_SHAPE.get_or_init(SettledValueShapes::w_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let changed = cx.bytes(0) == [1];
        cx.set(0, changed as u8);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        FIRED.with(|f| f.set(f.get() + 1));
        Ok(Flow::Continue)
    }
}
impl TwoWrites {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        TW.get_or_init(|| module::<TwoWrites>("TwoWrites"))
    }
}

impl SettledWatch {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        W.get_or_init(|| module::<SettledWatch>("Watch"))
    }
}

/// The shapes the `settled_value` devices are built from.
pub struct SettledValueShapes;

impl SettledValueShapes {
    /// The shape of `TwoWrites`, built once and shared by every instance.
    pub fn tw_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let x = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("x", role::PORT, &BIT) });
        let go = s.pin(PinSpec::event("go", role::PORT));
        let st = Statement { pin: x, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        s.reaction(ReactionSpec { name: "two", origin: ORIGIN, on: vec![go], writes: vec![st, st], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The shape of `SettledWatch`, built once and shared by every instance.
    pub fn w_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let x = s.pin(PinSpec::discrete("x", role::PORT, &BIT));
        let flip = s.pin(PinSpec::event("flip", role::PRIVATE));
        s.function(FunctionSpec {
            name: "edge",
            origin: ORIGIN,
            reads: vec![Cell { kind: cell::PIN, index: x, part: part::CHANGED }],
            writes: vec![Cell { kind: cell::PIN, index: flip, part: part::CHANGED }],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec { name: "count", origin: ORIGIN, on: vec![flip], ..ReactionSpec::default() });
        s.freeze()
    }
}
