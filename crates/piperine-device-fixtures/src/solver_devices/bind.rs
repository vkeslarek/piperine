//! The devices of the solver target `bind`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::{BYTE, F64};
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, cell, global, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const R_DEFAULT: [u8; 8] = 1.0e3_f64.to_le_bytes();

/// pins: r=0 (PARAM F64, default 1e3); a=1 (PORT, abstol 1e-9); b=2 (PORT, abstol 4e-10);
/// code=3, other=4 (PORT BYTE); temp=5 (GLOBAL TEMPERATURE); k=6 (GLOBAL NAMED);
/// i=7 (PRIVATE continuous)
/// terms: T0 FLOW{a, b}
/// fn 0 "i": reads [a, b, r, code, temp, k, i] writes [T0]   (`other` is read by no function)
pub struct BoundValuesProbe;
static PROBE: OnceLock<ShapeRef> = OnceLock::new();
static MODULE: OnceLock<Module> = OnceLock::new();

impl Device for BoundValuesProbe {
    fn prototype() -> &'static Shape<'static> {
        PROBE.get_or_init(BindShapes::probe_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((BoundValuesProbe, Arc::clone(PROBE.get_or_init(BindShapes::probe_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, 0.0_f64);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// One create per placement
// ---------------------------------------------------------------------------

/// The `create` calls `CreateCounter` has received in this process.
pub static CREATES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// pins: a=0 (PORT continuous), nothing else; counts its `create` calls.
pub struct CreateCounter;
static COUNTING_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COUNTING_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for CreateCounter {
    fn prototype() -> &'static Shape<'static> {
        COUNTING_SHAPE.get_or_init(BindShapes::counting_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        CREATES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok((CreateCounter, Arc::clone(COUNTING_SHAPE.get_or_init(BindShapes::counting_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
}
impl CreateCounter {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        COUNTING_MODULE.get_or_init(|| module::<CreateCounter>("counting"))
    }
}

impl BoundValuesProbe {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        MODULE.get_or_init(|| module::<BoundValuesProbe>("probe"))
    }
}

/// The shapes the `bind` devices are built from.
pub struct BindShapes;

impl BindShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The shape of `BoundValuesProbe`, built once and shared by every instance.
    pub fn probe_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &R_DEFAULT, ..PinSpec::discrete("r", role::PARAM, &F64) });
        let a = s.pin(PinSpec { abstol: [1.0e-9, 0.0], ..PinSpec::continuous("a", role::PORT) });
        let b = s.pin(PinSpec { abstol: [4.0e-10, 0.0], ..PinSpec::continuous("b", role::PORT) });
        s.pin(PinSpec::discrete("code", role::PORT, &BYTE));
        s.pin(PinSpec::discrete("other", role::PORT, &BYTE));
        s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temp", role::GLOBAL, &F64) });
        s.pin(PinSpec { global: global::NAMED, ..PinSpec::discrete("k", role::GLOBAL, &F64) });
        s.pin(PinSpec::continuous("i", role::PRIVATE));
        let t0 = s.term(TermSpec { name: "i", origin: "bind.rs", op: op::FLOW, plus: a, minus: b, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            reads: vec![BindShapes::pin(a), BindShapes::pin(b), BindShapes::pin(0), BindShapes::pin(3), BindShapes::pin(5), BindShapes::pin(6), BindShapes::pin(7)],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `CreateCounter`, built once and shared by every instance.
    pub fn counting_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec::continuous("a", role::PORT));
        s.freeze()
    }
}
