//! Sensitivities through declared partials (SAI-10): the adjoint driver on
//! devices takes `∂F/∂param` from the partials a device declares against its
//! `PARAM` pin (chained through discrete functions), differences by
//! re-evaluation where no declared path exists, and fails loud on a
//! `STRUCTURAL` param.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::abi_sensitivity::{Gd};
use piperine_device_fixtures::circuits::CircuitBuilding;

use piperine_abi::raw::Module;
use piperine_digital_solver::engine::{Horizon, Mode};
use piperine_digital_solver::tick::Tick;
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{AdjointOptions, CircuitBuilder, CircuitInstance, Context, Net};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn sensitivity_uses_declared_partials_and_falls_back_to_a_finite_difference() {
    let wrt = AbiSensitivityCases::params(&[("r1", "r"), ("r2", "r"), ("v1", "dc")]);
    let run = |difference: bool| {
        let (mut circuit, mid) = AbiSensitivityCases::divider();
        let output = AbiSensitivityCases::net_of(&circuit, &mid);
        let label = output.label().to_string();
        let mut options = AdjointOptions::new(output, wrt.clone());
        options.finite_difference_only = difference;
        (label, circuit.adjoint(options, Context::default()).expect("adjoint").solve().expect("solve"))
    };
    let (out, declared) = run(false);
    let (_, differenced) = run(true);

    assert!(declared.fallback_users.is_empty(), "the declared partials cover every param");
    assert_eq!(differenced.fallback_users.len(), 3, "every param is differenced: {:?}", differenced.fallback_users);
    // v(mid) = 10 r2 / (r1 + r2): -2.5e-3 and +2.5e-3 per ohm, 0.5 per volt.
    let analytic = [("r1", "r", -2.5e-3), ("r2", "r", 2.5e-3), ("v1", "dc", 0.5)];
    for (label, param, want) in analytic {
        let a = declared.get(&out, label, param).expect("declared");
        let b = differenced.get(&out, label, param).expect("differenced");
        assert!((a - want).abs() <= 1.0e-9 * want.abs(), "declared d/d{label}.{param} = {a:e} vs {want:e}");
        assert!((a - b).abs() <= 1.0e-6 * a.abs(), "{label}.{param}: declared {a:e} vs difference {b:e}");
    }
}

#[test]
fn a_structural_param_sensitivity_fails_loud() {
    let mut b = CircuitBuilder::new("opt");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top, &gnd);
    b.device(
        circuit::OptionalResistor::module(),
        "r1",
        &[("w", ParamValue::Real(2.0e-3))],
        &[Port::Node(top), Port::Node(mid.clone())],
    );
    CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    let mut circuit = b.build().expect("build");
    let options = AdjointOptions::new(AbiSensitivityCases::net_of(&circuit, &mid), AbiSensitivityCases::params(&[("r1", "w")]));
    let err = circuit.adjoint(options, Context::default()).expect("adjoint").solve().err().expect("refused");
    assert!(err.to_string().contains("STRUCTURAL"), "{err}");
}

#[test]
fn a_derivative_chains_through_a_discrete_function() {
    let mut b = CircuitBuilder::new("chain");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top, &gnd);
    b.device(AbiSensitivityCases::gd(), "x1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(top), Port::Node(mid.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    let mut circuit = b.build().expect("build");

    // Run the instant at t = 0 by hand (the protocol drives it in a transient) and
    // hand the analog side the derived conductance.
    {
        let bound = circuit.bound_mut();
        bound.digital.start().expect("start");
        let outcome = bound
            .digital
            .run_instant(Tick(0), &[], Horizon::uniform(Tick(0)), Mode { is_static: true, settle: false })
            .expect("instant");
        for (net, bytes) in outcome.boundary {
            let slot = bound.names.slots[&net];
            bound.analog.set_discrete(slot, &bytes);
        }
    }
    let output = AbiSensitivityCases::net_of(&circuit, &mid);
    let out = output.label().to_string();
    let options = AdjointOptions::new(output, AbiSensitivityCases::params(&[("x1", "r")]));
    let result = circuit.adjoint(options, Context::default()).expect("adjoint").solve().expect("solve");

    // v(mid) = 10 r2 / (r + r2), so dv/dr = -10 r2 / (r + r2)^2 = -2.5e-3.
    let d = result.get(&out, "x1", "r").expect("derivative");
    assert!((d + 2.5e-3).abs() <= 1.0e-9 * 2.5e-3, "dv/dr = {d:e}");
    assert!(result.fallback_users.is_empty());
}

/// The helpers of the abi_sens tests.
pub struct AbiSensitivityCases;

impl AbiSensitivityCases {

    /// The gd helper `gd`.
    fn gd() -> &'static Module {
        Gd::module()
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    /// The net of helper `net_of`.
    fn net_of(circuit: &CircuitInstance, node: &AnalogReference) -> Net {
        let var = node.variable().clone();
        circuit
            .nets()
            .into_iter()
            .find(|n| n.analog_variable().map(|v| **v == *var).unwrap_or(false))
            .expect("net is solved")
    }

    /// The divider `Top` of the sensitivity targets: 10 V over two 1 kΩ resistors.
    fn divider() -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("Top");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
        CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
        (b.build().expect("build"), mid)
    }

    /// The params helper `params`.
    fn params(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }
}
