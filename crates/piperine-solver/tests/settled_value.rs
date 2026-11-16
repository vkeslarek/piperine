//! One settled value per microstep on the solver (feature `solver-crossings`, DSV-01 AC 4;
//! docs/spec Part III §8.2.3, Part IV §12.1.2, §12.1.18): a reaction that commits `x <- HIGH;
//! x <- LOW;` to a net holding `LOW` leaves it as it was, so a function reading `x.CHANGED`
//! fires nothing. The pure-ABI repro of `ir-rust` T42.

#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::cell::Cell as StdCell;
use std::sync::{Arc, OnceLock};

use abi_mixed::delay;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, HIGH, LOW};
use piperine_abi::raw::{Cell, Module, Shape, Statement, cell, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};

thread_local! {
    static FIRED: StdCell<u32> = const { StdCell::new(0) };
}

const ORIGIN: &str = "settled_value.rs";

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

/// pins: x=0 (PORT BIT, init LOW); go=1 (PORT event)
/// reaction 0: on [go] writes [S0 {x}, S1 {x}]; x <- HIGH; x <- LOW, no delay
struct TwoWrites;
static TW_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TW: OnceLock<Module> = OnceLock::new();

fn tw_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let x = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("x", role::PORT, &BIT) });
    let go = s.pin(PinSpec::event("go", role::PORT));
    let st = Statement { pin: x, offset: 0, len: 2, flags: 0, min_after: 0.0 };
    s.reaction(ReactionSpec { name: "two", origin: ORIGIN, on: vec![go], writes: vec![st, st], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for TwoWrites {
    fn prototype() -> &'static Shape<'static> {
        TW_SHAPE.get_or_init(tw_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TwoWrites, Arc::clone(TW_SHAPE.get_or_init(tw_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        cx.commit(1, &LOW, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// pins: x=0 (PORT BIT); flip=1 (PRIVATE event)
/// fn 0: reads [x.CHANGED] writes [flip.CHANGED];  reaction 0: on [flip] counts
struct Watch;
static W_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static W: OnceLock<Module> = OnceLock::new();

fn w_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let x = s.pin(PinSpec::discrete("x", role::PORT, &BIT));
    let flip = s.pin(PinSpec::event("flip", role::PRIVATE));
    s.function(FunctionSpec {
        name: "edge",
        origin: ORIGIN,
        reads: vec![Cell { kind: cell::PIN, index: x, part: part::CHANGED }],
        writes: vec![Cell { kind: cell::PIN, index: flip, part: part::CHANGED }],
        ..FunctionSpec::default()
    });
    s.reaction(ReactionSpec { name: "count", origin: ORIGIN, on: vec![flip], ..ReactionSpec::default() });
    s.freeze()
}

impl Device for Watch {
    fn prototype() -> &'static Shape<'static> {
        W_SHAPE.get_or_init(w_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Watch, Arc::clone(W_SHAPE.get_or_init(w_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let changed = cx.bytes(0) == [1];
        cx.set(0, changed as u8);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        FIRED.with(|f| f.set(f.get() + 1));
        Ok(Flow::Continue)
    }
}

#[test]
fn x_changed_does_not_fire_when_the_settled_value_is_unchanged() {
    let mut b = CircuitBuilder::new("settled");
    b.device(delay(), "kick", &[("at", ParamValue::Real(1.0e-7))], &[net("go")]);
    b.device(TW.get_or_init(|| module::<TwoWrites>("TwoWrites")), "tw", &[], &[net("x"), net("go")]);
    b.device(W.get_or_init(|| module::<Watch>("Watch")), "w", &[], &[net("x")]);
    let mut solver =
        Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8)).build();
    solver.tran().expect("tran").solve().expect("solve");
    // `x` changes once, at (0, 0), when its init LOW is committed over the zeroed store (Part IV
    // §13: the initial value is a change). At 100 ns `x <- HIGH; x <- LOW;` settles to LOW, the
    // value it held: no change, so `x.changed()` fires no second time.
    assert_eq!(FIRED.with(|f| f.get()), 1, "x.changed() fired although the settled value is the one before the microstep");
    let bound = solver.circuit().bound().unwrap();
    assert_eq!(bound.digital.read(bound.names.nets["net:x"]), LOW, "the later statement wins");
}

/// Every function of this target's own devices passes the partials verifier (the guard of
/// `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("TwoWrites", TW.get_or_init(|| module::<TwoWrites>("TwoWrites")));
    partials::verify_every_function("Watch", W.get_or_init(|| module::<Watch>("Watch")));
}
