//! The devices of the solver target `abi_dc`, one type per device.

use std::result::Result;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

// ---------------------------------------------------------------------------
// The gmin-stepping continuation reaches a device through the `GMIN` input.
// ---------------------------------------------------------------------------

/// Drives the ambient `GMIN` (in amperes per siemens) into its node.
/// pins: a=0 (PORT); terms: T0 FLOW{a, GROUND} "i" = -GMIN
/// fn 0: reads [GMIN] writes [T0] (no partial: the term reads no unknown)
pub struct GminProbe;
static GMIN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GMIN_PROBE: OnceLock<Module> = OnceLock::new();

impl Device for GminProbe {
    fn prototype() -> &'static Shape<'static> {
        GMIN_SHAPE.get_or_init(AbiDcShapes::gmin_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((GminProbe, Arc::clone(GMIN_SHAPE.get_or_init(AbiDcShapes::gmin_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let gmin = cx.f64(0);
        cx.set(0, -gmin);
        Ok(())
    }
}
impl GminProbe {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        GMIN_PROBE.get_or_init(|| module::<GminProbe>("GminProbe"))
    }
}

/// The shapes the `abi_dc` devices are built from.
pub struct AbiDcShapes;

impl AbiDcShapes {
    /// The shape of `GminProbe`, built once and shared by every instance.
    pub fn gmin_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: "abi_dc.rs", op: op::FLOW, plus: a, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: "abi_dc.rs",
            reads: vec![Cell { kind: cell::AMBIENT, index: ambient::GMIN, part: part::VALUE }],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
