//! The devices of the solver target `failure_rules`, one type per device.

use std::result::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::BIT;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "failure_rules.rs";
const LATE_SOURCE_AT: f64 = 2.0e-7;

/// The §15 trigger: a contribution that is not finite.
pub struct NanSource;
static NAN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static NAN: OnceLock<Module> = OnceLock::new();

impl Device for NanSource {
    fn prototype() -> &'static Shape<'static> {
        NAN_SHAPE.get_or_init(FailureRulesShapes::two_port_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NanSource, Arc::clone(NAN_SHAPE.get_or_init(FailureRulesShapes::two_port_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, f64::NAN);
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
}

/// A pathological device: its conductance flips sign every evaluation, so no
/// Newton iteration can settle and no homotopy ramp helps. Nothing in the
/// stdlib behaves like this — it exists to reach §9's "every strategy failed"
/// path, which is otherwise only reachable by a real convergence disaster.
pub struct Oscillator {
    flip: AtomicBool,
}
static OSC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OSC: OnceLock<Module> = OnceLock::new();

impl Device for Oscillator {
    fn prototype() -> &'static Shape<'static> {
        OSC_SHAPE.get_or_init(FailureRulesShapes::two_port_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Oscillator { flip: AtomicBool::new(false) }, Arc::clone(OSC_SHAPE.get_or_init(FailureRulesShapes::two_port_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let flip = !self.flip.fetch_xor(true, Ordering::SeqCst);
        let g = if flip { 1e-3 } else { 1e3 };
        let rhs = if flip { 1.0 } else { -1.0 };
        let v = cx.f64(0) - cx.f64(1);
        cx.set(0, g * v - rhs);
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}

// ─── §17.1 E0508 / E0504 — a step that fails Newton at the floor ──────────────

/// A resistor that turns into the oscillator (its conductance flips sign every
/// evaluation, so no Newton iteration settles) once it trips: after 2e-7 s
/// (`LateOscillator`) or while its `go` pin is HIGH in a run (`JumpOscillator`).
pub struct LateOscillator {
    flip: AtomicBool,
}
/// A resistor that turns into the oscillator while its `go` pin is HIGH in a run: the jump-time twin of
/// `LateOscillator`.
pub struct JumpOscillator {
    flip: AtomicBool,
}
static LATE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LATE: OnceLock<Module> = OnceLock::new();
static JUMP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static JUMP: OnceLock<Module> = OnceLock::new();

impl Device for LateOscillator {
    fn prototype() -> &'static Shape<'static> {
        LATE_SHAPE.get_or_init(|| FailureRulesShapes::tripping_shape(false)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LateOscillator { flip: AtomicBool::new(false) }, Arc::clone(LATE_SHAPE.get_or_init(|| FailureRulesShapes::tripping_shape(false)))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let trips = cx.f64(2) > 2.0e-7;
        FailureRulesShapes::tripping_current(&self.flip, trips, cx);
        Ok(())
    }
}

impl Device for JumpOscillator {
    fn prototype() -> &'static Shape<'static> {
        JUMP_SHAPE.get_or_init(|| FailureRulesShapes::tripping_shape(true)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((JumpOscillator { flip: AtomicBool::new(false) }, Arc::clone(JUMP_SHAPE.get_or_init(|| FailureRulesShapes::tripping_shape(true)))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        // `go` HIGH once the run is under way (the operating point is at time 0).
        let trips = cx.bytes(2)[0] != 0 && cx.f64(3) > 0.0;
        FailureRulesShapes::tripping_current(&self.flip, trips, cx);
        Ok(())
    }
}

/// A potential source that exists only from `LATE_SOURCE_AT` on: before it the branch carries no
/// current (`f = 0`), after it `V(p, n) = 1 V` is imposed, so a second source across the same nodes
/// makes the Jacobian singular at a transient step while the operating point is fine.
/// pins: p=0, n=1 (PORT); f=2 (PRIVATE branch current)
/// terms: T0 FLOW{p, n} "i" = f, T1 FLOW{f, ground} "eq"
/// fn 0 "v": reads [p, n, f, TIME] writes [T0, T1] partials (0<-f) (1<-p) (1<-n) (1<-f)
pub struct LateSource;
static LATE_SOURCE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LATE_SOURCE: OnceLock<Module> = OnceLock::new();

impl Device for LateSource {
    fn prototype() -> &'static Shape<'static> {
        LATE_SOURCE_SHAPE.get_or_init(FailureRulesShapes::late_source_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LateSource, Arc::clone(LATE_SOURCE_SHAPE.get_or_init(FailureRulesShapes::late_source_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        if t > LATE_SOURCE_AT {
            cx.set(1, vp - vn - 1.0);
            cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, 0.0]);
        } else {
            cx.set(1, f);
            cx.jac().copy_from_slice(&[1.0, 0.0, 0.0, 1.0]);
        }
        Ok(())
    }
}

/// A 1 mS conductance at DC that is open at any frequency: a node it alone holds is fine at the
/// operating point and has a zero row in the small-signal system.
/// pins: p=0, n=1 (PORT); fn 0 "i": reads [p, n, FREQ] writes [T0] partials (0<-p) (0<-n)
pub struct OpenAboveDc;
static OPEN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OPEN: OnceLock<Module> = OnceLock::new();

impl Device for OpenAboveDc {
    fn prototype() -> &'static Shape<'static> {
        OPEN_SHAPE.get_or_init(FailureRulesShapes::open_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((OpenAboveDc, Arc::clone(OPEN_SHAPE.get_or_init(FailureRulesShapes::open_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = if cx.f64(2) > 0.0 { 0.0 } else { 1.0e-3 };
        cx.set(0, g * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}
impl JumpOscillator {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        JUMP.get_or_init(|| module::<JumpOscillator>("JumpOscillator"))
    }
}

impl LateOscillator {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        LATE.get_or_init(|| module::<LateOscillator>("LateOscillator"))
    }
}

impl LateSource {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        LATE_SOURCE.get_or_init(|| module::<LateSource>("LateSource"))
    }
}

impl NanSource {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        NAN.get_or_init(|| module::<NanSource>("NanSource"))
    }
}

impl OpenAboveDc {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        OPEN.get_or_init(|| module::<OpenAboveDc>("OpenAboveDc"))
    }
}

impl Oscillator {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        OSC.get_or_init(|| module::<Oscillator>("Oscillator"))
    }
}

/// The shapes the `failure_rules` devices are built from.
pub struct FailureRulesShapes;

impl FailureRulesShapes {
    /// The cell of kind `kind` and index `index`.
    pub fn cell_of(kind: u32, index: u32) -> Cell {
        Cell { kind, index, part: part::VALUE }
    }

    /// The shape of a two-port device `I(p, n) = f(V(p, n))`: one `FLOW` term and
    /// the partials against both ports.
    pub fn two_port_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![FailureRulesShapes::cell_of(cell::PIN, p), FailureRulesShapes::cell_of(cell::PIN, n)],
            writes: vec![FailureRulesShapes::cell_of(cell::TERM, t0)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// pins: p, n (PORT continuous), go (PORT BIT, jump variant only); one FLOW term.
    /// The function reads [p, n, TIME] or [p, n, go, TIME].
    pub fn tripping_shape(reads_go: bool) -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        let mut extra = Vec::new();
        if reads_go {
            let go = s.pin(PinSpec::discrete("go", role::PORT, &BIT));
            extra.push(FailureRulesShapes::cell_of(cell::PIN, go));
        }
        extra.push(time);
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: [vec![FailureRulesShapes::cell_of(cell::PIN, p), FailureRulesShapes::cell_of(cell::PIN, n)], extra].concat(),
            writes: vec![FailureRulesShapes::cell_of(cell::TERM, t0)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The current of a resistor of 1 kOhm, or of the oscillator when `trips`.
    pub fn tripping_current(flip: &AtomicBool, trips: bool, cx: &mut Eval) {
        let v = cx.f64(0) - cx.f64(1);
        let (g, rhs) = if trips {
            let flipped = !flip.fetch_xor(true, Ordering::SeqCst);
            if flipped { (1e-3, 1.0) } else { (1e3, -1.0) }
        } else {
            (1e-3, 0.0)
        };
        cx.set(0, g * v - rhs);
        cx.jac().copy_from_slice(&[g, -g]);
    }

    /// The shape of `LateSource`, built once and shared by every instance.
    pub fn late_source_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let t1 = s.term(TermSpec { name: "eq", origin: ORIGIN, op: op::FLOW, plus: f, minus: piperine_abi::raw::GROUND, abstol: DEFAULT, max: DEFAULT });
        let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![FailureRulesShapes::cell_of(cell::PIN, p), FailureRulesShapes::cell_of(cell::PIN, n), FailureRulesShapes::cell_of(cell::PIN, f), time],
            writes: vec![FailureRulesShapes::cell_of(cell::TERM, t0), FailureRulesShapes::cell_of(cell::TERM, t1)],
            partials: vec![
                Partial { write: 0, to: 0, read: 2, at: 0 },
                Partial { write: 1, to: 0, read: 0, at: 0 },
                Partial { write: 1, to: 0, read: 1, at: 0 },
                Partial { write: 1, to: 0, read: 2, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `OpenAboveDc`, built once and shared by every instance.
    pub fn open_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        let frequency = Cell { kind: cell::AMBIENT, index: ambient::FREQ, part: part::VALUE };
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![FailureRulesShapes::cell_of(cell::PIN, p), FailureRulesShapes::cell_of(cell::PIN, n), frequency],
            writes: vec![FailureRulesShapes::cell_of(cell::TERM, t0)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
