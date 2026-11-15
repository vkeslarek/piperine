//! A module that carries its data (feature `ir-interp`, INT-08):
//! `thunk::module_with(name, &'static factory)` returns a `Module` whose
//! `data` is the factory and whose prototype is the factory's; its `create`
//! goes through the factory with the same value checks and shape validation
//! as `module::<D>`; its `eval`/`react` run the instance the factory made,
//! a panic or a `Fault` being a `FAULT` with no unwinding; `destroy` drops the
//! instance. One factory type, two modules with different data, two results.

use std::cell::Cell as StdCell;
use std::ffi::c_void;

use piperine_abi::caller::{Commit, Handle};
use piperine_abi::device::{Eval, Factory, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, cell, op, part, pin_flag, role, status,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module_with;

thread_local! {
    static DROPS: StdCell<usize> = const { StdCell::new(0) };
}

const ONE: [u8; 8] = 1.0f64.to_le_bytes();

/// pins: scale=0 (PARAM F64, default 1.0), x=1 (PORT continuous), q=2 (PORT BIT), e=3 (PRIVATE event)
/// term T0 FLOW {x, GROUND} (T0 FLOW {99, GROUND} when `broken`: an invalid shape)
/// fn 0 "out": reads [x, scale] writes [T0] partials (0←0) (0←1): k·scale·x
/// fn 1 "boom": panics.   fn 2 "fault": returns a Fault with code 17.
/// reaction 0 "drive": on [e] writes [S0 {q}]: commits HIGH after k ns
/// reaction 1 "stop": on [e] returns Finish.   reaction 2 "boom": panics.
fn gain_shape(broken: bool) -> ShapeRef {
    let mut s = ShapeBuf::new();
    let scale = s.pin(PinSpec {
        flags: pin_flag::HAS_INIT,
        init: &ONE,
        ..PinSpec::discrete("scale", role::PARAM, &F64)
    });
    let x = s.pin(PinSpec::continuous("x", role::PORT));
    let q = s.pin(PinSpec::discrete("q", role::PORT, &BIT));
    let e = s.pin(PinSpec::event("e", role::PRIVATE));
    let plus = if broken { 99 } else { x };
    let t0 = s.term(TermSpec { name: "out", origin: "", op: op::FLOW, plus, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
    let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
    s.function(FunctionSpec {
        name: "out",
        reads: vec![pin(x), pin(scale)],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec { name: "boom", ..FunctionSpec::default() });
    s.function(FunctionSpec { name: "fault", ..FunctionSpec::default() });
    s.reaction(ReactionSpec {
        name: "drive",
        on: vec![e],
        writes: vec![Statement { pin: q, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.reaction(ReactionSpec { name: "stop", on: vec![e], ..ReactionSpec::default() });
    s.reaction(ReactionSpec { name: "boom", on: vec![e], ..ReactionSpec::default() });
    s.freeze()
}

/// The factory: one type, its gain `k` a value.
struct Gain {
    k: f64,
    broken: bool,
    proto: ShapeRef,
}

/// What `Gain::create` makes.
struct Scaled {
    k: f64,
    scale: f64,
}

impl Drop for Scaled {
    fn drop(&mut self) {
        DROPS.with(|d| d.set(d.get() + 1));
    }
}

impl Gain {
    fn leak(k: f64, broken: bool) -> &'static Gain {
        Box::leak(Box::new(Gain { k, broken, proto: gain_shape(false) }))
    }

    fn module(name: &'static str, k: f64) -> (&'static Gain, &'static Module) {
        let g = Gain::leak(k, false);
        (g, Box::leak(Box::new(module_with(name, g))))
    }
}

impl Factory for Gain {
    type Instance = Scaled;

    fn prototype(&self) -> &Shape<'_> {
        self.proto.shape()
    }

    fn create(&self, values: &Values<'_>) -> Result<(Scaled, ShapeRef), Fault> {
        let scale = values.f64(0).unwrap_or(1.0);
        Ok((Scaled { k: self.k, scale }, gain_shape(self.broken)))
    }

    fn eval(instance: &Scaled, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        match k {
            0 => {
                let (x, scale) = (cx.f64(0), cx.f64(1));
                assert_eq!(scale, instance.scale, "the param net holds the created value");
                cx.set(0, instance.k * scale * x);
                cx.jac().copy_from_slice(&[instance.k * scale, instance.k * x]);
                Ok(())
            }
            1 => panic!("gain {} panicked on purpose", instance.k),
            _ => Err(Fault::new(17, format!("gain {} faulted", instance.k))),
        }
    }

    fn react(instance: &Scaled, k: u32, cx: &mut React<'_>) -> Result<Flow, Fault> {
        match k {
            0 => {
                cx.commit(0, &HIGH, instance.k * 1e-9)?;
                Ok(Flow::Continue)
            }
            1 => Ok(Flow::Finish),
            _ => panic!("reaction of gain {} panicked on purpose", instance.k),
        }
    }
}

/// Runs function 0 at `x` with the given scale read; returns (output, jac).
fn out(h: &Handle, x: f64, scale: f64) -> (f64, [f64; 2]) {
    let (xb, sb) = (x.to_le_bytes(), scale.to_le_bytes());
    let mut o = [0u8; 8];
    let mut jac = [0.0; 2];
    h.eval(0, &[&xb, &sb], &mut [&mut o], &[1, 1], &mut jac).expect("function 0 runs");
    (f64::from_le_bytes(o), jac)
}

#[test]
fn module_with_carries_the_factory() {
    let (g2, m2) = Gain::module("gain2", 2.0);
    let (g5, m5) = Gain::module("gain5", 5.0);
    assert_eq!(m2.data(), (g2 as *const Gain).cast::<c_void>(), "data is the factory");
    assert_eq!(m5.data(), (g5 as *const Gain).cast::<c_void>(), "data is the factory");
    assert_ne!(m2.data(), m5.data());
    assert_eq!(m2.prototype_ptr(), g2.prototype() as *const Shape<'_> as *const Shape<'static>);
    assert_eq!(m5.prototype_ptr(), g5.prototype() as *const Shape<'_> as *const Shape<'static>);
    assert_eq!(m2.name().as_str(), "gain2");
    assert_eq!(m5.name().as_str(), "gain5");
    assert_eq!(m2.prototype().pins.len(), 4);
    assert_eq!(m2.prototype().pins.get(0).map(|p| p.role), Some(role::PARAM));
}

#[test]
fn create_goes_through_the_factory_and_validates() {
    let (_, m) = Gain::module("gain3", 3.0);

    // The values reach the factory's `create`: scale 4 instead of the default 1.
    let h = Handle::create(m, &[(0, &4.0f64.to_le_bytes())]).expect("create with a value");
    assert_eq!(out(&h, 0.5, 4.0), (6.0, [12.0, 1.5]), "k·scale·x with k = 3, scale = 4");
    let d = Handle::create(m, &[]).expect("create at defaults");
    assert_eq!(out(&d, 0.5, 1.0), (1.5, [3.0, 1.5]), "k·scale·x with the default scale 1");

    // The value checks of `module::<D>`.
    let err = Handle::create(m, &[(1, &[0u8; 8])]).expect_err("a value for a port");
    assert_eq!(err.status, status::FAULT);
    assert!(err.message.contains("not a PARAM pin"), "{}", err.message);
    let err = Handle::create(m, &[(0, &[0u8; 4])]).expect_err("a value of the wrong length");
    assert_eq!(err.status, status::FAULT);
    assert!(err.message.contains("has 4 bytes, its layout 8"), "{}", err.message);

    // The created shape is validated.
    let broken = Gain::leak(3.0, true);
    let m = Box::leak(Box::new(module_with("broken", broken)));
    let err = Handle::create(m, &[]).expect_err("an invalid created shape");
    assert_eq!(err.status, status::FAULT);
    assert!(err.message.starts_with("the created shape is invalid"), "{}", err.message);
}

#[test]
fn eval_and_react_run_the_created_instance() {
    let (_, m2) = Gain::module("gain2", 2.0);
    let (_, m5) = Gain::module("gain5", 5.0);
    let before = DROPS.with(|d| d.get());
    {
        let a = Handle::create(m2, &[]).expect("create");
        let mut b = Handle::create(m5, &[]).expect("create");

        // One device type, two results: each module evaluates with its own k.
        assert_eq!(out(&a, 1.5, 1.0), (3.0, [2.0, 3.0]));
        assert_eq!(out(&b, 1.5, 1.0), (7.5, [5.0, 7.5]));

        // A panic in the factory's eval is a FAULT with its message; nothing unwinds.
        let err = a.eval(1, &[], &mut [], &[], &mut []).expect_err("a panic");
        assert_eq!(err.status, status::FAULT);
        assert_eq!(err.message, "panic: gain 2 panicked on purpose");
        let err = b.eval(2, &[], &mut [], &[], &mut []).expect_err("a fault");
        assert_eq!((err.status, err.code, err.message.as_str()), (status::FAULT, 17, "gain 5 faulted"));
        assert_eq!(out(&b, 1.0, 1.0), (5.0, [5.0, 5.0]), "the instance survives the panic and the fault");

        // React runs the created instance: the commit carries its own k.
        let mut seen = Vec::new();
        let mut host = |c: Commit<'_>| {
            seen.push((c.statement, c.bytes.to_vec(), c.after));
            status::OK
        };
        assert_eq!(b.react(0, &[], &mut host), Ok(Flow::Continue));
        assert_eq!(b.react(1, &[], &mut host), Ok(Flow::Finish));
        let err = b.react(2, &[], &mut host).expect_err("a panic in react");
        assert_eq!(err.status, status::FAULT);
        assert_eq!(err.message, "panic: reaction of gain 5 panicked on purpose");
        assert_eq!(seen, vec![(0, HIGH.to_vec(), 5e-9)]);
        assert_eq!(DROPS.with(|d| d.get()), before, "no instance dropped while its handle lives");
    }
    assert_eq!(DROPS.with(|d| d.get()), before + 2, "destroy drops both instances");
}
