//! `piperine_solver::prelude` is enough on its own: a host builds a circuit of
//! devices, runs DC and transient, and reads the results using only prelude
//! imports — the host-facing surface stays self-sufficient (MD-17). The device
//! modules come from a device library (`helpers/abi_devices.rs`); the host
//! never names their types.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use piperine_solver::prelude::*;

/// 10 V over `r1` (top→mid) and `r2` (mid→gnd), with a capacitor on `mid`.
fn host_circuit() -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("test");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    let port = |n: &AnalogReference| Port::Node(n.clone());
    b.device(abi_devices::vsrc(), "v1", &[("dc", ParamValue::Real(10.0))], &[port(&top), port(&gnd)]);
    b.device(abi_devices::resistor(), "r1", &[("r", ParamValue::Real(1000.0))], &[port(&top), port(&mid)]);
    b.device(abi_devices::resistor(), "r2", &[("r", ParamValue::Real(1000.0))], &[port(&mid), port(&gnd)]);
    b.device(abi_devices::capacitor(), "c1", &[("c", ParamValue::Real(1.0e-9))], &[port(&mid), port(&gnd)]);
    (b.build().expect("build"), mid)
}

fn net_of(reference: &AnalogReference) -> Net {
    reference.into()
}

#[test]
fn prelude_voltage_divider() {
    let (mut circuit, mid) = host_circuit();
    let ctx = Context::default();
    let mut dc = circuit.dc(ctx).unwrap();
    let res = dc.solve().unwrap();

    let mid_val = res.get_net(&net_of(&mid)).unwrap();
    assert!((mid_val - 5.0).abs() < 1e-9);

    // Use SolverStats and Error to verify they are available
    let _stats: &SolverStats = &res.stats;
}

#[test]
fn prelude_transient_holds_the_divider() {
    let (mut circuit, mid) = host_circuit();
    let mut tran = circuit.transient(TransientAnalysisOptions::new(1.0e-6, 1.0e-7), Context::default()).unwrap();
    let res = tran.solve().unwrap();
    let last = res.last().expect("steps");
    assert!((last.get_net(&net_of(&mid)).unwrap() - 5.0).abs() < 1e-6);
}
