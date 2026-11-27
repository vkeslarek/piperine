//! The analog side's own fixture devices, each built for one rule of the analog assembly. The worked
//! examples they sit beside live in `examples`. Every shape lists its `PARAM` pins first (P5).

use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{F64};
use piperine_abi::raw::{
    Cell, Created, DEFAULT, Eval as RawEval, GROUND, Instance, List, Module, Note, Partial, React as RawReact, Shape,
    Field, Layout, Statement, Str, Value, ambient, cell, fn_flag, global, input, op, part, role, status, watch,
};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};

use crate::circuit::{Capacitor, Resistor, VoltageSource};
use crate::digital::DataFlipFlop;
use crate::examples::{DigitalToAnalog, KF, K_B, LimitingDiode, Q, ShapeParts};
use crate::mixed::Comparator;

const SQRT_2: f64 = std::f64::consts::SQRT_2;

/// A capacitor whose `DDT` term gives its own `abstol` (§14).
/// pins: c=0, tol=1 (PARAM F64); p=2, n=3 (PORT)      terms: T0 DDT{p,n} "q", abstol = tol
pub struct TightCapacitor;
static TIGHT_CAPACITOR: OnceLock<ShapeRef> = OnceLock::new();

impl TightCapacitor {
    /// The shape of `TightCapacitor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let c = s.pin(ShapeParts::param("c"));
        let tol = s.pin(ShapeParts::param("tol"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { abstol: tol, ..ShapeParts::flow("q", op::DDT, p, n) });
        s.function(FunctionSpec {
            name: "q",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(c)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for TightCapacitor {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&TIGHT_CAPACITOR, TightCapacitor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TightCapacitor, Arc::clone(ShapeParts::shared(&TIGHT_CAPACITOR, TightCapacitor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, c * v);
        cx.jac().copy_from_slice(&[c, -c, v]);
        Ok(())
    }
}

/// Every fixture module, by name.
#[derive(Clone, Copy)]
pub struct FixtureModules {
    /// The module of the resistor fixture.
    pub resistor: &'static Module,
    /// The module of the diode fixture.
    pub diode: &'static Module,
    /// The module of the capacitor fixture.
    pub capacitor: &'static Module,
    /// The module of the flip flop fixture.
    pub flip_flop: &'static Module,
    /// The module of the comparator fixture.
    pub comparator: &'static Module,
    /// The module of the dac fixture.
    pub digital_to_analog: &'static Module,
    /// The module of the cubic fixture.
    pub cubic: &'static Module,
    /// The module of the bad limit fixture.
    pub bad_limit: &'static Module,
    /// The module of the source fixture.
    pub source: &'static Module,
    /// The module of the counted fixture.
    pub counted: &'static Module,
    /// The module of the probe fixture.
    pub probe: &'static Module,
    /// The module of the faulty fixture.
    pub faulty: &'static Module,
    /// The module of the plain diode fixture.
    pub plain_diode: &'static Module,
    /// The module of the vsource fixture.
    pub voltage_source: &'static Module,
    /// The module of the clamp fixture.
    pub clamp: &'static Module,
    /// The module of the owner fixture.
    pub owner: &'static Module,
    /// The module of the reader fixture.
    pub reader: &'static Module,
    /// The module of the tracker fixture.
    pub tracker: &'static Module,
    /// The module of the delay line fixture.
    pub delay_line: &'static Module,
    /// The module of the trio fixture.
    pub trio: &'static Module,
    /// The module of the ac vsource fixture.
    pub ac_voltage_source: &'static Module,
    /// The module of the noisy fixture.
    pub noisy: &'static Module,
    /// The module of the split fixture.
    pub split: &'static Module,
    /// The module of the flicker fixture.
    pub flicker: &'static Module,
    /// The module of the capper fixture.
    pub cap_watch_device: &'static Module,
    /// The module of the tracer fixture.
    pub tracer: &'static Module,
    /// The module of the sample hold fixture.
    pub sample_hold: &'static Module,
    /// The module of the charge diode fixture.
    pub charge_diode: &'static Module,
    /// The module of the tight capacitor fixture.
    pub tight_capacitor: &'static Module,
}

/// pins: k=0 (PARAM F64); p=1, n=2 (PORT continuous)
/// terms: T0 FLOW {p, n} "i" = k·v³, whose partials vanish at v = 0
/// fn 0 "i": reads [p, n, k] writes [T0] partials (0←0) (0←1) (0←2)
pub struct Cubic;
static CUBIC: OnceLock<ShapeRef> = OnceLock::new();

impl Cubic {
    /// The shape of `Cubic`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let k = s.pin(ShapeParts::param("k"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(k)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Cubic {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&CUBIC, Cubic::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Cubic, Arc::clone(ShapeParts::shared(&CUBIC, Cubic::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, k) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, k * v * v * v);
        let g = 3.0 * k * v * v;
        cx.jac().copy_from_slice(&[g, -g, v * v * v]);
        Ok(())
    }
}

/// A shape that breaks abi.md A3: one function writes a limit and a term
/// without `OWNS_CORRECTION`.
/// pins: a=0, k=1 (PORT continuous)   inputs: I0 LIMIT "vd"   terms: T0 FLOW {a, k} "i"
/// fn 0 "both": reads [a, k] writes [I0.PROPOSED, I0.LIMITED, T0]
pub struct BadLimit;
static BAD_LIMIT: OnceLock<ShapeRef> = OnceLock::new();

impl BadLimit {
    /// The shape that breaks the rule.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let i0 = s.input(InputSpec { name: "vd", origin: "fixture:bad_limit", kind: input::LIMIT });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, a, k));
        s.function(FunctionSpec {
            name: "both",
            origin: "fixture:bad_limit:fn0",
            reads: vec![ShapeParts::pin(a), ShapeParts::pin(k)],
            writes: vec![ShapeParts::input_cell(i0, part::PROPOSED), ShapeParts::input_cell(i0, part::LIMITED), ShapeParts::term(t0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A foreign module whose `create` hands out the shape unchecked, as a C
    /// library could: the thunks of `piperine_abi::thunk` validate before they
    /// return a shape, so only a hand-written module reaches bind with one that
    /// breaks a rule. Its instance is never called.
    pub fn module() -> Module {
        // SAFETY: the prototype is a frozen static shape that lives for the
        // process; `create` hands out that shape and a never-dereferenced
        // instance, `destroy` frees nothing, and `eval`/`react` read nothing.
        unsafe {
            Module::from_raw_parts(
                Str::from_static("BadLimit"),
                ShapeParts::shared(&BAD_LIMIT, BadLimit::shape).as_raw(),
                std::ptr::null(),
                BadLimit::create,
                BadLimit::destroy,
                BadLimit::eval,
                BadLimit::react,
            )
        }
    }

    /// `create`: hands out the unchecked shape and a never-dereferenced instance.
    extern "C" fn create(_: *const Module, _: List<Value>, _: *mut Note) -> Created {
        let shape = ShapeParts::shared(&BAD_LIMIT, BadLimit::shape).as_raw();
        Created { status: status::OK, instance: NonNull::dangling().as_ptr(), shape }
    }

    /// `destroy`: frees nothing.
    extern "C" fn destroy(_: *mut Instance) {}

    /// `eval`: reads nothing and faults.
    extern "C" fn eval(_: *mut Instance, _: u32, _: *const RawEval) -> u32 {
        status::FAULT
    }

    /// `react`: reads nothing and faults.
    extern "C" fn react(_: *mut Instance, _: u32, _: *const RawReact) -> u32 {
        status::FAULT
    }
}

// ─── Recording fixtures ─────────────────────────────────────────────────────

/// One call of a recording fixture: its tag, the bytes it read and which of
/// its partials were seeded.
#[derive(Clone, Debug)]
pub struct RecordedCall {
    /// The tag.
    pub tag: u64,
    /// The reads.
    pub reads: Vec<Vec<u8>>,
    /// The seed.
    pub seed: Vec<bool>,
}

static CALLS: Mutex<Vec<RecordedCall>> = Mutex::new(Vec::new());

/// pins: tag=0, idc=1 (PARAM F64); p=2, n=3 (PORT continuous)
/// terms: T0 FLOW {p, n} "i" = idc: a current source that reads only a param
/// fn 0 "i": reads [idc] writes [T0] partials (0←0)=1        (records every call)
pub struct CurrentSource {
    tag: u64,
}
static SOURCE: OnceLock<ShapeRef> = OnceLock::new();

impl CurrentSource {
    /// The shape of `CurrentSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(ShapeParts::param("tag"));
        let idc = s.pin(ShapeParts::param("idc"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(idc)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for CurrentSource {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&SOURCE, CurrentSource::shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CurrentSource { tag: RecordedCall::tag_of(values) }, Arc::clone(ShapeParts::shared(&SOURCE, CurrentSource::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        RecordedCall::record(self.tag, cx, 1);
        let idc = cx.f64(0);
        cx.set(0, idc);
        cx.jac()[0] = 1.0;
        Ok(())
    }
}

/// pins: tag=0, r=1 (PARAM F64); p=2, n=3 (PORT continuous)
/// terms: T0 FLOW {p, n} "i"
/// fn 0 "i": reads [p, n, r] writes [T0] partials (0←0) (0←1) (0←2)   (records every call)
pub struct CountedResistor {
    tag: u64,
}
static COUNTED: OnceLock<ShapeRef> = OnceLock::new();

impl CountedResistor {
    /// The shape of `CountedResistor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(ShapeParts::param("tag"));
        let r = s.pin(ShapeParts::param("r"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(r)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for CountedResistor {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&COUNTED, CountedResistor::shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CountedResistor { tag: RecordedCall::tag_of(values) }, Arc::clone(ShapeParts::shared(&COUNTED, CountedResistor::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        RecordedCall::record(self.tag, cx, 3);
        let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        cx.set(0, v / r);
        cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]);
        Ok(())
    }
}

/// pins: tag=0 (PARAM F64); p=1 (PORT continuous)
/// terms: T0 FLOW {p, GROUND} "g" = v(p): a 1 S conductance
/// fn 0 "g": reads [p, TIME, FREQ, STATIC, GMIN, SOURCE_SCALE] writes [T0] partials (0←0)=1   (records)
pub struct RecordingConductance {
    tag: u64,
}
static PROBE: OnceLock<ShapeRef> = OnceLock::new();

impl RecordingConductance {
    /// The shape of `RecordingConductance`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(ShapeParts::param("tag"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(ShapeParts::flow("g", op::FLOW, p, GROUND));
        s.function(FunctionSpec {
            name: "g",
            reads: vec![
                ShapeParts::pin(p),
                ShapeParts::ambient(ambient::TIME),
                ShapeParts::ambient(ambient::FREQ),
                ShapeParts::ambient(ambient::STATIC),
                ShapeParts::ambient(ambient::GMIN),
                ShapeParts::ambient(ambient::SOURCE_SCALE),
            ],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for RecordingConductance {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&PROBE, RecordingConductance::shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((RecordingConductance { tag: RecordedCall::tag_of(values) }, Arc::clone(ShapeParts::shared(&PROBE, RecordingConductance::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        RecordedCall::record(self.tag, cx, 1);
        let v = cx.f64(0);
        cx.set(0, v);
        cx.jac()[0] = 1.0;
        Ok(())
    }
}

/// pins: mode=0 (PARAM F64); p=1, n=2 (PORT continuous)
/// terms: T0 FLOW {p, n} "i" = v(p) − v(n), origin "fixture:faulty:T0"
/// fn 0 "i": reads [p, n, mode] writes [T0] partials (0←0) (0←1) (0←2)
/// mode 1: the value is NaN; 2: the seeded partial (0←0) is +inf; 3: the
/// partial against the param (never seeded in a Newton solve) is +inf;
/// 4: the call faults with code 7.
pub struct ModeFaulty;
static FAULTY: OnceLock<ShapeRef> = OnceLock::new();

impl ModeFaulty {
    /// The shape of `ModeFaulty`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let mode = s.pin(ShapeParts::param("mode"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(TermSpec { origin: "fixture:faulty:T0", ..ShapeParts::flow("i", op::FLOW, p, n) });
        s.function(FunctionSpec {
            name: "i",
            origin: "fixture:faulty:fn0",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(mode)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for ModeFaulty {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&FAULTY, ModeFaulty::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ModeFaulty, Arc::clone(ShapeParts::shared(&FAULTY, ModeFaulty::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let mode = cx.f64(2);
        if mode == 4.0 {
            return Err(Fault::new(7, "mode four"));
        }
        let v = cx.f64(0) - cx.f64(1);
        cx.set(0, if mode == 1.0 { f64::NAN } else { v });
        let g = if mode == 2.0 { f64::INFINITY } else { 1.0 };
        let dm = if mode == 3.0 { f64::INFINITY } else { 0.0 };
        cx.jac().copy_from_slice(&[g, -1.0, dm]);
        Ok(())
    }
}

// ─── Plain devices for analytic circuits ────────────────────────────────────

/// The thermal voltage of the plain diode, as `tests/helpers/devices.rs` of
/// `piperine-solver` writes it.
pub const VT: f64 = 0.02585;

/// pins: is=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// terms: T0 FLOW {a, k} "id" = is·(exp(v/VT) − 1), no limiting
/// fn 0 "id": reads [a, k, is] writes [T0] partials (0←0) (0←1) (0←2)
pub struct PlainDiode;
static PLAIN_DIODE: OnceLock<ShapeRef> = OnceLock::new();

impl PlainDiode {
    /// The shape of `PlainDiode`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let is = s.pin(ShapeParts::param("is"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let t0 = s.term(ShapeParts::flow("id", op::FLOW, a, k));
        s.function(FunctionSpec {
            name: "id",
            reads: vec![ShapeParts::pin(a), ShapeParts::pin(k), ShapeParts::pin(is)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for PlainDiode {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&PLAIN_DIODE, PlainDiode::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((PlainDiode, Arc::clone(ShapeParts::shared(&PLAIN_DIODE, PlainDiode::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, is) = (cx.f64(0) - cx.f64(1), cx.f64(2));
        let e = (v / VT).exp();
        cx.set(0, is * (e - 1.0));
        let gd = is * e / VT;
        cx.jac().copy_from_slice(&[gd, -gd, e - 1.0]);
        Ok(())
    }
}

// ─── Limiting fixtures ──────────────────────────────────────────────────────

/// The conductance scale of the limiting fixtures.
pub const G_LIMIT: f64 = 1e-3;

/// A limit whose held iterate is offset from the proposed value by a param.
/// pins: off=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, k} "i" = G_LIMIT·x̂²
/// fn 0 "limit": reads [a, k, off] writes [I0.PROPOSED = v(a) − v(k), I0.LIMITED = PROPOSED + off]
///               partials (0←0)=1 (0←1)=−1
/// fn 1 "i":     reads [I0] writes [T0] partials (0←0)=2·G_LIMIT·x̂
pub struct Clamp;
static CLAMP: OnceLock<ShapeRef> = OnceLock::new();

impl Clamp {
    /// The reads of a limit fixture (pins `a` and `k`, then `extra`) and its partials against the two pins.
    fn limit_reads_and_partials(a: u32, k: u32, extra: Vec<Cell>) -> (Vec<Cell>, Vec<Partial>) {
        let mut reads = vec![ShapeParts::pin(a), ShapeParts::pin(k)];
        reads.extend(extra);
        (reads, vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1)])
    }

    /// The shape of `Clamp`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let off = s.pin(ShapeParts::param("off"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let i0 = s.input(InputSpec { name: "x", origin: "fixture:clamp:I0", kind: input::LIMIT });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, a, k));
        let (reads, partials) = Clamp::limit_reads_and_partials(a, k, vec![ShapeParts::pin(off)]);
        s.function(FunctionSpec {
            name: "limit",
            reads,
            writes: vec![ShapeParts::input_cell(i0, part::PROPOSED), ShapeParts::input_cell(i0, part::LIMITED)],
            partials,
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::input_cell(i0, part::VALUE)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Clamp {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&CLAMP, Clamp::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Clamp, Arc::clone(ShapeParts::shared(&CLAMP, Clamp::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let x = cx.f64(0) - cx.f64(1);
            let off = cx.f64(2);
            cx.set(0, x);
            cx.set(1, x + off);
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let xh = cx.f64(0);
            cx.set(0, G_LIMIT * xh * xh);
            cx.jac()[0] = 2.0 * G_LIMIT * xh;
        }
        Ok(())
    }
}

/// A function that writes a limit and the term depending on it in one call
/// (a compiled foreign model, abi.md A3): flagged `OWNS_CORRECTION`.
/// pins: off=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, k} "i" = G_LIMIT·(v(a) − v(k)) + 1e-3
/// fn 0 "all" OWNS_CORRECTION: reads [a, k, off]
///     writes [I0.PROPOSED = v, I0.LIMITED = v + off, T0]
///     partials (0←0)=1 (0←1)=−1 (2←0)=G_LIMIT (2←1)=−G_LIMIT
pub struct CorrectionOwner;
static OWNER: OnceLock<ShapeRef> = OnceLock::new();

impl CorrectionOwner {
    /// The shape of `CorrectionOwner`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let off = s.pin(ShapeParts::param("off"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let i0 = s.input(InputSpec { name: "x", origin: "fixture:owner:I0", kind: input::LIMIT });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, a, k));
        s.function(FunctionSpec {
            name: "all",
            flags: fn_flag::OWNS_CORRECTION,
            reads: vec![ShapeParts::pin(a), ShapeParts::pin(k), ShapeParts::pin(off)],
            writes: vec![ShapeParts::input_cell(i0, part::PROPOSED), ShapeParts::input_cell(i0, part::LIMITED), ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(2, 0), ShapeParts::partial(2, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for CorrectionOwner {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&OWNER, CorrectionOwner::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CorrectionOwner, Arc::clone(ShapeParts::shared(&OWNER, CorrectionOwner::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let v = cx.f64(0) - cx.f64(1);
        cx.set(0, v);
        cx.set(1, v + cx.f64(2));
        cx.set(2, G_LIMIT * v + 1e-3);
        cx.jac().copy_from_slice(&[1.0, -1.0, G_LIMIT, -G_LIMIT]);
        Ok(())
    }
}

/// A limit that no function writes, read by one (abi.md A3).
/// pins: tag=0 (PARAM F64); a=1 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, GROUND} "i" = 2·x̂
/// fn 0 "i": reads [I0] writes [T0] partials (0←0)=2        (records every call)
pub struct LimitReader {
    tag: u64,
}
static READER: OnceLock<ShapeRef> = OnceLock::new();

impl LimitReader {
    /// The shape of `LimitReader`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(ShapeParts::param("tag"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let i0 = s.input(InputSpec { name: "x", origin: "fixture:reader:I0", kind: input::LIMIT });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, a, GROUND));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::input_cell(i0, part::VALUE)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for LimitReader {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&READER, LimitReader::shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LimitReader { tag: RecordedCall::tag_of(values) }, Arc::clone(ShapeParts::shared(&READER, LimitReader::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        RecordedCall::record(self.tag, cx, 1);
        let xh = cx.f64(0);
        cx.set(0, 2.0 * xh);
        cx.jac()[0] = 2.0;
        Ok(())
    }
}

/// A limit that never limits, recording what its function reads of it.
/// pins: tag=0 (PARAM F64); a=1, k=2 (PORT continuous)
/// inputs: I0 LIMIT "x"     terms: T0 FLOW {a, k} "i" = G_LIMIT·x̂
/// fn 0 "limit": reads [a, k, I0.LAST, I0.AGE, STATIC] writes [I0.PROPOSED, I0.LIMITED] = v, v
///               partials (0←0)=1 (0←1)=−1            (records every call)
/// fn 1 "i":     reads [I0] writes [T0] partials (0←0)=G_LIMIT
pub struct Tracker {
    tag: u64,
}
static TRACKER: OnceLock<ShapeRef> = OnceLock::new();

impl Tracker {
    /// The shape of `Tracker`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(ShapeParts::param("tag"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let i0 = s.input(InputSpec { name: "x", origin: "fixture:tracker:I0", kind: input::LIMIT });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, a, k));
        let extra = vec![ShapeParts::input_cell(i0, part::LAST), ShapeParts::input_cell(i0, part::AGE), ShapeParts::ambient(ambient::STATIC)];
        let (reads, partials) = Clamp::limit_reads_and_partials(a, k, extra);
        s.function(FunctionSpec {
            name: "limit",
            reads,
            writes: vec![ShapeParts::input_cell(i0, part::PROPOSED), ShapeParts::input_cell(i0, part::LIMITED)],
            partials,
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::input_cell(i0, part::VALUE)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Tracker {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&TRACKER, Tracker::shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Tracker { tag: RecordedCall::tag_of(values) }, Arc::clone(ShapeParts::shared(&TRACKER, Tracker::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            RecordedCall::record(self.tag, cx, 2);
            let v = cx.f64(0) - cx.f64(1);
            cx.set(0, v);
            cx.set(1, v);
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let xh = cx.f64(0);
            cx.set(0, G_LIMIT * xh);
            cx.jac()[0] = G_LIMIT;
        }
        Ok(())
    }
}

// ─── Delay fixture ──────────────────────────────────────────────────────────

/// pins: td=0, max=1 (PARAM F64); a=2, o=3 (PORT continuous)
/// terms: T0 DELAY {o, GROUND} "d" = v(a), delay td, bound `max`;  origin "fixture:delay:T0"
/// fn 0 "d": reads [a, td] writes [T0.VALUE, T0.TIME] partials (0←0)=1
pub struct DelayLine;
static DELAY_LINE: OnceLock<ShapeRef> = OnceLock::new();

impl DelayLine {
    /// The shape of `DelayLine`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let td = s.pin(ShapeParts::param("td"));
        let max = s.pin(ShapeParts::param("max"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let o = s.pin(PinSpec::continuous("o", role::PORT));
        let t0 = s.term(TermSpec { origin: "fixture:delay:T0", max, ..ShapeParts::flow("d", op::DELAY, o, GROUND) });
        s.function(FunctionSpec {
            name: "d",
            reads: vec![ShapeParts::pin(a), ShapeParts::pin(td)],
            writes: vec![ShapeParts::term(t0), Cell { kind: cell::TERM, index: t0, part: part::TIME }],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for DelayLine {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&DELAY_LINE, DelayLine::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((DelayLine, Arc::clone(ShapeParts::shared(&DELAY_LINE, DelayLine::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, td) = (cx.f64(0), cx.f64(1));
        cx.set(0, v);
        cx.set(1, td);
        cx.jac()[0] = 1.0;
        Ok(())
    }
}

// ─── Small-signal fixtures ──────────────────────────────────────────────────

/// One term of each operator, each on its own pin.
/// pins: a=0, b=1, c=2, td=3 (PARAM F64); p0=4, p1=5, p2=6 (PORT continuous)
/// terms: T0 FLOW {p0, GROUND} = a·v(p0);  T1 DDT {p1, GROUND} = b·v(p1);
///        T2 DELAY {p2, GROUND} = c·v(p2), delay td
/// fn 0 "all": reads [p0, p1, p2, a, b, c, td] writes [T0, T1, T2, T2.TIME]
///             partials (0←0)=a (1←1)=b (2←2)=c
pub struct Trio;
static TRIO: OnceLock<ShapeRef> = OnceLock::new();

impl Trio {
    /// The shape of `Trio`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let (a, b, c, td) = (s.pin(ShapeParts::param("a")), s.pin(ShapeParts::param("b")), s.pin(ShapeParts::param("c")), s.pin(ShapeParts::param("td")));
        let p0 = s.pin(PinSpec::continuous("p0", role::PORT));
        let p1 = s.pin(PinSpec::continuous("p1", role::PORT));
        let p2 = s.pin(PinSpec::continuous("p2", role::PORT));
        let t0 = s.term(ShapeParts::flow("f", op::FLOW, p0, GROUND));
        let t1 = s.term(ShapeParts::flow("q", op::DDT, p1, GROUND));
        let t2 = s.term(ShapeParts::flow("d", op::DELAY, p2, GROUND));
        s.function(FunctionSpec {
            name: "all",
            reads: vec![ShapeParts::pin(p0), ShapeParts::pin(p1), ShapeParts::pin(p2), ShapeParts::pin(a), ShapeParts::pin(b), ShapeParts::pin(c), ShapeParts::pin(td)],
            writes: vec![ShapeParts::term(t0), ShapeParts::term(t1), ShapeParts::term(t2), Cell { kind: cell::TERM, index: t2, part: part::TIME }],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(1, 1), ShapeParts::partial(2, 2)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Trio {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&TRIO, Trio::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Trio, Arc::clone(ShapeParts::shared(&TRIO, Trio::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (a, b, c, td) = (cx.f64(3), cx.f64(4), cx.f64(5), cx.f64(6));
        cx.set(0, a * cx.f64(0));
        cx.set(1, b * cx.f64(1));
        cx.set(2, c * cx.f64(2));
        cx.set(3, td);
        cx.jac().copy_from_slice(&[a, b, c]);
        Ok(())
    }
}

/// The potential source of abi.md A1 with a small-signal stimulus.
/// pins: dc=0, re=1, im=2 (PARAM F64); p=3, n=4 (PORT continuous); f=5 (PRIVATE continuous)
/// inputs: I0 STIM "ac"
/// terms: T0 FLOW {p, n} "i" = f;  T1 FLOW {f, GROUND} "eq" = x(p) − x(n) − dc − I0
/// fn 0 "v":  reads [p, n, f, dc, I0] writes [T0, T1] partials (0←2)=1 (1←0)=1 (1←1)=−1 (1←4)=−1
/// fn 1 "ac": reads [re, im] writes [I0.RE, I0.IM]
pub struct AcVoltageSource;
static AC_VSOURCE: OnceLock<ShapeRef> = OnceLock::new();

impl AcVoltageSource {
    /// The shape of `AcVoltageSource`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let (dc, re, im) = (s.pin(ShapeParts::param("dc")), s.pin(ShapeParts::param("re")), s.pin(ShapeParts::param("im")));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let i0 = s.input(InputSpec { name: "ac", origin: "fixture:ac_vsource:I0", kind: input::STIM });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, n));
        let t1 = s.term(ShapeParts::flow("eq", op::FLOW, f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(f), ShapeParts::pin(dc), ShapeParts::input_cell(i0, part::VALUE)],
            writes: vec![ShapeParts::term(t0), ShapeParts::term(t1)],
            partials: vec![ShapeParts::partial(0, 2), ShapeParts::partial(1, 0), ShapeParts::partial(1, 1), ShapeParts::partial(1, 4)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "ac",
            reads: vec![ShapeParts::pin(re), ShapeParts::pin(im)],
            writes: vec![ShapeParts::input_cell(i0, part::RE), ShapeParts::input_cell(i0, part::IM)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for AcVoltageSource {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&AC_VSOURCE, AcVoltageSource::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AcVoltageSource, Arc::clone(ShapeParts::shared(&AC_VSOURCE, AcVoltageSource::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            cx.set(0, cx.f64(2));
            cx.set(1, cx.f64(0) - cx.f64(1) - cx.f64(3) - cx.f64(4));
            cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, -1.0]);
        } else {
            cx.set(0, cx.f64(0));
            cx.set(1, cx.f64(1));
        }
        Ok(())
    }
}

// ─── Noise fixtures ─────────────────────────────────────────────────────────

/// The temperature of the noise fixtures, kelvin.
pub const T_NOISE: f64 = 300.15;

/// pins: r=0 (PARAM F64); p=1, n=2 (PORT continuous)
/// inputs: I0 NOISE "thermal"       terms: T0 FLOW {p, n} "i" = v/r + I0
/// fn 0 "i":   reads [p, n, r, I0] writes [T0] partials (0←0)=1/r (0←1)=−1/r (0←2)=−v/r² (0←3)=1
/// fn 1 "psd": reads [r] writes [I0.PSD] = 4·k·T/r
pub struct FixedTemperatureNoisyResistor;
static NOISY: OnceLock<ShapeRef> = OnceLock::new();

impl FixedTemperatureNoisyResistor {
    /// The shape of `FixedTemperatureNoisyResistor`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let r = s.pin(ShapeParts::param("r"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let i0 = s.input(InputSpec { name: "thermal", origin: "fixture:noisy:I0", kind: input::NOISE });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(r), ShapeParts::input_cell(i0, part::VALUE)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1), ShapeParts::partial(0, 2), ShapeParts::partial(0, 3)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "psd",
            reads: vec![ShapeParts::pin(r)],
            writes: vec![ShapeParts::input_cell(i0, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for FixedTemperatureNoisyResistor {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&NOISY, FixedTemperatureNoisyResistor::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FixedTemperatureNoisyResistor, Arc::clone(ShapeParts::shared(&NOISY, FixedTemperatureNoisyResistor::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
            cx.set(0, v / r + cx.f64(3));
            cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r), 1.0]);
        } else {
            cx.set(0, 4.0 * K_B * T_NOISE / cx.f64(0));
        }
        Ok(())
    }
}

/// A resistor split into two parallel halves, each with a thermal source;
/// with `shared` = 1 the second half reads the first half's input with
/// partial −1 (one source on two paths), with `shared` = 0 its own input.
/// pins: r=0, shared=1 (PARAM F64); p=2, n=3 (PORT continuous)
/// inputs: I0 NOISE "a", I1 NOISE "b"
/// terms: T0 FLOW {p, n} "ha" = v/(2r) + I0;  T1 FLOW {p, n} "hb" = v/(2r) − shared·I0 + (1 − shared)·I1
/// fn 0 "i":   reads [p, n, r, shared, I0, I1] writes [T0, T1]
///             partials (0←0) (0←1) (0←4)=1 (1←0) (1←1) (1←4)=−shared (1←5)=1−shared
/// fn 1 "psd": reads [r] writes [I0.PSD, I1.PSD] = 4·k·T/(2r) each
pub struct Split;
static SPLIT: OnceLock<ShapeRef> = OnceLock::new();

impl Split {
    /// The shape of `Split`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let r = s.pin(ShapeParts::param("r"));
        let sh = s.pin(ShapeParts::param("shared"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let i0 = s.input(InputSpec { name: "a", origin: "fixture:split:I0", kind: input::NOISE });
        let i1 = s.input(InputSpec { name: "b", origin: "fixture:split:I1", kind: input::NOISE });
        let t0 = s.term(ShapeParts::flow("ha", op::FLOW, p, n));
        let t1 = s.term(ShapeParts::flow("hb", op::FLOW, p, n));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(n), ShapeParts::pin(r), ShapeParts::pin(sh), ShapeParts::input_cell(i0, part::VALUE), ShapeParts::input_cell(i1, part::VALUE)],
            writes: vec![ShapeParts::term(t0), ShapeParts::term(t1)],
            partials: vec![
                ShapeParts::partial(0, 0),
                ShapeParts::partial(0, 1),
                ShapeParts::partial(0, 4),
                ShapeParts::partial(1, 0),
                ShapeParts::partial(1, 1),
                ShapeParts::partial(1, 4),
                ShapeParts::partial(1, 5),
            ],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "psd",
            reads: vec![ShapeParts::pin(r)],
            writes: vec![ShapeParts::input_cell(i0, part::PSD), ShapeParts::input_cell(i1, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Split {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&SPLIT, Split::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Split, Arc::clone(ShapeParts::shared(&SPLIT, Split::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let (v, r, sh) = (cx.f64(0) - cx.f64(1), cx.f64(2), cx.f64(3));
            let g = 1.0 / (2.0 * r);
            cx.set(0, g * v + cx.f64(4));
            cx.set(1, g * v - sh * cx.f64(4) + (1.0 - sh) * cx.f64(5));
            cx.jac().copy_from_slice(&[g, -g, 1.0, g, -g, -sh, 1.0 - sh]);
        } else {
            let psd = 4.0 * K_B * T_NOISE / (2.0 * cx.f64(0));
            cx.set(0, psd);
            cx.set(1, psd);
        }
        Ok(())
    }
}

/// A flicker source whose density reads `FREQ`.
/// pins: tag=0 (PARAM F64); p=1 (PORT continuous)
/// inputs: I0 NOISE "flicker"       terms: T0 FLOW {p, GROUND} "i" = G_LIMIT·v + I0
/// fn 0 "i":   reads [p, I0] writes [T0] partials (0←0)=G_LIMIT (0←1)=1
/// fn 1 "psd": reads [FREQ] writes [I0.PSD] = KF/freq        (records every call)
pub struct Flicker {
    tag: u64,
}
static FLICKER: OnceLock<ShapeRef> = OnceLock::new();

impl Flicker {
    /// The shape of `Flicker`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(ShapeParts::param("tag"));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let i0 = s.input(InputSpec { name: "flicker", origin: "fixture:flicker:I0", kind: input::NOISE });
        let t0 = s.term(ShapeParts::flow("i", op::FLOW, p, GROUND));
        s.function(FunctionSpec {
            name: "i",
            reads: vec![ShapeParts::pin(p), ShapeParts::input_cell(i0, part::VALUE)],
            writes: vec![ShapeParts::term(t0)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "psd",
            reads: vec![ShapeParts::ambient(ambient::FREQ)],
            writes: vec![ShapeParts::input_cell(i0, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for Flicker {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&FLICKER, Flicker::shape).shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Flicker { tag: RecordedCall::tag_of(values) }, Arc::clone(ShapeParts::shared(&FLICKER, Flicker::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            cx.set(0, G_LIMIT * cx.f64(0) + cx.f64(1));
            cx.jac().copy_from_slice(&[G_LIMIT, 1.0]);
        } else {
            RecordedCall::record(self.tag, cx, 0);
            cx.set(0, KF / cx.f64(0));
        }
        Ok(())
    }
}

// ─── Watch fixtures ─────────────────────────────────────────────────────────

/// Two step-size caps.
/// pins: c1=0, c2=1 (PARAM F64); p=2 (PORT continuous)
/// watches: W0 CAP "a" = c1·(1 + v(p)), W1 CAP "b" = c2
/// fn 0 "caps": reads [p, c1, c2] writes [W0, W1]
pub struct CapWatch;
static CAPPER: OnceLock<ShapeRef> = OnceLock::new();

impl CapWatch {
    /// The `CAP` watch named `name` of the capper fixture.
    fn cap_watch(name: &'static str) -> WatchSpec<'static> {
        WatchSpec { name, origin: "fixture:capper", kind: watch::CAP, layout: &F64, event: 0, ttol: DEFAULT, etol: DEFAULT }
    }

    /// The shape of `CapWatch`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let (c1, c2) = (s.pin(ShapeParts::param("c1")), s.pin(ShapeParts::param("c2")));
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let w0 = s.watch(CapWatch::cap_watch("a"));
        let w1 = s.watch(CapWatch::cap_watch("b"));
        let w = |index| Cell { kind: cell::WATCH, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "caps",
            reads: vec![ShapeParts::pin(p), ShapeParts::pin(c1), ShapeParts::pin(c2)],
            writes: vec![w(w0), w(w1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for CapWatch {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&CAPPER, CapWatch::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CapWatch, Arc::clone(ShapeParts::shared(&CAPPER, CapWatch::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (v, c1, c2) = (cx.f64(0), cx.f64(1), cx.f64(2));
        cx.set(0, c1 * (1.0 + v));
        cx.set(1, c2);
        Ok(())
    }
}

/// A conductance that traces its voltage whenever it exceeds 0.5 V.
/// pins: p=0 (PORT continuous)
/// terms: T0 FLOW {p, GROUND} "g" = v(p)     watches: W0 TRACE "hi" (layout `HI`)
/// fn 0 "g": reads [p] writes [T0, W0] partials (0←0)=1; W0 = {some: 1, v} when v > 0.5,
///           else left empty (all zero, A5)
pub struct ThresholdTracer;
static TRACER: OnceLock<ShapeRef> = OnceLock::new();

static HI_FIELDS: [Field; 2] = [
    Field { name: Str::from_static("some"), leaf: piperine_abi::raw::leaf::BOOL, offset: 0, count: 1 },
    Field { name: Str::from_static("v"), leaf: piperine_abi::raw::leaf::F64, offset: 8, count: 1 },
];
/// The record of `ThresholdTracer`: a `BOOL` presence field, then the traced `F64` (abi.md A5).
pub static HI: Layout = Layout { size: 16, align: 8, fields: piperine_abi::raw::List::from_slice(&HI_FIELDS) };

impl ThresholdTracer {
    /// The shape of `ThresholdTracer`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(ShapeParts::flow("g", op::FLOW, p, GROUND));
        let w0 = s.watch(WatchSpec {
            name: "hi",
            origin: "fixture:tracer:W0",
            kind: watch::TRACE,
            layout: &HI,
            event: 0,
            ttol: DEFAULT,
            etol: DEFAULT,
        });
        s.function(FunctionSpec {
            name: "g",
            reads: vec![ShapeParts::pin(p)],
            writes: vec![ShapeParts::term(t0), Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
            partials: vec![ShapeParts::partial(0, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for ThresholdTracer {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&TRACER, ThresholdTracer::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ThresholdTracer, Arc::clone(ShapeParts::shared(&TRACER, ThresholdTracer::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let v = cx.f64(0);
        cx.set(0, v);
        cx.jac()[0] = 1.0;
        if v > 0.5 {
            let rec = cx.out_bytes(1);
            rec[0] = 1;
            rec[8..16].copy_from_slice(&v.to_le_bytes());
        }
        Ok(())
    }
}

/// A sample-and-hold: a reaction reads a continuous pin.
/// pins: inp=0 (PORT continuous); clk=1 (PORT event); out=2 (PORT F64)
/// reaction 0 "sample": on [clk] reads [inp] writes [S0 {out}]
pub struct SampleHold;
static SAMPLE_HOLD: OnceLock<ShapeRef> = OnceLock::new();

impl SampleHold {
    /// The shape of `SampleHold`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let inp_ = s.pin(PinSpec::continuous("inp", role::PORT));
        let clk = s.pin(PinSpec::event("clk", role::PORT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &F64));
        s.reaction(ReactionSpec {
            name: "sample",
            on: vec![clk],
            reads: vec![ShapeParts::pin(inp_)],
            writes: vec![Statement { pin: out, offset: 0, len: 8, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }
}

impl Device for SampleHold {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&SAMPLE_HOLD, SampleHold::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SampleHold, Arc::clone(ShapeParts::shared(&SAMPLE_HOLD, SampleHold::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &cx.f64(0).to_le_bytes(), 0.0)?;
        Ok(Flow::Continue)
    }
}

// ─── A limited junction with a nonlinear charge ─────────────────────────────

/// A diode whose diffusion charge `tt·id` is nonlinear in the limited voltage.
/// pins:   is=0, n=1, tt=2 (PARAM F64); a=3, k=4 (PORT); temperature=5 (GLOBAL TEMPERATURE)
/// inputs: I0 LIMIT "vd"     terms: T0 FLOW{a,k} "id", T1 DDT{a,k} "q" = tt·id
/// fn 0 "limit":   reads [a, k, is, n, temperature, I0.LAST, I0.AGE, STATIC]
///                 writes [I0.PROPOSED, I0.LIMITED]      partials (0←0)=1 (0←1)=−1
/// fn 1 "current": reads [I0, is, n, tt, temperature]    writes [T0, T1]
///                 partials (0←0) (1←0)
pub struct ChargeDiode;
static CHARGE_DIODE: OnceLock<ShapeRef> = OnceLock::new();

impl ChargeDiode {
    /// The shape of `ChargeDiode`, built once and shared by every instance.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let is = s.pin(ShapeParts::param("is"));
        let n = s.pin(ShapeParts::param("n"));
        let tt = s.pin(ShapeParts::param("tt"));
        let a = s.pin(PinSpec::continuous("a", role::PORT));
        let k = s.pin(PinSpec::continuous("k", role::PORT));
        let t = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temperature", role::GLOBAL, &F64) });
        let i0 = s.input(InputSpec { name: "vd", origin: "abi.md:11.2", kind: input::LIMIT });
        let t0 = s.term(ShapeParts::flow("id", op::FLOW, a, k));
        let t1 = s.term(ShapeParts::flow("q", op::DDT, a, k));
        s.function(FunctionSpec {
            name: "limit",
            reads: vec![ShapeParts::pin(a), ShapeParts::pin(k), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(t), ShapeParts::input_cell(i0, part::LAST), ShapeParts::input_cell(i0, part::AGE), ShapeParts::ambient(ambient::STATIC)],
            writes: vec![ShapeParts::input_cell(i0, part::PROPOSED), ShapeParts::input_cell(i0, part::LIMITED)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "current",
            reads: vec![ShapeParts::input_cell(i0, part::VALUE), ShapeParts::pin(is), ShapeParts::pin(n), ShapeParts::pin(tt), ShapeParts::pin(t)],
            writes: vec![ShapeParts::term(t0), ShapeParts::term(t1)],
            partials: vec![ShapeParts::partial(0, 0), ShapeParts::partial(1, 0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}

impl Device for ChargeDiode {
    fn prototype() -> &'static Shape<'static> {
        ShapeParts::shared(&CHARGE_DIODE, ChargeDiode::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((ChargeDiode, Arc::clone(ShapeParts::shared(&CHARGE_DIODE, ChargeDiode::shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let nvt = cx.f64(3) * K_B * cx.f64(4) / Q;
            let vcrit = nvt * (nvt / (SQRT_2 * cx.f64(2))).ln();
            let v = cx.f64(0) - cx.f64(1);
            let (age, is_static) = (cx.u8(6), cx.bool(7));
            // ngspice's INITJCT: the first evaluation, or the first of an equilibrium solve, starts at
            // the seed itself, unlimited; every later one limits from `LAST`.
            let limited = if age == 0 || (is_static && age == 1) { vcrit } else { LimitingDiode::limit_junction(v, cx.f64(5), nvt, vcrit) };
            cx.set(0, v);
            cx.set(1, limited);
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        } else {
            let (vd, is, tt) = (cx.f64(0), cx.f64(1), cx.f64(3));
            let nvt = cx.f64(2) * K_B * cx.f64(4) / Q;
            let e = (vd / nvt).exp();
            cx.set(0, is * (e - 1.0));
            cx.set(1, tt * is * (e - 1.0));
            cx.jac().copy_from_slice(&[is * e / nvt, tt * is * e / nvt]);
        }
        Ok(())
    }
}

impl FixtureModules {
    // ─── 11.4 D flip-flop ───────────────────────────────────────────────────────

    /// pins: clk=0, d=1 (PORT BIT);  q=2 (PORT BIT, init LOW);  rise=3 (PRIVATE event)
    /// fn 0 "rise":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]


    /// One module per fixture type, built once.
    pub fn all() -> FixtureModules {
        static M: [OnceLock<Module>; 29] = [const { OnceLock::new() }; 29];
        FixtureModules {
            resistor: Resistor::module(),
            diode: ShapeParts::module_of::<LimitingDiode>(&M[1], "Diode"),
            capacitor: Capacitor::module(),
            flip_flop: DataFlipFlop::module(),
            comparator: Comparator::module(),
            digital_to_analog: ShapeParts::module_of::<DigitalToAnalog>(&M[5], "Dac"),
            cubic: ShapeParts::module_of::<Cubic>(&M[6], "Cubic"),
            bad_limit: M[7].get_or_init(BadLimit::module),
            source: ShapeParts::module_of::<CurrentSource>(&M[8], "CurrentSource"),
            counted: ShapeParts::module_of::<CountedResistor>(&M[9], "CountedResistor"),
            probe: ShapeParts::module_of::<RecordingConductance>(&M[10], "Probe"),
            faulty: ShapeParts::module_of::<ModeFaulty>(&M[11], "ModeFaulty"),
            plain_diode: ShapeParts::module_of::<PlainDiode>(&M[12], "PlainDiode"),
            voltage_source: VoltageSource::module(),
            clamp: ShapeParts::module_of::<Clamp>(&M[14], "Clamp"),
            owner: ShapeParts::module_of::<CorrectionOwner>(&M[15], "Owner"),
            reader: ShapeParts::module_of::<LimitReader>(&M[16], "Reader"),
            tracker: ShapeParts::module_of::<Tracker>(&M[17], "Tracker"),
            delay_line: ShapeParts::module_of::<DelayLine>(&M[18], "DelayLine"),
            trio: ShapeParts::module_of::<Trio>(&M[19], "Trio"),
            ac_voltage_source: ShapeParts::module_of::<AcVoltageSource>(&M[20], "AcVSource"),
            noisy: ShapeParts::module_of::<FixedTemperatureNoisyResistor>(&M[21], "FixedTemperatureNoisyResistor"),
            split: ShapeParts::module_of::<Split>(&M[22], "Split"),
            flicker: ShapeParts::module_of::<Flicker>(&M[23], "Flicker"),
            cap_watch_device: ShapeParts::module_of::<CapWatch>(&M[24], "Capper"),
            tracer: ShapeParts::module_of::<ThresholdTracer>(&M[25], "ThresholdTracer"),
            sample_hold: ShapeParts::module_of::<SampleHold>(&M[26], "SampleHold"),
            charge_diode: ShapeParts::module_of::<ChargeDiode>(&M[27], "ChargeDiode"),
            tight_capacitor: ShapeParts::module_of::<TightCapacitor>(&M[28], "TightCapacitor"),
        }
    }
}

impl RecordedCall {
    /// Appends to the recorded calls one call tagged `tag`: the bytes of every read and which of its `partials` were seeded.
    fn record(tag: u64, cx: &Eval, partials: usize) {
        let call = RecordedCall {
            tag,
            reads: (0..cx.reads()).map(|i| cx.bytes(i).to_vec()).collect(),
            seed: (0..partials).map(|p| cx.wanted(p)).collect(),
        };
        CALLS.lock().unwrap_or_else(|e| e.into_inner()).push(call);
    }

    /// Every recorded call of the fixture tagged `tag`, in order. Each test uses
    /// its own tags, so parallel tests do not see each other's calls.
    pub fn recorded(tag: u64) -> Vec<RecordedCall> {
        CALLS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|c| c.tag == tag).cloned().collect()
    }

    /// The tag a recording fixture was created with (its `tag` param, pin 0).
    fn tag_of(values: &Values<'_>) -> u64 {
        values.f64(0).map_or(0, |t| t as u64)
    }
}
