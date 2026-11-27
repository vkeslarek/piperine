//! The devices of the solver target `protocol_real`, one type per device.

use std::cell::Cell as StdCell;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

// ---------------------------------------------------------------------------
// A conductance whose function faults once, the first time it is evaluated
// inside `FAULT_WINDOW`: one Newton failure at a step that is then retried.
// ---------------------------------------------------------------------------

thread_local! {
    /// Whether `FlakyG` has already faulted in this thread.
    pub static FAULTED: StdCell<bool> = const { StdCell::new(false) };
}
const FAULT_WINDOW: (f64, f64) = (3.0e-4, 4.0e-4);

/// pins: p=0, n=1 (PORT); fn 0: reads [p, n, TIME] writes [T0] partials (0<-0) (0<-1); g = 1 mS
pub struct FlakyG;
static FLAKY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAKY_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for FlakyG {
    fn prototype() -> &'static Shape<'static> {
        FLAKY_SHAPE.get_or_init(ProtocolRealShapes::flaky_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlakyG, Arc::clone(FLAKY_SHAPE.get_or_init(ProtocolRealShapes::flaky_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let t = cx.f64(2);
        if t > FAULT_WINDOW.0 && t < FAULT_WINDOW.1 && !FAULTED.with(|f| f.replace(true)) {
            return Err(Fault::new(7, "once"));
        }
        let g = 1.0e-3;
        cx.set(0, g * cx.f64(0) - g * cx.f64(1));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}
impl FlakyG {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        FLAKY_MODULE.get_or_init(|| module::<FlakyG>("FlakyG"))
    }
}

/// The shapes the `protocol_real` devices are built from.
pub struct ProtocolRealShapes;

impl ProtocolRealShapes {
    /// The shape of `FlakyG`, built once and shared by every instance.
    pub fn flaky_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: "protocol_real.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "i",
            origin: "protocol_real.rs",
            reads: vec![pin(p), pin(n), Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
