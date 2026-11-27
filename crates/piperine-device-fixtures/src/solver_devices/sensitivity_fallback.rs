//! The devices of the solver target `sensitivity_fallback`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "sensitivity_fallback.rs";
const G_DEFAULT: [u8; 8] = 1.0e-3_f64.to_le_bytes();

/// A conductance that declares its partial against the param `g`: the sensitivity takes the analytic path.
pub struct Gan;
static GAN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GAN_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Gan {
    fn prototype() -> &'static Shape<'static> {
        GAN_SHAPE.get_or_init(|| SensitivityFallbackShapes::conductor_shape(true)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gan, Arc::clone(GAN_SHAPE.get_or_init(|| SensitivityFallbackShapes::conductor_shape(true)))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        SensitivityFallbackShapes::conductor_eval(cx, k);
        Ok(())
    }
}

/// A conductance that declares no partial against the param `g`: the sensitivity falls back to a finite difference.
pub struct Gfb;
static GFB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GFB_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Gfb {
    fn prototype() -> &'static Shape<'static> {
        GFB_SHAPE.get_or_init(|| SensitivityFallbackShapes::conductor_shape(false)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gfb, Arc::clone(GFB_SHAPE.get_or_init(|| SensitivityFallbackShapes::conductor_shape(false)))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        SensitivityFallbackShapes::conductor_eval(cx, k);
        Ok(())
    }
}
impl Gan {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        GAN_MODULE.get_or_init(|| module::<Gan>("Gan"))
    }
}

impl Gfb {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        GFB_MODULE.get_or_init(|| module::<Gfb>("Gfb"))
    }
}

/// The shapes the `sensitivity_fallback` devices are built from.
pub struct SensitivityFallbackShapes;

impl SensitivityFallbackShapes {
    /// The cell of kind `kind` and index `index`.
    pub fn cell_of(kind: u32, index: u32) -> Cell {
        Cell { kind, index, part: part::VALUE }
    }

    /// A parametric conductor `I = g·V(p, n)` that shows the `g` it reads as a
    /// `VAR` watch. `Gan` declares the partial against its `g` param (the declared
    /// route); `Gfb` declares only the partials against its ports, so its `∂F/∂g`
    /// takes the stamp-perturbation fallback.
    ///
    /// pins: g=0 (PARAM F64, 1e-3); p=1, n=2 (PORT)
    /// terms: T0 FLOW{p, n}; watches: W0 VAR "g"
    /// fn 0 "i": reads [p, n, g] writes [T0] partials (0<-p) (0<-n) [(0<-g) when declared]
    /// fn 1 "show": reads [g] writes [W0]
    pub fn conductor_shape(declared: bool) -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &G_DEFAULT, ..PinSpec::discrete("g", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let w0 = s.watch(WatchSpec { name: "g", origin: ORIGIN, kind: watch::VAR, layout: &F64, event: DEFAULT, ttol: DEFAULT, etol: DEFAULT });
        let mut partials = vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }];
        if declared {
            partials.push(Partial { write: 0, to: 0, read: 2, at: 0 });
        }
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![SensitivityFallbackShapes::cell_of(cell::PIN, p), SensitivityFallbackShapes::cell_of(cell::PIN, n), SensitivityFallbackShapes::cell_of(cell::PIN, 0)],
            writes: vec![SensitivityFallbackShapes::cell_of(cell::TERM, t0)],
            partials,
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "show",
            origin: ORIGIN,
            reads: vec![SensitivityFallbackShapes::cell_of(cell::PIN, 0)],
            writes: vec![SensitivityFallbackShapes::cell_of(cell::WATCH, w0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The value and partials of the conductor function `k`: `k` 1 shows the param, anything else is the current `g * V(p, n)` with its partials.
    pub fn conductor_eval(cx: &mut Eval, k: u32) {
        if k == 1 {
            let g = cx.f64(0);
            cx.set(0, g);
            return;
        }
        let (vp, vn, g) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, g * vp - g * vn);
        let jac = cx.jac();
        jac[0] = g;
        jac[1] = -g;
        if jac.len() > 2 {
            // d(g·V)/dg = V(p, n): the declared derivative.
            jac[2] = vp - vn;
        }
    }
}
