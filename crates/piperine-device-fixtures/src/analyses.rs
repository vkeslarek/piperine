//! The doubles the `tf`, `pz` and `sp` unit tests of the solver build their circuits from beside the
//! `circuit` resistor, capacitor, voltage source and inductor: a DC-only bias conductance, an ideal wire
//! and a device whose AC stamp is quadratic in frequency.

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

// ── Bias: a 1 nS conductance to ground that exists at DC only ───────────

/// A 1 nS conductance to ground that exists at DC only.
pub struct Bias;
static BIAS_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static BIAS_MODULE: OnceLock<Module> = OnceLock::new();

impl Bias {
    /// The module of a 1 nS conductance to ground that exists at DC only.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<Bias>(&BIAS_MODULE, "Bias")
    }

    /// The shape of `Bias`, built once and shared by every instance.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(AnalysisParts::flow("i", op::FLOW, p, GROUND));
        let is_static = Cell { kind: cell::AMBIENT, index: ambient::STATIC, part: part::VALUE };
        s.function(FunctionSpec {
            name: "i",
            origin: "test_devices.rs",
            reads: vec![AnalysisParts::pin(p), is_static],
            writes: vec![AnalysisParts::term(t0)],
            partials: vec![AnalysisParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Bias {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&BIAS_SHAPE, Bias::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Bias, Arc::clone(AnalysisParts::shared(&BIAS_SHAPE, Bias::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = if cx.bool(1) { 1.0e-9 } else { 0.0 };
        cx.set(0, g * cx.f64(0));
        cx.jac().copy_from_slice(&[g]);
        Ok(())
    }
}

/// An ideal wire between two nodes.
pub struct Wire;
static WIRE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static WIRE_MODULE: OnceLock<Module> = OnceLock::new();

impl Wire {
    /// The module of an ideal wire between two nodes.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<Wire>(&WIRE_MODULE, "Wire")
    }

    /// The shape of a wire: a private branch current `f`, a FLOW term {p, n} of value `f` and a FLOW term
    /// {f, GROUND} of `V(p) - V(n)`.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(AnalysisParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(AnalysisParts::flow("eq", op::FLOW, f, GROUND));
        s.function(FunctionSpec {
            name: "branch",
            origin: "test_devices.rs",
            reads: vec![AnalysisParts::pin(p), AnalysisParts::pin(n), AnalysisParts::pin(f)],
            writes: vec![AnalysisParts::term(t0), AnalysisParts::term(t1)],
            partials: vec![AnalysisParts::partial(0, 2), AnalysisParts::partial(1, 0), AnalysisParts::partial(1, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Wire {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&WIRE_SHAPE, Wire::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Wire, Arc::clone(AnalysisParts::shared(&WIRE_SHAPE, Wire::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, f);
        cx.set(1, vp - vn);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
}
// ── Quadratic: an AC stamp `jω²k` at (n1, n2) ───────────────────────────

/// A device whose AC stamp is `jω²k`, not affine in `jω`.
pub struct Quadratic;
static QUAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static QUAD_MODULE: OnceLock<Module> = OnceLock::new();

impl Quadratic {
    /// The module of a device whose AC stamp is `jω²k`, not affine in `jω`.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<Quadratic>(&QUAD_MODULE, "Quadratic")
    }

    /// The shape of `Quadratic`, built once and shared by every instance.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(AnalysisParts::param("k"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let b = s.pin(PinSpec::continuous("b", role::PORT));
        let t0 = s.term(AnalysisParts::flow("q", op::DDT, a, GROUND));
        let freq = Cell { kind: cell::AMBIENT, index: ambient::FREQ, part: part::VALUE };
        s.function(FunctionSpec {
            name: "q",
            origin: "test_devices.rs",
            reads: vec![AnalysisParts::pin(b), AnalysisParts::pin(0), freq],
            writes: vec![AnalysisParts::term(t0)],
            partials: vec![AnalysisParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Quadratic {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&QUAD_SHAPE, Quadratic::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Quadratic, Arc::clone(AnalysisParts::shared(&QUAD_SHAPE, Quadratic::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        // The DDT term's response is jω; the partial `ω·k` makes the stamp `jω²k`.
        let omega = 2.0 * std::f64::consts::PI * cx.f64(2);
        let g = omega * cx.f64(1);
        cx.set(0, g * cx.f64(0));
        cx.jac().copy_from_slice(&[g]);
        Ok(())
    }
}

/// The cells, specs and shared shapes every analysis fixture device builds its shape from.
pub struct AnalysisParts;

impl AnalysisParts {
    /// The cell of the value of pin `index`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of the value of term `index`.
    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The term spec named `name` of operator `operator`, between pins `plus` and `minus`.
    fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: "test_devices.rs", op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The `F64` param pin named `name`.
    fn param(name: &'static str) -> PinSpec<'static> {
        PinSpec::discrete(name, role::PARAM, &F64)
    }

    /// The shape and module of one device type behind a pair of statics.
    fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// The module of the device type `Kind` named `name`, built the first time and kept in `slot`.
    fn module_of<Kind: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
        slot.get_or_init(|| module::<Kind>(name))
    }
}
