//! The devices of the solver target `protocol_crossings`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, Module, Partial, Shape, Statement, cell, part, pin_flag, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, WatchSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "protocol_crossings.rs";
const ONE_PS: [u8; 8] = 1.0e-12f64.to_le_bytes();
const ONE_US: [u8; 8] = 1.0e-6f64.to_le_bytes();
const VTH: [u8; 8] = 0.999f64.to_le_bytes();

/// A comparator whose `ttol` and `etol` are its own params (abi.md §3 `Watch { ttol, etol }`).
/// pins: ttol=0, etol=1 (PARAM F64); inp=2, inn=3 (PORT continuous); out=4 (PORT BIT, init LOW);
///       up=5 (PRIVATE event)
/// watches: W0 CROSS "rise" -> up, tolerances from pins 0 and 1
/// fn 0: reads [inp, inn] writes [W0] partials (0<-0) (0<-1);  reaction 0: on [up] out <- HIGH
pub struct ToleranceComparator;
static TOL_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TOL: OnceLock<Module> = OnceLock::new();

impl Device for ToleranceComparator {
    fn prototype() -> &'static Shape<'static> {
        TOL_SHAPE.get_or_init(ProtocolCrossingsShapes::tol_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ToleranceComparator, Arc::clone(TOL_SHAPE.get_or_init(ProtocolCrossingsShapes::tol_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, cx.f64(0) - cx.f64(1));
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// A `CROSS` watch on a curved function of time: `e = 1 − exp(−t/tau) − vth`, concave, so the
/// secant of a bracket always lands past the crossing.
/// pins: tau=0, vth=1 (PARAM F64); up=2 (PRIVATE event)
/// watches: W0 CROSS "rise" -> up;  fn 0: reads [TIME, tau, vth] writes [W0]
pub struct Saturating;
static SAT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SAT: OnceLock<Module> = OnceLock::new();

impl Device for Saturating {
    fn prototype() -> &'static Shape<'static> {
        SAT_SHAPE.get_or_init(ProtocolCrossingsShapes::saturating_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Saturating, Arc::clone(SAT_SHAPE.get_or_init(ProtocolCrossingsShapes::saturating_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (t, tau, vth) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, 1.0 - (-t / tau).exp() - vth);
        Ok(())
    }
}
impl Saturating {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SAT.get_or_init(|| module::<Saturating>("Saturating"))
    }
}

impl ToleranceComparator {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        TOL.get_or_init(|| module::<ToleranceComparator>("TolComparator"))
    }
}

/// The shapes the `protocol_crossings` devices are built from.
pub struct ProtocolCrossingsShapes;

impl ProtocolCrossingsShapes {
    /// The shape of `ToleranceComparator`, built once and shared by every instance.
    pub fn tol_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let param = |name, init| PinSpec { flags: pin_flag::HAS_INIT, init, ..PinSpec::discrete(name, role::PARAM, &F64) };
        let ttol = s.pin(param("ttol", &ONE_PS));
        let etol = s.pin(param("etol", &ONE_PS));
        let inp = s.pin(PinSpec::continuous("inp", role::PORT));
        let inn = s.pin(PinSpec::continuous("inn", role::PORT));
        let out = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("out", role::PORT, &BIT) });
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let w0 = s.watch(WatchSpec { name: "rise", origin: ORIGIN, kind: watch::CROSS, layout: &F64, event: up, ttol, etol });
        let cell = |kind, index| Cell { kind, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "e",
            origin: ORIGIN,
            reads: vec![cell(cell::PIN, inp), cell(cell::PIN, inn)],
            writes: vec![cell(cell::WATCH, w0)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        let st = Statement { pin: out, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        s.reaction(ReactionSpec { name: "up", origin: ORIGIN, on: vec![up], writes: vec![st], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The shape of `Saturating`, built once and shared by every instance.
    pub fn saturating_shape() -> ShapeRef {
        use piperine_abi::raw::{DEFAULT, ambient};
        let mut s = ShapeBuf::new();
        let param = |name, init| PinSpec { flags: pin_flag::HAS_INIT, init, ..PinSpec::discrete(name, role::PARAM, &F64) };
        let tau = s.pin(param("tau", &ONE_US));
        let vth = s.pin(param("vth", &VTH));
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let w0 = s.watch(WatchSpec { name: "rise", origin: ORIGIN, kind: watch::CROSS, layout: &F64, event: up, ttol: DEFAULT, etol: DEFAULT });
        s.function(FunctionSpec {
            name: "e",
            origin: ORIGIN,
            reads: vec![
                Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE },
                Cell { kind: cell::PIN, index: tau, part: part::VALUE },
                Cell { kind: cell::PIN, index: vth, part: part::VALUE },
            ],
            writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
