//! Scope: the bounds a session can set (host-ii HII-02 AC 6 and 7): the rounds of an operating
//! point (`Context::max_rounds`, default 20) and the microsteps of an instant (`CircuitBuilder::
//! microsteps`, default 1000). Each bound is observed by the failure it causes when it is
//! reached, and its default by the value the failure names.

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use abi_devices::{add_vsrc, node_id};
use abi_mixed::{comparator, dac, dff, inverter, settle_count};
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context};

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

/// A DAC, a comparator, a flip-flop and an inverter in a loop: `code_from` is the net the
/// inverter reads. From `q` it settles after three rounds; from `cmp` it never settles.
fn mixed_loop(code_from: &str) -> (CircuitBuilder, AnalogReference) {
    let mut b = CircuitBuilder::new("mixed loop");
    let gnd = b.ground();
    let a = b.node("a");
    let r = b.node("r");
    add_vsrc(&mut b, "vr", 0.5, &r, &gnd);
    b.device(dac(), "dac", &[("vref", ParamValue::Real(1.0))], &[net("code"), Port::Node(a.clone())]);
    b.device(comparator(), "cmp", &[], &[Port::Node(a.clone()), Port::Node(r), net("cmp")]);
    b.device(dff(), "ff", &[], &[net("cmp"), net("code"), net("q")]);
    b.device(inverter(), "inv", &[], &[net(code_from), net("code")]);
    b.device(settle_count(), "settle", &[], &[]);
    (b, a)
}

#[test]
fn max_rounds_defaults_to_twenty() {
    // HII-02 AC 1: no option leaves the bound where it was, 20 rounds. A loop that never
    // settles fails naming the bound it reached; with a default context that is 20.
    assert_eq!(Context::default().max_rounds, 20);
    let (b, _) = mixed_loop("cmp");
    let mut circuit = b.build().expect("build");
    let error = circuit.dc(Context::default()).expect("dc").solve().expect_err("an oscillator does not settle");
    assert!(format!("{error}").contains("20 rounds"), "{error}");
}

#[test]
fn an_operating_point_past_max_rounds_fails_naming_the_bound() {
    // HII-02 AC 6: the loop that settles in three rounds, run with `max_rounds = 1`, fails with
    // `E0506` naming 1; with the default it settles (the control), so the bound is what failed it.
    let (b, _) = mixed_loop("q");
    let mut circuit = b.build().expect("build");
    let context = Context { max_rounds: 1, ..Context::default() };
    let error = circuit.dc(context).expect("dc").solve().expect_err("one round is not enough");
    let text = format!("{error}");
    assert!(text.contains("E0506"), "{text}");
    assert!(text.contains("1 rounds"), "{text}");

    let (b, a2) = mixed_loop("q");
    let mut circuit = b.build().expect("build");
    let op = circuit.dc(Context::default()).expect("dc").solve().expect("the default bound lets it settle");
    assert!(op.get_node(&node_id(&a2)).unwrap().abs() < 1e-12);
}
