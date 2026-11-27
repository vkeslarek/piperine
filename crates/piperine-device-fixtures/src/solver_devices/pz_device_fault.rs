//! The devices of the solver target `pz_device_fault`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

/// A conductance to ground that faults once `FREQ` is non-zero (an AC probe).
pub struct AcFault;
static SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static MODULE: OnceLock<Module> = OnceLock::new();

impl Device for AcFault {
    fn prototype() -> &'static Shape<'static> {
        SHAPE.get_or_init(PzDeviceFaultShapes::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AcFault, Arc::clone(SHAPE.get_or_init(PzDeviceFaultShapes::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.f64(1) != 0.0 {
            return Err(Fault::new(9, "no small-signal model"));
        }
        cx.set(0, 1.0e-3 * cx.f64(0));
        cx.jac().copy_from_slice(&[1.0e-3]);
        Ok(())
    }
}
impl AcFault {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        MODULE.get_or_init(|| module::<AcFault>("AcFault"))
    }
}

/// The shapes the `pz_device_fault` devices are built from.
pub struct PzDeviceFaultShapes;

impl PzDeviceFaultShapes {
    /// The shape of the shape fixture, built once and shared by every instance.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let _ = F64;
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: "pz_device_fault.rs", op: op::FLOW, plus: p, minus: piperine_abi::raw::GROUND, abstol: DEFAULT, max: DEFAULT });
        let pin = Cell { kind: cell::PIN, index: p, part: part::VALUE };
        let freq = Cell { kind: cell::AMBIENT, index: ambient::FREQ, part: part::VALUE };
        s.function(FunctionSpec {
            name: "i",
            origin: "pz_device_fault.rs",
            reads: vec![pin, freq],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
