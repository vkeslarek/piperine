//! The two-terminal doubles the `tf`, `pz` and `sp` unit tests build their
//! circuits from, as `Device`s: a resistor, a capacitor, a DC-only bias
//! conductance, an ideal wire, a DC voltage source, an inductor and a device
//! whose AC stamp is quadratic in frequency. Each is added to a
//! `CircuitBuilder` by a plain function.

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

// ── R: `I(p, n) <+ V(p, n) / r` ─────────────────────────────────────────

/// A resistor `I(p, n) <+ V(p, n) / r`.
pub struct Resistor;
static R_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static R_MODULE: OnceLock<Module> = OnceLock::new();

impl Resistor {
    /// The module of a resistor `I(p, n) <+ V(p, n) / r`.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<Resistor>(&R_MODULE, "Resistor")
    }

    /// The shape helper `shape`.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(AnalysisParts::param("r"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(AnalysisParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: "test_devices.rs",
            reads: vec![AnalysisParts::pin(p), AnalysisParts::pin(n), AnalysisParts::pin(0)],
            writes: vec![AnalysisParts::term(t0)],
            partials: vec![AnalysisParts::partial(0, 0), AnalysisParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Resistor {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&R_SHAPE, Resistor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resistor, Arc::clone(AnalysisParts::shared(&R_SHAPE, Resistor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, g) = (cx.f64(0), cx.f64(1), 1.0 / cx.f64(2));
        cx.set(0, g * vp - g * vn);
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

// ── C: `I(p, n) <+ c * ddt(V(p, n))` ────────────────────────────────────

/// A capacitor `I(p, n) <+ c * ddt(V(p, n))`.
pub struct Capacitor;
static C_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static C_MODULE: OnceLock<Module> = OnceLock::new();

impl Capacitor {
    /// The module of a capacitor `I(p, n) <+ c * ddt(V(p, n))`.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<Capacitor>(&C_MODULE, "Capacitor")
    }

    /// The shape helper `shape`.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(AnalysisParts::param("c"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(AnalysisParts::flow("q", op::DDT, p, n));
        s.function(FunctionSpec {
            name: "q",
            origin: "test_devices.rs",
            reads: vec![AnalysisParts::pin(p), AnalysisParts::pin(n), AnalysisParts::pin(0)],
            writes: vec![AnalysisParts::term(t0)],
            partials: vec![AnalysisParts::partial(0, 0), AnalysisParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Capacitor {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&C_SHAPE, Capacitor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Capacitor, Arc::clone(AnalysisParts::shared(&C_SHAPE, Capacitor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, c) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, c * vp - c * vn);
        cx.jac().copy_from_slice(&[c, -c]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

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

    /// The shape helper `shape`.
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
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
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
}

impl Device for Wire {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&WIRE_SHAPE, || BranchShape::of(None)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Wire, Arc::clone(AnalysisParts::shared(&WIRE_SHAPE, || BranchShape::of(None)))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, f);
        cx.set(1, vp - vn);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A DC voltage source.
pub struct VoltageSource;
static VSRC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static VSRC_MODULE: OnceLock<Module> = OnceLock::new();

impl VoltageSource {
    /// The module of a DC voltage source.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<VoltageSource>(&VSRC_MODULE, "VoltageSource")
    }
}

impl Device for VoltageSource {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&VSRC_SHAPE, || BranchShape::of(Some(("dc", false)))).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((VoltageSource, Arc::clone(AnalysisParts::shared(&VSRC_SHAPE, || BranchShape::of(Some(("dc", false)))))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, dc) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn - dc);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// An ideal inductor: a DC short whose AC branch row adds `-jωL`.
pub struct Inductor;
static L_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static L_MODULE: OnceLock<Module> = OnceLock::new();

impl Inductor {
    /// The module of an ideal inductor: a DC short whose AC branch row adds `-jωL`.
    pub fn module() -> &'static Module {
        AnalysisParts::module_of::<Inductor>(&L_MODULE, "Inductor")
    }
}

impl Device for Inductor {
    fn prototype() -> &'static Shape<'static> {
        AnalysisParts::shared(&L_SHAPE, || BranchShape::of(Some(("l", true)))).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Inductor, Arc::clone(AnalysisParts::shared(&L_SHAPE, || BranchShape::of(Some(("l", true)))))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, l) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        cx.set(1, vp - vn);
        cx.set(2, -l * f);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -l, -f]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
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

    /// The shape helper `shape`.
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
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// The cells, specs and shared shapes every analysis fixture device builds its shape from.
pub struct AnalysisParts;

impl AnalysisParts {
    /// The pin helper `pin`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The term helper `term`.
    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The partial helper `partial`.
    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The flow helper `flow`.
    fn flow(name: &'static str, operator: u32, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: "test_devices.rs", op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The param helper `param`.
    fn param(name: &'static str) -> PinSpec<'static> {
        PinSpec::discrete(name, role::PARAM, &F64)
    }

    /// The shape and module of one device type behind a pair of statics.
    fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// The module of helper `module_of`.
    fn module_of<Kind: Device>(slot: &'static OnceLock<Module>, name: &'static str) -> &'static Module {
        slot.get_or_init(|| module::<Kind>(name))
    }
}

/// The shape of the devices that own a private branch current.
pub struct BranchShape;

impl BranchShape {
    // ── Branch devices: wire, voltage source, inductor ──────────────────────
    //
    // Each owns a private branch current `f`: a FLOW term {p, n} of value `f` and
    // a FLOW term {f, GROUND} of `V(p) - V(n) - value`; the inductor adds a DDT
    // term {f, GROUND} of `-l * f`.

    /// The of helper `of`.
    pub fn of(extra: Option<(&'static str, bool)>) -> ShapeRef {
        let mut s = ShapeBuf::new();
        if let Some((name, _)) = extra {
            s.pin(AnalysisParts::param(name));
        }
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(AnalysisParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(AnalysisParts::flow("eq", op::FLOW, f, GROUND));
        let mut writes = vec![AnalysisParts::term(t0), AnalysisParts::term(t1)];
        let mut partials = vec![AnalysisParts::partial(0, 2), AnalysisParts::partial(1, 0), AnalysisParts::partial(1, 1)];
        let mut reads = vec![AnalysisParts::pin(p), AnalysisParts::pin(n), AnalysisParts::pin(f)];
        if let Some((_, reactive)) = extra {
            reads.push(AnalysisParts::pin(0));
            if reactive {
                // The inductor: flux `-l * f`.
                let t2 = s.term(AnalysisParts::flow("flux", op::DDT, f, GROUND));
                writes.push(AnalysisParts::term(t2));
                partials.push(AnalysisParts::partial(2, 2));
                partials.push(AnalysisParts::partial(2, 3));
            } else {
                // The source: its value enters `eq` with partial -1.
                partials.push(AnalysisParts::partial(1, 3));
            }
        }
        s.function(FunctionSpec { name: "branch", origin: "test_devices.rs", reads, writes, partials, ..FunctionSpec::default() });
        s.freeze()
    }
}
