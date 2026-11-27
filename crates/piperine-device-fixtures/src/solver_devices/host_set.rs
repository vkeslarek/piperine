//! The devices of the solver target `host_set`, one type per device.

use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "host_set.rs";
const KILOHM: [u8; 8] = 1.0e3_f64.to_le_bytes();

// ---------------------------------------------------------------------------
// CheckedR: a resistor whose `r` has `where r > 0`
// ---------------------------------------------------------------------------

/// pins: r=0 (PARAM F64, default 1 kΩ, check -> ok); p=1, n=2 (PORT); ok=3 (PRIVATE BIT)
/// terms: T0 FLOW{p, n}
/// fn 0 "where": reads [r] writes [ok]; ok = r > 0
/// fn 1 "i": reads [p, n, r] writes [T0] partials (0<-0) (0<-1); g = 1/r
pub struct CheckedR;
static CHECKED_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CHECKED: OnceLock<Module> = OnceLock::new();

impl Device for CheckedR {
    fn prototype() -> &'static Shape<'static> {
        CHECKED_SHAPE.get_or_init(HostSetShapes::checked_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((CheckedR, Arc::clone(CHECKED_SHAPE.get_or_init(HostSetShapes::checked_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let ok = if cx.f64(0) > 0.0 { HIGH } else { LOW };
            cx.out_bytes(0).copy_from_slice(&ok);
            return Ok(());
        }
        let g = 1.0 / cx.f64(2);
        cx.set(0, g * cx.f64(0) - g * cx.f64(1));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Collapsible: a resistor with an internal node above a threshold
// ---------------------------------------------------------------------------

const THRESHOLD: f64 = 100.0;

/// At `r >= THRESHOLD`: two halves `r/2` in series through a private node `m`;
/// below it the node collapses: one resistor `r`. Each shape's setup function
/// asks for the other shape when `r` crosses the threshold (`RESHAPE`).
///
/// split pins: r=0 (PARAM F64, default 1 kΩ); p=1, n=2 (PORT); big=3 (PRIVATE BIT); m=4 (PRIVATE)
///   terms T0 FLOW{p, m}, T1 FLOW{m, n}; fn 0 "setup": reads [r] writes [big];
///   fn 1 "i": reads [p, m, n, r] writes [T0, T1] partials (0<-0) (0<-1) (1<-1) (1<-2)
/// whole pins: r=0; p=1, n=2; big=3; terms T0 FLOW{p, n}; fn 0 "setup"; fn 1 "i": reads [p, n, r]
pub struct Collapsible {
    split: bool,
}
/// The shape `Collapsible` takes when `r` is at or above its threshold: the `big` pin and the middle node `m` exist.
pub static SPLIT_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
/// The shape `Collapsible` takes when `r` is below its threshold: one resistor with no middle node.
pub static WHOLE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COLLAPSIBLE: OnceLock<Module> = OnceLock::new();

impl Device for Collapsible {
    fn prototype() -> &'static Shape<'static> {
        HostSetShapes::split_shape().shape()
    }
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let split = values.f64(0).unwrap_or(1.0e3) >= THRESHOLD;
        let shape = if split { HostSetShapes::split_shape() } else { HostSetShapes::whole_shape() };
        Ok((Collapsible { split }, Arc::clone(shape)))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            if (cx.f64(0) >= THRESHOLD) != self.split {
                return Err(Fault::reshape("r crossed the threshold"));
            }
            cx.out_bytes(0).copy_from_slice(&HIGH);
            return Ok(());
        }
        if self.split {
            let g = 2.0 / cx.f64(3);
            cx.set(0, g * cx.f64(0) - g * cx.f64(1));
            cx.set(1, g * cx.f64(1) - g * cx.f64(2));
            cx.jac().copy_from_slice(&[g, -g, g, -g]);
        } else {
            let g = 1.0 / cx.f64(2);
            cx.set(0, g * cx.f64(0) - g * cx.f64(1));
            cx.jac().copy_from_slice(&[g, -g]);
        }
        Ok(())
    }
}
impl CheckedR {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        CHECKED.get_or_init(|| module::<CheckedR>("CheckedR"))
    }
}

impl Collapsible {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        COLLAPSIBLE.get_or_init(|| module::<Collapsible>("Collapsible"))
    }
}

/// The shapes the `host_set` devices are built from.
pub struct HostSetShapes;

impl HostSetShapes {
    /// The cell of the value of pin `index`.
    pub fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The cell of the value of term `index`.
    pub fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The partial of write `write` against read `read`: the first seed, against nothing else.
    pub fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The term spec named `name` of operator `operator`, between pins `plus` and `minus`.
    pub fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The shape of `CheckedR`, built once and shared by every instance.
    pub fn checked_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec {
            flags: pin_flag::HAS_INIT | pin_flag::HAS_CHECK,
            init: &KILOHM,
            check: 3,
            ..PinSpec::discrete("r", role::PARAM, &F64)
        });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let ok = s.pin(PinSpec::discrete("ok", role::PRIVATE, &BIT));
        let t0 = s.term(HostSetShapes::flow("i", p, n));
        s.function(FunctionSpec { name: "where", origin: ORIGIN, reads: vec![HostSetShapes::pin(0)], writes: vec![HostSetShapes::pin(ok)], ..FunctionSpec::default() });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![HostSetShapes::pin(p), HostSetShapes::pin(n), HostSetShapes::pin(0)],
            writes: vec![HostSetShapes::term(t0)],
            partials: vec![HostSetShapes::partial(0, 0), HostSetShapes::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of the collapsible fixture, built once and shared by every instance.
    pub fn collapsible_shape(split: bool) -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &KILOHM, ..PinSpec::discrete("r", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let big = s.pin(PinSpec::discrete("big", role::PRIVATE, &BIT));
        s.function(FunctionSpec { name: "setup", origin: ORIGIN, reads: vec![HostSetShapes::pin(0)], writes: vec![HostSetShapes::pin(big)], ..FunctionSpec::default() });
        if split {
            let m = s.pin(PinSpec::continuous("m", role::PRIVATE));
            let t0 = s.term(HostSetShapes::flow("a", p, m));
            let t1 = s.term(HostSetShapes::flow("b", m, n));
            s.function(FunctionSpec {
                name: "i",
                origin: ORIGIN,
                reads: vec![HostSetShapes::pin(p), HostSetShapes::pin(m), HostSetShapes::pin(n), HostSetShapes::pin(0)],
                writes: vec![HostSetShapes::term(t0), HostSetShapes::term(t1)],
                partials: vec![HostSetShapes::partial(0, 0), HostSetShapes::partial(0, 1), HostSetShapes::partial(1, 1), HostSetShapes::partial(1, 2)],
                ..FunctionSpec::default()
            });
        } else {
            let t0 = s.term(HostSetShapes::flow("a", p, n));
            s.function(FunctionSpec {
                name: "i",
                origin: ORIGIN,
                reads: vec![HostSetShapes::pin(p), HostSetShapes::pin(n), HostSetShapes::pin(0)],
                writes: vec![HostSetShapes::term(t0)],
                partials: vec![HostSetShapes::partial(0, 0), HostSetShapes::partial(0, 1)],
                ..FunctionSpec::default()
            });
        }
        s.freeze()
    }

    /// The shape of `Collapsible`, built once and shared by every instance.
    pub fn split_shape() -> &'static ShapeRef {
        SPLIT_SHAPE.get_or_init(|| HostSetShapes::collapsible_shape(true))
    }

    /// The shape of `Collapsible`, built once and shared by every instance.
    pub fn whole_shape() -> &'static ShapeRef {
        WHOLE_SHAPE.get_or_init(|| HostSetShapes::collapsible_shape(false))
    }
}
