//! The devices of the solver target `array_windows`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE};
use piperine_abi::raw::{Cell, Module, Shape, Statement, WHOLE, cell, part, role};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

/// Writes `1` to its scalar pin `bit` when `go` fires; the digital engine's `array_nets` target binds it to one element of an array net.
pub struct Poker;
static POKER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static POKER: OnceLock<Module> = OnceLock::new();

impl Poker {
    /// pins: `bit` (0, `BIT`), `go` (1, event); reaction 0 on `go` writes statement 0 on `bit`.
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let bit = buffer.pin(PinSpec::discrete("bit", role::PORT, &BIT));
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        let write = Statement { pin: bit, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        buffer.reaction(ReactionSpec { name: "poke", on: vec![go], writes: vec![write], ..ReactionSpec::default() });
        buffer.freeze()
    }

    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        POKER.get_or_init(|| module::<Poker>("poker"))
    }
}

impl Device for Poker {
    fn prototype() -> &'static Shape<'static> {
        POKER_SHAPE.get_or_init(Poker::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Poker, Arc::clone(POKER_SHAPE.get_or_init(Poker::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &[1], 0.0)?;
        Ok(Flow::Continue)
    }
}

/// On `go`, counts the elements of its array pin `mem` (four elements, read whole) that are `1` into `sum`.
pub struct Summer;
static SUMMER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUMMER: OnceLock<Module> = OnceLock::new();

impl Summer {
    /// pins: `mem` (0, `BIT`, 4 elements), `go` (1, event), `sum` (2, `BYTE`).
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let mem = buffer.pin(PinSpec { count: 4, ..PinSpec::discrete("mem", role::PORT, &BIT) });
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        let sum = buffer.pin(PinSpec::discrete("sum", role::PORT, &BYTE));
        let write = Statement { pin: sum, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        buffer.reaction(ReactionSpec {
            name: "sum",
            on: vec![go],
            reads: vec![Cell { kind: cell::PIN, index: mem, part: part::VALUE }],
            elements: vec![WHOLE],
            writes: vec![write],
            ..ReactionSpec::default()
        });
        buffer.freeze()
    }

    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SUMMER.get_or_init(|| module::<Summer>("summer"))
    }
}

impl Device for Summer {
    fn prototype() -> &'static Shape<'static> {
        SUMMER_SHAPE.get_or_init(Summer::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Summer, Arc::clone(SUMMER_SHAPE.get_or_init(Summer::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let view = cx.array(0);
        let ones = (0..view.count).filter(|element| view.element(*element, part::VALUE) == Some(&[1][..])).count();
        cx.commit(0, &[ones as u8], 0.0)?;
        Ok(Flow::Continue)
    }
}