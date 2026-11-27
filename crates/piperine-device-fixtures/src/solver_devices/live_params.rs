//! The devices of the solver target `live_params`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

/// A device whose shape declares no param: one port and nothing else.
pub struct Mute;
static MUTE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static MUTE: OnceLock<Module> = OnceLock::new();

impl Device for Mute {
    fn prototype() -> &'static Shape<'static> {
        MUTE_SHAPE.get_or_init(LiveParamsShapes::mute_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Mute, Arc::clone(MUTE_SHAPE.get_or_init(LiveParamsShapes::mute_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
}
impl Mute {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        MUTE.get_or_init(|| module::<Mute>("Mute"))
    }
}

/// The shapes the `live_params` devices are built from.
pub struct LiveParamsShapes;

impl LiveParamsShapes {
    /// The shape of `Mute`, built once and shared by every instance.
    pub fn mute_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec::continuous("p", role::PORT));
        s.freeze()
    }
}
