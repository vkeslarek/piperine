//! The devices of the solver target `abi_noise`, one type per device.

use std::result::Result;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

/// A resistor whose branch carries two noise sources: a thermal one of density
/// `4kT/r` (T = 300 K) and a flicker one of `1e-24`.
/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT)
/// inputs: I0 NOISE "thermal", I1 NOISE "flicker"; terms: T0 FLOW{p, n}
/// fn 0 "i": reads [p, n, r, I0, I1] writes [T0] partials against all five
/// fn 1 "thermal": reads [r] writes [I0.PSD]; fn 2 "flicker": reads [] writes [I1.PSD]
pub struct NoisyR;
static NOISY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static NOISY: OnceLock<Module> = OnceLock::new();

impl Device for NoisyR {
    fn prototype() -> &'static Shape<'static> {
        NOISY_SHAPE.get_or_init(AbiNoiseShapes::noisy_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NoisyR, Arc::clone(NOISY_SHAPE.get_or_init(AbiNoiseShapes::noisy_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        match k {
            0 => {
                let (vp, vn, r, thermal, flicker) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4));
                let g = 1.0 / r;
                cx.set(0, g * vp - g * vn + thermal + flicker);
                cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r), 1.0, 1.0]);
            }
            1 => cx.set(0, 4.0 * 1.380649e-23 * 300.0 / cx.f64(0)),
            _ => cx.set(0, 1.0e-24_f64),
        }
        Ok(())
    }
}
impl NoisyR {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        NOISY.get_or_init(|| module::<NoisyR>("NoisyR"))
    }
}

/// The shapes the `abi_noise` devices are built from.
pub struct AbiNoiseShapes;

impl AbiNoiseShapes {
    /// The cell of kind `kind` and index `index`.
    pub fn cell_of(kind: u32, index: u32, part: u32) -> Cell {
        Cell { kind, index, part }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    pub fn partial(read: u32) -> Partial {
        Partial { write: 0, to: 0, read, at: 0 }
    }

    /// The shape of `NoisyR`, built once and shared by every instance.
    pub fn noisy_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &1.0e3_f64.to_le_bytes(), ..PinSpec::discrete("r", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let i0 = s.input(InputSpec { name: "thermal", origin: "abi_noise.rs", kind: input::NOISE });
        let i1 = s.input(InputSpec { name: "flicker", origin: "abi_noise.rs", kind: input::NOISE });
        let t0 = s.term(TermSpec { name: "i", origin: "abi_noise.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: "abi_noise.rs",
            reads: vec![
                AbiNoiseShapes::cell_of(cell::PIN, p, part::VALUE),
                AbiNoiseShapes::cell_of(cell::PIN, n, part::VALUE),
                AbiNoiseShapes::cell_of(cell::PIN, 0, part::VALUE),
                AbiNoiseShapes::cell_of(cell::INPUT, i0, part::VALUE),
                AbiNoiseShapes::cell_of(cell::INPUT, i1, part::VALUE),
            ],
            writes: vec![AbiNoiseShapes::cell_of(cell::TERM, t0, part::VALUE)],
            partials: (0..5).map(AbiNoiseShapes::partial).collect(),
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "thermal",
            origin: "abi_noise.rs",
            reads: vec![AbiNoiseShapes::cell_of(cell::PIN, 0, part::VALUE)],
            writes: vec![AbiNoiseShapes::cell_of(cell::INPUT, i0, part::PSD)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "flicker",
            origin: "abi_noise.rs",
            reads: vec![],
            writes: vec![AbiNoiseShapes::cell_of(cell::INPUT, i1, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
