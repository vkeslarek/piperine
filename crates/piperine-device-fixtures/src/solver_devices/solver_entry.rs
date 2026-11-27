//! The devices of the solver target `solver_entry`, one type per device.

use std::result::Result;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

/// A device whose creation fails: the error reaches the host at build.
pub struct BadElement;
static BAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static BAD: OnceLock<Module> = OnceLock::new();

impl Device for BadElement {
    fn prototype() -> &'static Shape<'static> {
        BAD_SHAPE.get_or_init(SolverEntryShapes::bad_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let _ = Arc::clone(BAD_SHAPE.get_or_init(SolverEntryShapes::bad_shape));
        Err(Fault::new(1, "setup error"))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
}
impl BadElement {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        BAD.get_or_init(|| module::<BadElement>("BadElement"))
    }
}

/// The shapes the `solver_entry` devices are built from.
pub struct SolverEntryShapes;

impl SolverEntryShapes {
    /// The shape of `BadElement`, built once and shared by every instance.
    pub fn bad_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec::continuous("p", role::PORT));
        s.freeze()
    }
}
