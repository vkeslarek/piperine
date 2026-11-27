//! The devices of the solver target `protocol_transient`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::BIT;
use piperine_abi::raw::{Cell, Module, Shape, cell, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

/// A function that never settles on its own net: it writes the inverse of the level it reads.
/// Bound with `a` and `y` on one `RELAX` net it relaxes forever.
///
/// pins: a=0 (PORT BIT); y=1 (PORT BIT, RELAX).  fn 0 "flip": reads [a] writes [y]
pub struct Flipper;
static FLIPPER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLIPPER: OnceLock<Module> = OnceLock::new();

impl Flipper {
    /// The shape every `Flipper` shares.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &BIT) });
        s.function(FunctionSpec {
            name: "flip",
            origin: "protocol_transient.rs",
            reads: vec![Cell { kind: cell::PIN, index: a, part: part::VALUE }],
            writes: vec![Cell { kind: cell::PIN, index: y, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Flipper {
    fn prototype() -> &'static Shape<'static> {
        FLIPPER_SHAPE.get_or_init(Flipper::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Flipper, Arc::clone(FLIPPER_SHAPE.get_or_init(Flipper::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = cx.bytes(0)[0] == 0;
        cx.out_bytes(0).copy_from_slice(&[level as u8]);
        Ok(())
    }
}
impl Flipper {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        FLIPPER.get_or_init(|| module::<Flipper>("Flipper"))
    }
}
