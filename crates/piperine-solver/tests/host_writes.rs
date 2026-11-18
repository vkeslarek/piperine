//! Scope: a host write of a value to a discrete net, between analyses and during a transient
//! (host-ii HII-12): `CircuitInstance::write_net` is an initial value of every later analysis,
//! and `TransientSolver::schedule_write` commits at its time. The nets are an inverter's, whose
//! output is unknown while its input is, so a write is seen at the output.

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

use abi_mixed::inverter;
use piperine_digital_solver::net::NetId;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context};

fn net(name: &str) -> Port {
    Port::Net(name.to_string())
}

fn inverter_circuit() -> CircuitInstance {
    let mut b = CircuitBuilder::new("inverter");
    b.device(inverter(), "inv", &[], &[net("a"), net("y")]);
    b.build().expect("build")
}

/// The value and known bytes of the discrete net `name` of the plan.
fn read(circuit: &CircuitInstance, name: &str) -> Vec<u8> {
    let bound = circuit.bound().expect("bound");
    let at = bound.digital.plan().nets.iter().position(|n| n.name == name).expect("the net is in the plan");
    bound.digital.read(NetId(at as u32)).to_vec()
}

#[test]
fn write_net_before_a_run_is_an_initial_value() {
    // HII-12 AC 1 and 5: a net the host never writes is unknown (known byte 0) after the
    // operating point, and one written before the run holds the value (known byte 1) there, the
    // inverter's output following it
    let mut untouched = inverter_circuit();
    untouched.dc(Context::default()).expect("dc").solve().expect("op");
    assert_eq!(read(&untouched, "y"), [0, 0], "unwritten input: unknown output");

    let mut circuit = inverter_circuit();
    circuit.write_net("a", ParamValue::Bool(true)).expect("write");
    circuit.dc(Context::default()).expect("dc").solve().expect("op");
    assert_eq!(read(&circuit, "a"), [1, 1], "the written value, known");
    assert_eq!(read(&circuit, "y"), [0, 1], "the inverter of a high input is low and known");

    // a later write of the same net replaces the earlier one, and a name that is no net is refused
    circuit.write_net("a", ParamValue::Bool(false)).expect("rewrite");
    circuit.dc(Context::default()).expect("dc").solve().expect("op");
    assert_eq!(read(&circuit, "y"), [1, 1]);
    let refused = circuit.write_net("nothing", ParamValue::Bool(true)).expect_err("no such net");
    assert!(format!("{refused}").contains("nothing"), "{refused}");
}
