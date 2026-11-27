//! The devices of the solver target `truncation`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, ambient, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const ONE_NANO: [u8; 8] = 1.0e-9_f64.to_le_bytes();
const STEP_AT: [u8; 8] = 1.03e-6_f64.to_le_bytes();

/// A capacitor whose charge is `q = c0·v + c1·v³` (`v = V(p, n)`).
/// pins: c0=0, c1=1 (PARAM F64); p=2, n=3 (PORT)      terms: T0 DDT{p, n} "q"
/// fn 0 "q": reads [p, n, c0, c1]  writes [T0]  partials (0←0) (0←1)
pub struct CubicCapacitor;
static CUBIC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CUBIC_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for CubicCapacitor {
    fn prototype() -> &'static Shape<'static> {
        CUBIC_SHAPE.get_or_init(TruncationShapes::cubic_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CubicCapacitor, Arc::clone(CUBIC_SHAPE.get_or_init(TruncationShapes::cubic_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c0, c1) = (cx.f64(0) - cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, c0 * v + c1 * v * v * v);
        let g = c0 + 3.0 * c1 * v * v;
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}

/// A voltage source that goes from 0 to 1 V at the time `at`, computed from `TIME` (nothing
/// announces the corner to the solver): a hard step when `width` is 0, else `½(1 + tanh((t − at)/width))`.
/// Pins: at=0, width=1 (PARAM F64); p=2, n=3 (PORT); f=4 (PRIVATE).
/// fn 0: reads [p, n, f, at, width, TIME] writes [T0, T1] partials (0←f) (1←p) (1←n)
pub struct StepSource;
static STEP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STEP_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for StepSource {
    fn prototype() -> &'static Shape<'static> {
        STEP_SHAPE.get_or_init(TruncationShapes::step_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((StepSource, Arc::clone(STEP_SHAPE.get_or_init(TruncationShapes::step_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, at, width, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4), cx.f64(5));
        let wanted = if width > 0.0 { 0.5 * (1.0 + ((t - at) / width).tanh()) } else if t >= at { 1.0 } else { 0.0 };
        cx.set(0, f);
        cx.set(1, vp - vn - wanted);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
}

/// A conductance of 1 mS that faults the first time it is evaluated after `FAULT_AFTER`: one
/// Newton failure, at whatever step reaches it first. `FAULTED` says it happened.
/// pins: p=0, n=1 (PORT); fn 0: reads [p, n, TIME] writes [T0] partials (0<-0) (0<-1)
pub struct FaultOnce;
static FAULT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FAULT_MODULE: OnceLock<Module> = OnceLock::new();
const FAULT_AFTER: f64 = 5.0e-7;

thread_local! {
    /// Whether `FaultOnce` has already faulted in this thread.
    pub static FAULTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Device for FaultOnce {
    fn prototype() -> &'static Shape<'static> {
        FAULT_SHAPE.get_or_init(TruncationShapes::fault_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FaultOnce, Arc::clone(FAULT_SHAPE.get_or_init(TruncationShapes::fault_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.f64(2) > FAULT_AFTER && !FAULTED.with(|faulted| faulted.replace(true)) {
            return Err(Fault::new(7, "once"));
        }
        let g = 1.0e-3;
        cx.set(0, g * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}
impl CubicCapacitor {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        CUBIC_MODULE.get_or_init(|| module::<CubicCapacitor>("CubicCapacitor"))
    }
}

impl FaultOnce {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        FAULT_MODULE.get_or_init(|| module::<FaultOnce>("FaultOnce"))
    }
}

impl StepSource {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        STEP_MODULE.get_or_init(|| module::<StepSource>("StepSource"))
    }
}

/// The shapes the `truncation` devices are built from.
pub struct TruncationShapes;

impl TruncationShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of the value of term `index`.
    pub fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The `F64` param pin named `name` whose default is `default`.
    pub fn real_param(name: &'static str, default: &'static [u8; 8]) -> PinSpec<'static> {
        PinSpec { flags: pin_flag::HAS_INIT, init: default, ..PinSpec::discrete(name, role::PARAM, &F64) }
    }

    /// The shape of `CubicCapacitor`, built once and shared by every instance.
    pub fn cubic_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(TruncationShapes::real_param("c0", &ONE_NANO));
        s.pin(TruncationShapes::real_param("c1", &ONE_NANO));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "q", origin: "truncation.rs", op: op::DDT, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "q",
            origin: "truncation.rs",
            reads: vec![TruncationShapes::pin(p), TruncationShapes::pin(n), TruncationShapes::pin(0), TruncationShapes::pin(1)],
            writes: vec![TruncationShapes::term(t0)],
            partials: vec![
                piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
                piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `StepSource`, built once and shared by every instance.
    pub fn step_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(TruncationShapes::real_param("at", &STEP_AT));
        s.pin(TruncationShapes::real_param("width", &[0; 8]));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let flow = |name, plus, minus| TermSpec { name, origin: "truncation.rs", op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT };
        let t0 = s.term(flow("i", p, n));
        let t1 = s.term(flow("eq", f, GROUND));
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        s.function(FunctionSpec {
            name: "v",
            origin: "truncation.rs",
            reads: vec![TruncationShapes::pin(p), TruncationShapes::pin(n), TruncationShapes::pin(f), TruncationShapes::pin(0), TruncationShapes::pin(1), time],
            writes: vec![TruncationShapes::term(t0), TruncationShapes::term(t1)],
            partials: vec![
                piperine_abi::raw::Partial { write: 0, to: 0, read: 2, at: 0 },
                piperine_abi::raw::Partial { write: 1, to: 0, read: 0, at: 0 },
                piperine_abi::raw::Partial { write: 1, to: 0, read: 1, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `FaultOnce`, built once and shared by every instance.
    pub fn fault_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: "truncation.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        s.function(FunctionSpec {
            name: "i",
            origin: "truncation.rs",
            reads: vec![TruncationShapes::pin(p), TruncationShapes::pin(n), time],
            writes: vec![TruncationShapes::term(t0)],
            partials: vec![piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 }, piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
