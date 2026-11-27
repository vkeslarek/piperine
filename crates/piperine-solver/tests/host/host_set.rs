//! Host writes to device params (SAI-08; abi.md §10 "Host actions", P5, P6):
//! a false `check` refuses a `set`; a `RESHAPE` is refused inside a transient
//! and re-creates and re-binds the instance between analyses; a scheduled
//! write is a landing followed by a step of 1e-3 of the last; a `STRUCTURAL`
//! param cannot change during a run.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::host_set::{CheckedR, Collapsible};
use piperine_device_fixtures::circuits::CircuitBuilding;

use piperine_abi::raw::Module;
use piperine_solver::abi::{AnalogReference, ParamValue, Port, Value};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions};


#[test]
fn a_false_check_refuses_a_set() {
    let (mut circuit, mid) = HostSetCases::divider(CheckedR::module());
    assert!((HostSetCases::v_mid(&mut circuit, &mid) - 0.5).abs() < 1e-12);

    let err = circuit.set("x", "r", ParamValue::Real(-5.0)).expect_err("r > 0 is false").to_string();
    assert!(err.contains("x.ok"), "the check is named: {err}");
    assert!((HostSetCases::v_mid(&mut circuit, &mid) - 0.5).abs() < 1e-12, "the next solve uses the old value");

    circuit.set("x", "r", ParamValue::Real(3.0e3)).expect("r > 0 holds");
    assert!((HostSetCases::v_mid(&mut circuit, &mid) - 0.75).abs() < 1e-12, "an accepted write is read");
}

#[test]
fn reshape_is_refused_in_a_transient_and_rebinds_between_analyses() {
    let (mut circuit, mid) = HostSetCases::divider(Collapsible::module());
    circuit.set("r1", "r", ParamValue::Real(3.0e3)).expect("a plain write");
    let unknowns = circuit.netlist().all_references().len();
    assert!((HostSetCases::v_mid(&mut circuit, &mid) - 0.25).abs() < 1e-12);

    // Inside a transient the write is refused, and nothing changed.
    {
        let mut solver =
            circuit.transient(TransientAnalysisOptions::new(1.0e-3, 1.0e-5), Context::default()).expect("tran");
        solver.schedule_set(5.0e-4, "x", "r", Value::Real(50.0));
        let err = solver.solve().expect_err("RESHAPE inside a transient").to_string();
        assert!(err.contains("another shape"), "{err}");
    }
    assert_eq!(circuit.netlist().all_references().len(), unknowns);
    assert!((HostSetCases::v_mid(&mut circuit, &mid) - 0.25).abs() < 1e-12);

    // Between analyses the instance is created and bound again: its private
    // node is gone, and every param net keeps its value (r1 stays 3 kΩ).
    circuit.set("x", "r", ParamValue::Real(50.0)).expect("re-created and re-bound");
    assert_eq!(circuit.netlist().all_references().len(), unknowns - 1, "the internal node collapsed");
    let expected = 50.0 / (3.0e3 + 50.0);
    assert!((HostSetCases::v_mid(&mut circuit, &mid) - expected).abs() < 1e-12, "v(mid) = {}", HostSetCases::v_mid(&mut circuit, &mid));
}

#[test]
fn a_scheduled_host_write_lands_and_restarts_small() {
    let at = 5.0e-4;
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &out);
    CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-6, &out, &gnd);
    let mut circuit = b.build().expect("build");
    let options = TransientAnalysisOptions::new(1.0e-3, 1.0e-5);
    let dt_min = options.dt_min;
    let mut solver = circuit.transient(options, Context::default()).expect("tran");
    solver.schedule_set(at, "v1", "dc", Value::Real(2.0));
    let result = solver.solve().expect("solve");

    let k = result.iter().rposition(|s| (s.time() - at).abs() < 1.0e-15).expect("the write's time is an accepted time");
    let t = |i: usize| result.get(i).unwrap().time();
    let v = |i: usize| result.get(i).unwrap().get_node(&CircuitBuilding::node_id(&top)).unwrap();
    assert_eq!(v(k), 1.0, "the landing is solved before the write");
    assert_eq!(v(k + 1), 2.0, "the next step reads it");
    let (last, next) = (t(k) - t(k - 1), t(k + 1) - t(k));
    let small = (0.1 * last).max(dt_min);
    assert!((next - small).abs() <= 1e-9 * small, "the next step is 0.1 of the last: {next:e} vs {small:e}");
}

#[test]
fn a_structural_set_during_a_run_is_refused() {
    let mut b = CircuitBuilder::new("structural");
    let gnd = b.ground();
    let top = b.node("top");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
    b.device(circuit::OptionalResistor::module(), "x", &[], &[Port::Node(top), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");
    let w_net = circuit.bound().names.params[&("x".to_string(), "w".to_string())];
    {
        let mut solver =
            circuit.transient(TransientAnalysisOptions::new(1.0e-3, 1.0e-5), Context::default()).expect("tran");
        solver.schedule_set(5.0e-4, "x", "w", Value::Real(2.0e-3));
        let err = solver.solve().expect_err("a STRUCTURAL pin cannot change during a run").to_string();
        assert!(err.contains("STRUCTURAL"), "{err}");
    }
    let bound = circuit.bound();
    assert_eq!(bound.digital.read(w_net), 1.0e-3f64.to_le_bytes(), "nothing changed: `w` still holds its default");
    assert!(bound.host_writes.is_empty());
}

/// The helpers of the host_set tests.
pub struct HostSetCases;

impl HostSetCases {

    /// A 1 V source over `r1` (1 kΩ) into `x`, a device of `module`, to ground.
    fn divider(module: &'static Module) -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("divider");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
        b.device(module, "x", &[], &[Port::Node(mid.clone()), Port::Node(gnd)]);
        (b.build().expect("build"), mid)
    }

    /// The v mid helper `v_mid`.
    fn v_mid(circuit: &mut CircuitInstance, mid: &AnalogReference) -> f64 {
        circuit.dc(Context::default()).expect("dc").solve().expect("dc").get_node(&CircuitBuilding::node_id(mid)).expect("v(mid)")
    }
}
