//! The devices of the solver target `abi_sensitivity`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const R_DEFAULT: [u8; 8] = 1.0e3_f64.to_le_bytes();

// ---------------------------------------------------------------------------
// Gd: a conductance derived from a param by a discrete function
// ---------------------------------------------------------------------------

/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT); g=3 (PRIVATE discrete F64)
/// terms: T0 FLOW{p, n}
/// fn 0 "g" (discrete): reads [r] writes [g] partials (0<-0) = -1/r^2
/// fn 1 "i" (analog): reads [p, n, g] writes [T0] partials (0<-p) = g, (0<-n) = -g, (0<-g) = vp - vn
pub struct Gd;
static GD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GD_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Gd {
    fn prototype() -> &'static Shape<'static> {
        GD_SHAPE.get_or_init(AbiSensitivityShapes::gd_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gd, Arc::clone(GD_SHAPE.get_or_init(AbiSensitivityShapes::gd_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let r = cx.f64(0);
            cx.set(0, 1.0 / r);
            cx.jac().copy_from_slice(&[-1.0 / (r * r)]);
        } else {
            let (vp, vn, g) = (cx.f64(0), cx.f64(1), cx.f64(2));
            cx.set(0, g * vp - g * vn);
            cx.jac().copy_from_slice(&[g, -g, vp - vn]);
        }
        Ok(())
    }
}
impl Gd {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        GD_MODULE.get_or_init(|| module::<Gd>("Gd"))
    }
}

/// The shapes the `abi_sensitivity` devices are built from.
pub struct AbiSensitivityShapes;

impl AbiSensitivityShapes {
    /// The cell of kind `kind` and index `index`.
    pub fn cell_of(kind: u32, index: u32) -> Cell {
        Cell { kind, index, part: part::VALUE }
    }

    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        AbiSensitivityShapes::cell_of(cell::PIN, index)
    }

    /// The shape of `Gd`, built once and shared by every instance.
    pub fn gd_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &R_DEFAULT, ..PinSpec::discrete("r", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let g = s.pin(PinSpec::discrete("g", role::PRIVATE, &F64));
        let t0 = s.term(TermSpec { name: "i", origin: "abi_sens.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "g",
            origin: "abi_sens.rs",
            reads: vec![AbiSensitivityShapes::pin(0)],
            writes: vec![AbiSensitivityShapes::pin(g)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "i",
            origin: "abi_sens.rs",
            reads: vec![AbiSensitivityShapes::pin(p), AbiSensitivityShapes::pin(n), AbiSensitivityShapes::pin(g)],
            writes: vec![AbiSensitivityShapes::cell_of(cell::TERM, t0)],
            partials: vec![
                Partial { write: 0, to: 0, read: 0, at: 0 },
                Partial { write: 0, to: 0, read: 1, at: 0 },
                Partial { write: 0, to: 0, read: 2, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
