//! Windows of an array net (composite-values CV-23, abi.md §3.1 P4a): the builder declares an array net, a scalar pin
//! binds to one of its elements, an array pin to the whole net or to a run of its elements, and a pin of another
//! count is a bind error naming both counts. The bound circuit's digital engine then runs on the elements.

#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE};
use piperine_abi::raw::{Cell, Module, Shape, Statement, WHOLE, cell, part, role};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_digital_solver::engine::{Horizon, Mode};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::tick::Tick;
use piperine_solver::abi::{Port, Site};
use piperine_solver::prelude::CircuitBuilder;

/// Writes `1` to its scalar pin `bit` when `go` fires.
struct Poker;
static POKER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static POKER: OnceLock<Module> = OnceLock::new();

impl Poker {
    /// pins: `bit` (0, `BIT`), `go` (1, event); reaction 0 on `go` writes statement 0 on `bit`.
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let bit = buffer.pin(PinSpec::discrete("bit", role::PORT, &BIT));
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        let write = Statement { pin: bit, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        buffer.reaction(ReactionSpec { name: "poke", on: vec![go], writes: vec![write], ..ReactionSpec::default() });
        buffer.freeze()
    }

    fn module() -> &'static Module {
        POKER.get_or_init(|| module::<Poker>("poker"))
    }
}

impl Device for Poker {
    fn prototype() -> &'static Shape<'static> {
        POKER_SHAPE.get_or_init(Poker::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Poker, Arc::clone(POKER_SHAPE.get_or_init(Poker::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &[1], 0.0)?;
        Ok(Flow::Continue)
    }
}

/// On `go`, counts the elements of its array pin `mem` (four elements, read whole) that are `1` into `sum`.
struct Summer;
static SUMMER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUMMER: OnceLock<Module> = OnceLock::new();

impl Summer {
    /// pins: `mem` (0, `BIT`, 4 elements), `go` (1, event), `sum` (2, `BYTE`).
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let mem = buffer.pin(PinSpec { count: 4, ..PinSpec::discrete("mem", role::PORT, &BIT) });
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        let sum = buffer.pin(PinSpec::discrete("sum", role::PORT, &BYTE));
        let write = Statement { pin: sum, offset: 0, len: 1, flags: 0, min_after: 0.0 };
        buffer.reaction(ReactionSpec {
            name: "sum",
            on: vec![go],
            reads: vec![Cell { kind: cell::PIN, index: mem, part: part::VALUE }],
            elements: vec![WHOLE],
            writes: vec![write],
            ..ReactionSpec::default()
        });
        buffer.freeze()
    }

    fn module() -> &'static Module {
        SUMMER.get_or_init(|| module::<Summer>("summer"))
    }
}

impl Device for Summer {
    fn prototype() -> &'static Shape<'static> {
        SUMMER_SHAPE.get_or_init(Summer::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Summer, Arc::clone(SUMMER_SHAPE.get_or_init(Summer::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let view = cx.array(0);
        let ones = (0..view.count).filter(|element| view.element(*element, part::VALUE) == Some(&[1][..])).count();
        cx.commit(0, &[ones as u8], 0.0)?;
        Ok(Flow::Continue)
    }
}

#[test]
fn a_window_binds_a_pin_to_elements_of_an_array_net() {
    // CV-23: `lines` has eight elements; `poker` is on element 5, `low` sums elements 4..8 (four) and `all`'s pin of
    // four elements cannot take the whole net of eight (P4).
    let mut b = CircuitBuilder::new("windows");
    b.array_net("lines", 8, &[0]);
    b.device(Poker::module(), "poker", &[], &[Port::Window { net: "lines".into(), first: 5, length: 1 }]);
    b.device(
        Summer::module(),
        "high",
        &[],
        &[Port::Window { net: "lines".into(), first: 4, length: 4 }, Port::Net("high".into())],
    );
    b.device(
        Summer::module(),
        "low",
        &[],
        &[Port::Window { net: "lines".into(), first: 0, length: 4 }, Port::Net("low".into())],
    );
    let mut circuit = b.build().expect("the windows bind");
    let bound = circuit.bound_mut();
    let lines = bound.names.nets["net:lines"];
    assert_eq!(bound.names.pins[0][0], Site::ArrayElement { net: lines, element: 5 }, "a scalar pin on one element");
    assert_eq!(bound.names.pins[1][0], Site::ArrayWindow { net: lines, first: 4, length: 4 }, "an array pin on a run");
    let net = |bound: &piperine_solver::abi::Bound, name: &str| {
        NetId(bound.digital.plan().nets.iter().position(|n| n.name == name).unwrap_or_else(|| panic!("no net {name}")) as u32)
    };
    let (go_poker, go_high, go_low) = (net(bound, "poker.go"), net(bound, "high.go"), net(bound, "low.go"));
    let (high, low) = (bound.names.nets["net:high"], bound.names.nets["net:low"]);

    bound.digital.start().expect("start");
    let run = |bound: &mut piperine_solver::abi::Bound, at: i64| {
        bound.digital.run_instant(Tick(at), &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs");
    };
    run(bound, 0);
    bound.digital.fire(go_poker, Tick(1)).expect("go is an event net");
    run(bound, 1);
    for element in 0..8u64 {
        assert_eq!(bound.digital.read_element(lines, element), [u8::from(element == 5)], "element {element}");
    }
    bound.digital.fire(go_high, Tick(2)).expect("event");
    bound.digital.fire(go_low, Tick(2)).expect("event");
    run(bound, 2);
    assert_eq!(bound.digital.read(high), [1], "window 4..8 holds element 5");
    assert_eq!(bound.digital.read(low), [0], "window 0..4 holds none");
}

#[test]
fn a_pin_of_another_count_is_a_bind_error_naming_both_counts() {
    // P4: an array pin and its net have equal `count`.
    let mut b = CircuitBuilder::new("mismatch");
    b.array_net("lines", 8, &[]);
    b.device(Summer::module(), "all", &[], &[Port::Net("lines".into()), Port::Net("sum".into())]);
    let Err(error) = b.build() else { panic!("four elements on a net of eight bind") };
    let error = error.to_string();
    assert!(error.contains("4 elements") && error.contains("8 elements"), "both counts are named: {error}");

    let mut window = CircuitBuilder::new("window length");
    window.array_net("lines", 8, &[]);
    window.device(
        Summer::module(),
        "all",
        &[],
        &[Port::Window { net: "lines".into(), first: 0, length: 2 }, Port::Net("sum".into())],
    );
    let Err(error) = window.build() else { panic!("four elements on a window of two bind") };
    let error = error.to_string();
    assert!(error.contains("4 elements") && error.contains("2 elements"), "both counts are named: {error}");

    let mut past = CircuitBuilder::new("past the end");
    past.array_net("lines", 8, &[]);
    past.device(Poker::module(), "poker", &[], &[Port::Window { net: "lines".into(), first: 8, length: 1 }]);
    assert!(past.build().is_err(), "element 8 of 8 is past the end");
}

#[test]
fn the_fixture_devices_pass_the_partials_check() {
    // HYG-16: every hand-written device of a target is in a partials check (these declare no partials).
    partials::verify_every_function("Poker", Poker::module());
    partials::verify_every_function("Summer", Summer::module());
}
