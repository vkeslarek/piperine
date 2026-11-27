//! The devices of the solver target `jump_retake`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, cell, global, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "jump_retake.rs";
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();

/// Cells, terms and partials of the hand-built shapes of this target.
pub struct Shapes;

impl Shapes {
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }
}

/// A source of `vref * code + m1 + m2 + m3` volts: the three modes add a volt each.
/// pins: vref=0 (PARAM F64, 1); code=1, m1=2, m2=3, m3=4 (PORT BIT); out=5 (PORT); f=6 (PRIVATE)
/// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
pub struct Gate;
static GATE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GATE: OnceLock<Module> = OnceLock::new();

impl Gate {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let vref = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("vref", role::PARAM, &F64) });
        let code = shape.pin(PinSpec::discrete("code", role::PORT, &BIT));
        let modes: Vec<u32> = ["m1", "m2", "m3"].into_iter().map(|name| shape.pin(PinSpec::discrete(name, role::PORT, &BIT))).collect();
        let out = shape.pin(PinSpec::continuous("out", role::PORT));
        let branch = shape.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = shape.term(Shapes::flow("i", out, GROUND));
        let t1 = shape.term(Shapes::flow("eq", branch, GROUND));
        shape.function(FunctionSpec {
            name: "gate",
            origin: ORIGIN,
            reads: vec![Shapes::pin(code), Shapes::pin(modes[0]), Shapes::pin(modes[1]), Shapes::pin(modes[2]), Shapes::pin(out), Shapes::pin(branch), Shapes::pin(vref)],
            writes: vec![Shapes::term(t0), Shapes::term(t1)],
            partials: vec![Shapes::partial(0, 5), Shapes::partial(1, 4), Shapes::partial(1, 6)],
            ..FunctionSpec::default()
        });
        shape.freeze()
    }

    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        GATE.get_or_init(|| module::<Gate>("Gate"))
    }
}

impl Device for Gate {
    fn prototype() -> &'static Shape<'static> {
        GATE_SHAPE.get_or_init(Gate::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gate, Arc::clone(GATE_SHAPE.get_or_init(Gate::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let bit = |index: usize| cx.bytes(index)[0] as f64;
        let (code, modes) = (bit(0), bit(1) + bit(2) + bit(3));
        let (out, branch, vref) = (cx.f64(4), cx.f64(5), cx.f64(6));
        cx.set(0, branch);
        cx.set(1, out - (code * vref + modes));
        cx.jac().copy_from_slice(&[1.0, 1.0, -code]);
        Ok(())
    }
}

/// Sets `out` HIGH in the settle phase of every time point: a settle commit (§15 S1), read by the
/// step that arrives at the next time point.
/// pins: settle=0 (GLOBAL SETTLE event); out=1 (PORT BIT, init LOW)   reaction 0: on [settle] writes [S0 {out}]
pub struct SettleSetter;
static SETTLE_SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_SETTER: OnceLock<Module> = OnceLock::new();

impl SettleSetter {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let settle = shape.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        let out = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        shape.reaction(ReactionSpec {
            name: "set",
            origin: ORIGIN,
            on: vec![settle],
            writes: vec![Statement { pin: out, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        shape.freeze()
    }

    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SETTLE_SETTER.get_or_init(|| module::<SettleSetter>("SettleSetter"))
    }
}

impl Device for SettleSetter {
    fn prototype() -> &'static Shape<'static> {
        SETTLE_SETTER_SHAPE.get_or_init(SettleSetter::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleSetter, Arc::clone(SETTLE_SETTER_SHAPE.get_or_init(SettleSetter::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// Copies `src` to `dst` in the settle phase of every time point, as a settle commit: it writes
/// the final discrete state of the instant.
/// pins: settle=0 (GLOBAL SETTLE event); src=1 (PORT BIT); dst=2 (PORT BIT, init LOW)
/// reaction 0: on [settle] reads [src] writes [S0 {dst}]
pub struct SettleMirror;
static SETTLE_MIRROR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTLE_MIRROR: OnceLock<Module> = OnceLock::new();

impl SettleMirror {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let settle = shape.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
        let src = shape.pin(PinSpec::discrete("src", role::PORT, &BIT));
        let dst = shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("dst", role::PORT, &BIT) });
        shape.reaction(ReactionSpec {
            name: "copy",
            origin: ORIGIN,
            on: vec![settle],
            reads: vec![Shapes::pin(src)],
            writes: vec![Statement { pin: dst, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        shape.freeze()
    }

    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SETTLE_MIRROR.get_or_init(|| module::<SettleMirror>("SettleMirror"))
    }
}

impl Device for SettleMirror {
    fn prototype() -> &'static Shape<'static> {
        SETTLE_MIRROR_SHAPE.get_or_init(SettleMirror::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SettleMirror, Arc::clone(SETTLE_MIRROR_SHAPE.get_or_init(SettleMirror::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let value = [cx.bytes(0)[0]];
        cx.commit(0, &value, 0.0)?;
        Ok(Flow::Continue)
    }
}