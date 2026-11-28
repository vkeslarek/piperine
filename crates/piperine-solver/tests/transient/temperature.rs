//! Temperature protocol contract tests (ABI-19/20) on the `Device` trait: the
//! `TEMPERATURE` global holds the run's temperature before the first
//! evaluation, a host running a temperature sweep re-seeds it through
//! `CircuitInstance::set_temperature`, and a device with no `TEMPERATURE` pin
//! is untouched by either.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::solver_devices::temperature::{Thermo};

use piperine_solver::abi::{AnalogReference, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, Solver, Tolerances};

/// ABI-19: when the solver enters an analysis, the `TEMPERATURE` global
/// holds the run's temperature before the first evaluation: the very first
/// operating point already reads it.
#[test]
fn set_temperature_called_before_first_load() {
    let (circuit, out) = TemperatureCircuit::circuit();
    let run_temperature = 325.0;
    let ctx = Context {
        tolerances: Tolerances { temperature: run_temperature, ..Tolerances::default() },
        ..Context::default()
    };
    let mut solver = Solver::new(circuit).with_context(ctx).build();
    let result = solver.dc().unwrap().solve().unwrap();
    let v = result.get_node(&CircuitBuilding::node_id(&out)).unwrap();
    assert!(
        (v - run_temperature * 1.0e-3).abs() < 1e-9,
        "the first solve read the run temperature {run_temperature}: v(out) = {v}"
    );
}

/// ABI-20: a temperature sweep point change re-seeds the global; the next
/// solve uses the new temperature.
#[test]
fn temperature_sweep_reseeds_every_element() {
    let (mut circuit, out) = TemperatureCircuit::circuit();
    let nominal = Tolerances::default().temperature;
    assert!((TemperatureCircuit::v_out(&mut circuit, &out) - nominal * 1.0e-3).abs() < 1e-9);

    let sweep_t = 380.0;
    circuit.set_temperature(sweep_t);
    let v = TemperatureCircuit::v_out(&mut circuit, &out);
    assert!((v - sweep_t * 1.0e-3).abs() < 1e-9, "sweep temperature {sweep_t} reached the device: v(out) = {v}");

    // Back to the seeded temperature: nothing stays written.
    circuit.set_temperature(nominal);
    assert!((TemperatureCircuit::v_out(&mut circuit, &out) - nominal * 1.0e-3).abs() < 1e-9);
}

/// ABI-19 edge case: a device with no `TEMPERATURE` pin is unaffected.
#[test]
fn default_set_temperature_is_noop() {
    let mut b = CircuitBuilder::new("default-noop");
    let gnd = b.ground();
    let src = b.node("src");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &src, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &src, &mid);
    CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    let mut circuit = b.build().expect("build");
    let before = TemperatureCircuit::v_out(&mut circuit, &mid);
    circuit.set_temperature(350.0);
    assert_eq!(TemperatureCircuit::v_out(&mut circuit, &mid), before);
    assert!((before - 0.5).abs() < 1e-12);
}

/// The circuit whose output is read at a temperature.
struct TemperatureCircuit;

impl TemperatureCircuit {

    /// 1 kΩ from `out` to ground, a `Thermo` pulling `T` µA out of `out`:
    /// `v(out) = T` millivolts. Also a source and a resistor the device does not touch.
    fn circuit() -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("temp");
        let gnd = b.ground();
        let out = b.node("out");
        let src = b.node("src");
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &out, &gnd);
        b.device(Thermo::module(), "t1", &[], &[Port::Node(out.clone()), Port::Node(gnd.clone())]);
        CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &src, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &src, &gnd);
        (b.build().expect("build"), out)
    }

    /// The v out helper `v_out`.
    fn v_out(circuit: &mut CircuitInstance, out: &AnalogReference) -> f64 {
        circuit.dc(Context::default()).unwrap().solve().unwrap().get_node(&CircuitBuilding::node_id(out)).unwrap()
    }
}
