//! The devices of the solver target `probe_selection`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "probe_selection.rs";

/// A conductor to ground that declares two observables: its term `state[0]`
/// (the current `1 mS · V(p)`) and its `VAR` watch `var[0]` (`10 · V(p)`).
/// Driven by a ramp both advance with the time.
/// pins: p=0 (PORT); terms: T0 FLOW{p, GROUND} "state[0]"; watches: W0 VAR "var[0]"
/// fn 0 "i": reads [p] writes [T0] partials (0<-0); fn 1 "show": reads [p] writes [W0]
pub struct StateAndVariableProbe;
static STUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STUB: OnceLock<Module> = OnceLock::new();

impl Device for StateAndVariableProbe {
    fn prototype() -> &'static Shape<'static> {
        STUB_SHAPE.get_or_init(ProbeSelectionShapes::stub_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((StateAndVariableProbe, Arc::clone(STUB_SHAPE.get_or_init(ProbeSelectionShapes::stub_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let vp = cx.f64(0);
        if k == 1 {
            cx.set(0, 10.0 * vp);
        } else {
            cx.set(0, 1.0e-3 * vp);
            cx.jac().copy_from_slice(&[1.0e-3]);
        }
        Ok(())
    }
}
impl StateAndVariableProbe {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        STUB.get_or_init(|| module::<StateAndVariableProbe>("Stub"))
    }
}

/// The shapes the `probe_selection` devices are built from.
pub struct ProbeSelectionShapes;

impl ProbeSelectionShapes {
    /// The cell of kind `kind` and index `index`.
    pub fn cell_of(kind: u32, index: u32) -> Cell {
        Cell { kind, index, part: part::VALUE }
    }

    /// The shape of `StateAndVariableProbe`, built once and shared by every instance.
    pub fn stub_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(TermSpec {
            name: "state[0]",
            origin: ORIGIN,
            op: op::FLOW,
            plus: p,
            minus: piperine_abi::raw::GROUND,
            abstol: DEFAULT,
            max: DEFAULT,
        });
        let w0 = s.watch(WatchSpec {
            name: "var[0]",
            origin: ORIGIN,
            kind: watch::VAR,
            layout: &F64,
            event: DEFAULT,
            ttol: DEFAULT,
            etol: DEFAULT,
        });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![ProbeSelectionShapes::cell_of(cell::PIN, p)],
            writes: vec![ProbeSelectionShapes::cell_of(cell::TERM, t0)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "show",
            origin: ORIGIN,
            reads: vec![ProbeSelectionShapes::cell_of(cell::PIN, p)],
            writes: vec![ProbeSelectionShapes::cell_of(cell::WATCH, w0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
