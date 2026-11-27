//! The devices of the solver target `abi_tran`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, ambient, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const C_DEFAULT: [u8; 8] = 1.0e-6_f64.to_le_bytes();
const INITIAL_VOLTAGE: f64 = 3.0;
const V_INIT: [u8; 8] = INITIAL_VOLTAGE.to_le_bytes();
const FLAKY_AFTER: f64 = 8.0e-6;

// ---------------------------------------------------------------------------
// ChargedC: the capacitor of `@initial { V(p, n) <- 3.0 }`
// ---------------------------------------------------------------------------

/// pins: c=0 (PARAM F64, 1e-6); p=1 (PORT, init 3.0); n=2 (PORT)
/// terms: T0 DDT{p, n}; fn 0: reads [p, n, c] writes [T0]
pub struct ChargedC;
static CHARGED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CHARGED_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for ChargedC {
    fn prototype() -> &'static Shape<'static> {
        CHARGED_SHAPE.get_or_init(AbiTranShapes::charged_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ChargedC, Arc::clone(CHARGED_SHAPE.get_or_init(AbiTranShapes::charged_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let c = cx.f64(2);
        cx.set(0, c * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[c, -c]);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FlakyC: the same capacitor, whose function faults late in time
// ---------------------------------------------------------------------------

/// `ChargedC` plus a read of `TIME`: the function returns a `FAULT` once the
/// time passes `FLAKY_AFTER`, so a step's second stage fails after its first
/// converged and pushed a row of every history.
/// fn 0: reads [p, n, c, TIME] writes [T0]
pub struct FlakyC;
static FLAKY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAKY_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for FlakyC {
    fn prototype() -> &'static Shape<'static> {
        FLAKY_SHAPE.get_or_init(AbiTranShapes::flaky_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlakyC, Arc::clone(FLAKY_SHAPE.get_or_init(AbiTranShapes::flaky_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.f64(3) > FLAKY_AFTER {
            return Err(Fault::new(7, "late"));
        }
        let c = cx.f64(2);
        cx.set(0, c * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[c, -c]);
        Ok(())
    }
}
impl ChargedC {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        CHARGED_MODULE.get_or_init(|| module::<ChargedC>("ChargedC"))
    }
}

impl FlakyC {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        FLAKY_MODULE.get_or_init(|| module::<FlakyC>("FlakyC"))
    }
}

/// The shapes the `abi_tran` devices are built from.
pub struct AbiTranShapes;

impl AbiTranShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of the value of term `index`.
    pub fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The term spec named `name` of operator `operator`, between pins `plus` and `minus`.
    pub fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: "abi_tran.rs", op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The shape of `ChargedC`, built once and shared by every instance.
    pub fn charged_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &C_DEFAULT, ..PinSpec::discrete("c", role::PARAM, &F64) });
        let p = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &V_INIT, ..PinSpec::continuous("p", role::PORT) });
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(AbiTranShapes::flow("q", op::DDT, p, n));
        s.function(FunctionSpec {
            name: "q",
            origin: "abi_tran.rs",
            reads: vec![AbiTranShapes::pin(p), AbiTranShapes::pin(n), AbiTranShapes::pin(0)],
            writes: vec![AbiTranShapes::term(t0)],
            partials: vec![
                piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
                piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `FlakyC`, built once and shared by every instance.
    pub fn flaky_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &C_DEFAULT, ..PinSpec::discrete("c", role::PARAM, &F64) });
        let p = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &V_INIT, ..PinSpec::continuous("p", role::PORT) });
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(AbiTranShapes::flow("q", op::DDT, p, n));
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        s.function(FunctionSpec {
            name: "q",
            origin: "abi_tran.rs",
            reads: vec![AbiTranShapes::pin(p), AbiTranShapes::pin(n), AbiTranShapes::pin(0), time],
            writes: vec![AbiTranShapes::term(t0)],
            partials: vec![
                piperine_abi::raw::Partial { write: 0, to: 0, read: 0, at: 0 },
                piperine_abi::raw::Partial { write: 0, to: 0, read: 1, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
