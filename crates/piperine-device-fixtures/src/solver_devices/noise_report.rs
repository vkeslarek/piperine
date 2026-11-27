//! The devices of the solver target `noise_report`, one type per device.

use std::result::Result;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

/// The density constants of the two sources of [`TwinFlicker`]: `FIRST/f` and `SECOND/f` A²/Hz.
const FIRST: f64 = 1.0e-24;
const SECOND: f64 = 3.0e-24;

// ─── A resistor whose branch carries two flicker sources of one name ────────

/// Cells and partials of the hand-built shape of this target.
pub struct Cells;

impl Cells {
    fn at(kind: u32, index: u32, part: u32) -> Cell {
        Cell { kind, index, part }
    }

    fn partial(read: u32) -> Partial {
        Partial { write: 0, to: 0, read, at: 0 }
    }
}

/// A resistor `r` from `p` to `n` carrying two `NOISE` inputs both named `flicker`, of densities
/// `FIRST/f` and `SECOND/f`.
/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT)
/// inputs: I0 NOISE "flicker", I1 NOISE "flicker"; terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, r, I0, I1] writes [T0] partials against all five
/// fn 1 "first": reads [FREQ] writes [I0.PSD]; fn 2 "second": reads [FREQ] writes [I1.PSD]
pub struct TwinFlicker;
static TWIN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TWIN: OnceLock<Module> = OnceLock::new();

impl TwinFlicker {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &1.0e3_f64.to_le_bytes(), ..PinSpec::discrete("r", role::PARAM, &F64) });
        let p = shape.pin(PinSpec::continuous("p", role::PORT));
        let n = shape.pin(PinSpec::continuous("n", role::PORT));
        let i0 = shape.input(InputSpec { name: "flicker", origin: "noise_report.rs", kind: input::NOISE });
        let i1 = shape.input(InputSpec { name: "flicker", origin: "noise_report.rs", kind: input::NOISE });
        let t0 = shape.term(TermSpec { name: "i", origin: "noise_report.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        shape.function(FunctionSpec {
            name: "i",
            origin: "noise_report.rs",
            reads: vec![
                Cells::at(cell::PIN, p, part::VALUE),
                Cells::at(cell::PIN, n, part::VALUE),
                Cells::at(cell::PIN, 0, part::VALUE),
                Cells::at(cell::INPUT, i0, part::VALUE),
                Cells::at(cell::INPUT, i1, part::VALUE),
            ],
            writes: vec![Cells::at(cell::TERM, t0, part::VALUE)],
            partials: (0..5).map(Cells::partial).collect(),
            ..FunctionSpec::default()
        });
        for (name, input) in [("first", i0), ("second", i1)] {
            shape.function(FunctionSpec {
                name,
                origin: "noise_report.rs",
                reads: vec![Cells::at(cell::AMBIENT, ambient::FREQ, part::VALUE)],
                writes: vec![Cells::at(cell::INPUT, input, part::PSD)],
                ..FunctionSpec::default()
            });
        }
        shape.freeze()
    }
}

impl Device for TwinFlicker {
    fn prototype() -> &'static Shape<'static> {
        TWIN_SHAPE.get_or_init(TwinFlicker::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TwinFlicker, Arc::clone(TWIN_SHAPE.get_or_init(TwinFlicker::shape))))
    }
    fn eval(&self, function: u32, cx: &mut Eval) -> Result<(), Fault> {
        match function {
            0 => {
                let (vp, vn, r, first, second) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4));
                let g = 1.0 / r;
                cx.set(0, g * vp - g * vn + first + second);
                cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r), 1.0, 1.0]);
            }
            1 => cx.set(0, FIRST / cx.f64(0)),
            _ => cx.set(0, SECOND / cx.f64(0)),
        }
        Ok(())
    }
}

impl TwinFlicker {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        TWIN.get_or_init(|| module::<TwinFlicker>("TwinFlicker"))
    }
}
