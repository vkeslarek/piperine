//! `piperine_solver::abi` is enough on its own: a device implements the
//! `Device` contract — a shape, evaluation with its partials, reactions —
//! importing nothing but `abi` (MD-17's two-tier surface), and it builds into
//! a circuit and solves.

use piperine_device_fixtures::circuit;

use piperine_solver::abi::{CircuitBuilder, Context, ParamValue, Port};

#[test]
fn abi_compiles_a_device() {
    let mut b = CircuitBuilder::new("test");
    let gnd = b.ground();
    let n1 = b.node("n1");
    // A 1 kΩ resistor to ground alone: nothing drives the node.
    b.device(
        circuit::Resistor::module(),
        "r1",
        &[("r", ParamValue::Real(1000.0))],
        &[Port::Node(n1.clone()), Port::Node(gnd)],
    );
    let mut circuit = b.build().expect("build");
    let res = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert_eq!(res.get(n1.variable().clone()), Some(0.0));
}

