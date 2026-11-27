//! Lifecycle contract test (ABI-16) on the `Device` trait: an instance is
//! created once per placement when the circuit is bound and destroyed (the
//! device dropped) once when the circuit is dropped; a failing `create`
//! reaches the host at build.
//!
//! The hook-ordering charts of Part VII §19 over the removed per-hook contract
//! (`setup`, `set_temperature`, `update`, `load_*`, `limiting_report`,
//! `accept_timestep`, `set_param`) are gone with them; the order the solver
//! evaluates a device in is the protocol's, proven by `protocol_*.rs`.

use piperine_device_fixtures::solver_devices::lifecycle::{CREATES, Counted, DESTROYS, FAIL_CREATES, Failing};
use std::sync::atomic::Ordering;

use piperine_solver::abi::Port;
use piperine_solver::prelude::{CircuitBuilder, Context};

#[test]
fn test_lifecycle_hooks_called() {
    let mut b = CircuitBuilder::new("test_circuit");
    let gnd = b.ground();
    b.device(Counted::module(), "dev", &[], &[Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");
    assert_eq!(CREATES.load(Ordering::SeqCst), 1, "created once, at bind");

    let context = Context::default();
    let mut dc = circuit.dc(context.clone()).unwrap();
    let _ = dc.solve().unwrap();
    let _ac = circuit.ac(context.clone()).unwrap();
    assert_eq!(CREATES.load(Ordering::SeqCst), 1, "an analysis does not create it again");
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 0);

    drop(circuit);
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 1);
}

#[test]
fn test_setup_error_propagates() {
    let mut b = CircuitBuilder::new("test_circuit");
    let gnd = b.ground();
    b.device(Failing::module(), "dev", &[], &[Port::Node(gnd)]);
    let result = b.build();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("test setup error"));
    assert_eq!(FAIL_CREATES.load(Ordering::SeqCst), 1);
}

