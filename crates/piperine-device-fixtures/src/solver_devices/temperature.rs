//! The devices of the solver target `temperature`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, cell, global, op, part, role};
use piperine_abi::layout::F64;
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "temperature.rs";

/// A current of `T · 1 µA` drawn out of `p` into `n`, `T` the `TEMPERATURE`
/// global.
/// pins: t=0 (GLOBAL TEMPERATURE F64); p=1, n=2 (PORT)
/// terms: T0 FLOW{p, n}; fn 0: reads [t] writes [T0], no partial (constant in the unknowns)
pub struct Thermo;
static THERMO_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static THERMO: OnceLock<Module> = OnceLock::new();

impl Device for Thermo {
    fn prototype() -> &'static Shape<'static> {
        THERMO_SHAPE.get_or_init(TemperatureShapes::thermo_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Thermo, Arc::clone(THERMO_SHAPE.get_or_init(TemperatureShapes::thermo_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let t = cx.f64(0);
        cx.set(0, -t * 1.0e-6);
        Ok(())
    }
}
impl Thermo {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        THERMO.get_or_init(|| module::<Thermo>("Thermo"))
    }
}

/// The shapes the `temperature` devices are built from.
pub struct TemperatureShapes;

impl TemperatureShapes {
    /// The shape of `Thermo`, built once and shared by every instance.
    pub fn thermo_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("t", role::GLOBAL, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![Cell { kind: cell::PIN, index: 0, part: part::VALUE }],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
