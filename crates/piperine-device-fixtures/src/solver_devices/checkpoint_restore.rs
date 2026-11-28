//! The devices of the solver target `checkpoint_restore`, one type per device.

use crate::examples::LimitingDiode;
use std::cell::RefCell;
use std::result::Result;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

// ── T3: DC homotopy retry ─────────────────────────────────────────────────
//
// Spec ABI-07: when a DC homotopy strategy falls through (failed attempt →
// next strategy), the retry starts from the pre-attempt state. A stiff diode
// whose plain-Newton attempt is starved of iterations (max_iter=4) falls
// through to gmin stepping, which converges.

const ORIGIN: &str = "checkpoint_restore.rs";
const IS_DEFAULT: [u8; 8] = 1.0e-14_f64.to_le_bytes();

const THERMAL_VOLTAGE: f64 = 0.02585;

thread_local! {
    /// The `AGE` of the junction limit at every evaluation of its limit function.
    pub static AGES: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// A diode with `pnjlim` on its junction voltage that writes down the limit's
/// `AGE` at each evaluation (`AGES`): the limit state the solver checkpoints.
/// pins: is_sat=0 (PARAM F64); p=1, n=2 (PORT)
/// inputs: I0 LIMIT "vd"; terms: T0 FLOW{p, n} "i" = is·(exp(vd/vt) − 1)
/// fn 0 "limit": reads [p, n, is, I0.LAST, I0.AGE, STATIC] writes [I0.PROPOSED, I0.LIMITED]
///               partials (0<-0)=1 (0<-1)=-1
/// fn 1 "i": reads [I0, is] writes [T0] partials (0<-0)=gd (0<-1)
pub struct AgedDiode;
static AGED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static AGED: OnceLock<Module> = OnceLock::new();

impl Device for AgedDiode {
    fn prototype() -> &'static Shape<'static> {
        AGED_SHAPE.get_or_init(CheckpointRestoreShapes::aged_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AgedDiode, Arc::clone(AGED_SHAPE.get_or_init(CheckpointRestoreShapes::aged_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let vt = THERMAL_VOLTAGE;
            let vcrit = vt * (vt / (std::f64::consts::SQRT_2 * cx.f64(2))).ln();
            let v = cx.f64(0) - cx.f64(1);
            let (age, is_static) = (cx.u8(4), cx.bool(5));
            AGES.with(|a| a.borrow_mut().push(age));
            let old = if age == 0 || (is_static && age == 1) { vcrit } else { cx.f64(3) };
            cx.set(0, v);
            cx.set(1, LimitingDiode::limit_junction(v, old, vt, vcrit));
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let (vd, is) = (cx.f64(0), cx.f64(1));
            let e = (vd / THERMAL_VOLTAGE).exp();
            cx.set(0, is * (e - 1.0));
            cx.jac().copy_from_slice(&[is * e / THERMAL_VOLTAGE, e - 1.0]);
        }
        Ok(())
    }
}
impl AgedDiode {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        AGED.get_or_init(|| module::<AgedDiode>("AgedDiode"))
    }
}

/// The shapes the `checkpoint_restore` devices are built from.
pub struct CheckpointRestoreShapes;

impl CheckpointRestoreShapes {
    /// The cell of kind `kind` and index `index`.
    pub fn cell_of(kind: u32, index: u32, part: u32) -> Cell {
        Cell { kind, index, part }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    pub fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The shape of `AgedDiode`, built once and shared by every instance.
    pub fn aged_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let is = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &IS_DEFAULT, ..PinSpec::discrete("is_sat", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let i0 = s.input(InputSpec { name: "vd", origin: ORIGIN, kind: input::LIMIT });
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "limit",
            origin: ORIGIN,
            reads: vec![
                CheckpointRestoreShapes::cell_of(cell::PIN, p, part::VALUE),
                CheckpointRestoreShapes::cell_of(cell::PIN, n, part::VALUE),
                CheckpointRestoreShapes::cell_of(cell::PIN, is, part::VALUE),
                CheckpointRestoreShapes::cell_of(cell::INPUT, i0, part::LAST),
                CheckpointRestoreShapes::cell_of(cell::INPUT, i0, part::AGE),
                CheckpointRestoreShapes::cell_of(cell::AMBIENT, ambient::STATIC, part::VALUE),
            ],
            writes: vec![CheckpointRestoreShapes::cell_of(cell::INPUT, i0, part::PROPOSED), CheckpointRestoreShapes::cell_of(cell::INPUT, i0, part::LIMITED)],
            partials: vec![CheckpointRestoreShapes::partial(0, 0), CheckpointRestoreShapes::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![CheckpointRestoreShapes::cell_of(cell::INPUT, i0, part::VALUE), CheckpointRestoreShapes::cell_of(cell::PIN, is, part::VALUE)],
            writes: vec![CheckpointRestoreShapes::cell_of(cell::TERM, t0, part::VALUE)],
            partials: vec![CheckpointRestoreShapes::partial(0, 0), CheckpointRestoreShapes::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
