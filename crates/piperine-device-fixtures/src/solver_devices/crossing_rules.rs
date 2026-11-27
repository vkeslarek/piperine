//! The devices of the solver target `crossing_rules`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, GROUND, Module, Partial, Shape, Statement, cell, global, op, part, pin_flag, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "crossing_rules.rs";
const ONE_PS: [u8; 8] = 1.0e-12f64.to_le_bytes();

// ─── Devices ────────────────────────────────────────────────────────────────

/// A source of 0 V that a reaction on `START` raises to 1 V: the operating point's first
/// round solves 0 V, the instant after it commits `hi`, and the next round solves 1 V.
/// pins: p=0 (PORT continuous); f=1 (PRIVATE continuous); hi=2 (PRIVATE BIT, init LOW);
///       start=3 (GLOBAL START)
/// terms: T0 FLOW{p, GROUND} "i" = f, T1 FLOW{f, GROUND} "eq" = v(p) − (hi ? 1 : 0)
/// fn 0: reads [p, f, hi] writes [T0, T1] partials (0<-f) (1<-p)
/// reaction 0: on [start] writes [S0 {hi}]; hi <- HIGH, no delay
pub struct Riser;
static RISER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RISER: OnceLock<Module> = OnceLock::new();

impl Device for Riser {
    fn prototype() -> &'static Shape<'static> {
        RISER_SHAPE.get_or_init(CrossingRulesShapes::riser_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Riser, Arc::clone(RISER_SHAPE.get_or_init(CrossingRulesShapes::riser_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = if cx.bytes(2) == HIGH { 1.0 } else { 0.0 };
        cx.set(0, cx.f64(1));
        cx.set(1, cx.f64(0) - level);
        cx.jac().copy_from_slice(&[1.0, 1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// A comparator whose `ttol` and `etol` are its own params (abi.md §3 `Watch { ttol, etol }`).
/// pins: ttol=0, etol=1 (PARAM F64, 1 ps); inp=2, inn=3 (PORT continuous); up=4 (PRIVATE event)
/// watches: W0 CROSS "rise" -> up, tolerances from pins 0 and 1
/// fn 0: reads [inp, inn] writes [W0] partials (0<-0) (0<-1)
pub struct ToleranceWatch;
static TOL_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TOL: OnceLock<Module> = OnceLock::new();

impl Device for ToleranceWatch {
    fn prototype() -> &'static Shape<'static> {
        TOL_SHAPE.get_or_init(CrossingRulesShapes::tol_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ToleranceWatch, Arc::clone(TOL_SHAPE.get_or_init(CrossingRulesShapes::tol_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, cx.f64(0) - cx.f64(1));
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
}
impl Riser {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        RISER.get_or_init(|| module::<Riser>("Riser"))
    }
}

impl ToleranceWatch {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        TOL.get_or_init(|| module::<ToleranceWatch>("TolComparator"))
    }
}

/// The shapes the `crossing_rules` devices are built from.
pub struct CrossingRulesShapes;

impl CrossingRulesShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    pub fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The shape of `Riser`, built once and shared by every instance.
    pub fn riser_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let hi = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("hi", role::PRIVATE, &BIT) });
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let flow = |name, plus| TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus: GROUND, abstol: piperine_abi::raw::DEFAULT, max: piperine_abi::raw::DEFAULT };
        let t0 = s.term(flow("i", p));
        let t1 = s.term(flow("eq", f));
        let term = |index| Cell { kind: cell::TERM, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![CrossingRulesShapes::pin(p), CrossingRulesShapes::pin(f), CrossingRulesShapes::pin(hi)],
            writes: vec![term(t0), term(t1)],
            partials: vec![CrossingRulesShapes::partial(0, 1), CrossingRulesShapes::partial(1, 0)],
            ..FunctionSpec::default()
        });
        let st = Statement { pin: hi, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        s.reaction(ReactionSpec { name: "rise", origin: ORIGIN, on: vec![start], writes: vec![st], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The shape of `ToleranceWatch`, built once and shared by every instance.
    pub fn tol_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let param = |name, init| PinSpec { flags: pin_flag::HAS_INIT, init, ..PinSpec::discrete(name, role::PARAM, &F64) };
        let ttol = s.pin(param("ttol", &ONE_PS));
        let etol = s.pin(param("etol", &ONE_PS));
        let inp = s.pin(PinSpec::continuous("inp", role::PORT));
        let inn = s.pin(PinSpec::continuous("inn", role::PORT));
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let w0 = s.watch(WatchSpec { name: "rise", origin: ORIGIN, kind: watch::CROSS, layout: &F64, event: up, ttol, etol });
        s.function(FunctionSpec {
            name: "e",
            origin: ORIGIN,
            reads: vec![CrossingRulesShapes::pin(inp), CrossingRulesShapes::pin(inn)],
            writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
            partials: vec![CrossingRulesShapes::partial(0, 0), CrossingRulesShapes::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
