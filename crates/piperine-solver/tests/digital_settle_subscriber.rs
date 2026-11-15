//! T23 (DVT-27) on the `Device` trait — the settle subscriber: a reaction on
//! the `SETTLE` global runs once per time point, strictly after the instant
//! reached quiescence, so it reads the settled value of a net and never a
//! value from before the instant:
//! - a fixture whose settled value differs from the one before the instant
//!   (`c = a OR NOT(NOT(a))`): the subscriber sees the settled one;
//! - a classic AND-gate glitch fixture (`c = a AND NOT(a)`) whose settled value
//!   is the same before and after: the subscriber sees only that value.
//!
//! The premise of the second fixture in the original (that `c` really reads
//! `One` for an intermediate delta cycle) was about the two-phase engine's
//! delta cycles, which a reaction cannot observe; it is not asserted here.
//!
//! The two structural claims of the original (`EventKind` keeps its four
//! variants; `on_digital_settled` has one call site) were about the removed
//! per-hook contract and its event queue; they are listed in `removed.txt`.

#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use std::cell::RefCell;
use std::result::Result;
use std::sync::{Arc, OnceLock};

use abi_mixed::{delay, inverter};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, HIGH, LOW};
use piperine_abi::raw::{Cell, Module, Shape, Statement, ambient, cell, global, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Solver, TransientAnalysisOptions};

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

thread_local! {
    /// `(time, value)` of every `SETTLE` the subscriber saw on this thread.
    static SETTLED: RefCell<Vec<(f64, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

/// Drives `a` from `LOW` to `HIGH` when `go` fires.
/// pins: a=0 (PORT BIT, init LOW); go=1 (PORT event); reaction 0: on [go] writes [S0 {a}]
struct Setter;
static SETTER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SETTER: OnceLock<Module> = OnceLock::new();

fn setter_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &LOW, ..PinSpec::discrete("a", role::PORT, &BIT) });
    let go = s.pin(PinSpec::event("go", role::PORT));
    s.reaction(ReactionSpec {
        name: "flip",
        origin: "digital_settle_subscriber.rs",
        on: vec![go],
        writes: vec![Statement { pin: a, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Setter {
    fn prototype() -> &'static Shape<'static> {
        SETTER_SHAPE.get_or_init(setter_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Setter, Arc::clone(SETTER_SHAPE.get_or_init(setter_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &HIGH, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// A two-input gate: `AND` or `OR`.
/// pins: a=0, b=1, y=2 (PORT BIT); fn 0: reads [a, b] writes [y]
struct Gate<const AND: bool>;
static AND_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static AND_MODULE: OnceLock<Module> = OnceLock::new();
static OR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OR_MODULE: OnceLock<Module> = OnceLock::new();

fn gate_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
    let b = s.pin(PinSpec::discrete("b", role::PORT, &BIT));
    let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
    s.function(FunctionSpec {
        name: "gate",
        origin: "digital_settle_subscriber.rs",
        reads: vec![pin(a), pin(b)],
        writes: vec![pin(y)],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl<const AND: bool> Device for Gate<AND> {
    fn prototype() -> &'static Shape<'static> {
        let slot = if AND { &AND_SHAPE } else { &OR_SHAPE };
        slot.get_or_init(gate_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let slot = if AND { &AND_SHAPE } else { &OR_SHAPE };
        Ok((Gate::<AND>, Arc::clone(slot.get_or_init(gate_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let y = if AND {
            if a == LOW || b == LOW { LOW } else if a == HIGH && b == HIGH { HIGH } else { [0, 0] }
        } else if a == HIGH || b == HIGH {
            HIGH
        } else if a == LOW && b == LOW {
            LOW
        } else {
            [0, 0]
        };
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// Records the value of `c` at every `SETTLE`.
/// pins: settle=0 (GLOBAL SETTLE event); c=1 (PORT BIT)
/// reaction 0 "settled": on [settle] reads [TIME, c]
struct Subscriber;
static SUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUB: OnceLock<Module> = OnceLock::new();

fn sub_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let settle = s.pin(PinSpec { global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) });
    let c = s.pin(PinSpec::discrete("c", role::PORT, &BIT));
    s.reaction(ReactionSpec {
        name: "settled",
        origin: "digital_settle_subscriber.rs",
        on: vec![settle],
        reads: vec![Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }, pin(c)],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Subscriber {
    fn prototype() -> &'static Shape<'static> {
        SUB_SHAPE.get_or_init(sub_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Subscriber, Arc::clone(SUB_SHAPE.get_or_init(sub_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let (t, c) = (cx.f64(0), cx.bytes(1).to_vec());
        SETTLED.with(|s| s.borrow_mut().push((t, c)));
        Ok(Flow::Continue)
    }
}

const FLIP_AT: f64 = 1.0e-7;

fn start(b: &mut CircuitBuilder) {
    b.device(delay(), "kick", &[("at", ParamValue::Real(FLIP_AT))], &[net("go")]);
    b.device(SETTER.get_or_init(|| module::<Setter>("Setter")), "driver", &[], &[net("a"), net("go")]);
}

fn subscribe(b: &mut CircuitBuilder) {
    b.device(SUB.get_or_init(|| module::<Subscriber>("Subscriber")), "subscriber", &[], &[net("c")]);
}

/// A transient that stops at the flip: the instant of the flip is the last one,
/// and the `SETTLE` of the stop time follows its quiescence.
fn run(circuit: CircuitInstance) {
    SETTLED.with(|s| s.borrow_mut().clear());
    let mut solver = Solver::new(circuit).with_tran_opts(TransientAnalysisOptions::new(FLIP_AT, 1.0e-8)).build();
    solver.tran().expect("transient").solve().expect("runs");
}

fn settled_at(time: f64) -> Vec<Vec<u8>> {
    SETTLED.with(|s| s.borrow().iter().filter(|(t, _)| (t - time).abs() < 1.0e-12).map(|(_, c)| c.clone()).collect())
}

fn settled_before(time: f64) -> Vec<Vec<u8>> {
    SETTLED.with(|s| s.borrow().iter().filter(|(t, _)| *t < time - 1.0e-12).map(|(_, c)| c.clone()).collect())
}

#[test]
fn the_subscriber_sees_the_post_settle_value_even_when_it_differs_from_the_pre_accept_snapshot() {
    // c = a OR NOT(NOT(a)): before the flip c = LOW, after the flip and the
    // two inverter stages' settle c = HIGH.
    let mut b = CircuitBuilder::new("before_after");
    start(&mut b);
    b.device(inverter(), "inv1", &[], &[net("a"), net("b1")]);
    b.device(inverter(), "inv2", &[], &[net("b1"), net("b2")]);
    b.device(OR_MODULE.get_or_init(|| module::<Gate<false>>("Or2")), "or", &[], &[net("a"), net("b2"), net("c")]);
    subscribe(&mut b);
    run(b.build().expect("build"));

    assert!(settled_before(FLIP_AT).iter().all(|c| *c == LOW), "c starts LOW (a = LOW OR b2 = LOW) before the flip");
    let settled = settled_at(FLIP_AT);
    assert_eq!(settled.len(), 1, "SETTLE fires exactly once for this one settle: {settled:?}");
    assert_eq!(
        settled[0], HIGH,
        "the subscriber must see c's SETTLED value (HIGH, once a = HIGH propagates through both inverters), \
         not the value before the instant (LOW)"
    );
}

#[test]
fn the_subscriber_sees_only_the_settled_value_never_the_mid_delta_cycle_glitch() {
    // c = a AND NOT(a): always LOW once settled, though it may read HIGH for
    // an intermediate round.
    let mut b = CircuitBuilder::new("glitch");
    start(&mut b);
    b.device(inverter(), "inv", &[], &[net("a"), net("nb")]);
    b.device(AND_MODULE.get_or_init(|| module::<Gate<true>>("And2")), "and", &[], &[net("a"), net("nb"), net("c")]);
    subscribe(&mut b);
    run(b.build().expect("build"));

    let settled = settled_at(FLIP_AT);
    assert_eq!(settled.len(), 1, "SETTLE fires exactly once for this one settle: {settled:?}");
    assert_eq!(settled[0], LOW, "the subscriber must see c's correct SETTLED value (LOW), never a transient glitch (HIGH)");
}
