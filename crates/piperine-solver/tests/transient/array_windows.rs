//! Windows of an array net (composite-values CV-23, abi.md §3.1 P4a): the builder declares an array net, a scalar pin
//! binds to one of its elements, an array pin to the whole net or to a run of its elements, and a pin of another
//! count is a bind error naming both counts. The bound circuit's digital engine then runs on the elements.

use piperine_device_fixtures::solver_devices::array_windows::{Poker, Summer};

use piperine_digital_solver::engine::{Horizon, Mode};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::tick::Tick;
use piperine_solver::abi::{Port, Site};
use piperine_solver::prelude::CircuitBuilder;

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
