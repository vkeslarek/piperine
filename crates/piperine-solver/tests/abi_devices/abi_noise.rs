//! The noise driver on devices (SAI-11): a divider of resistors with thermal
//! noise reads the analytic `4kT·(R1 ‖ R2)`, a noise input injected through
//! two terms is one correlated source and cancels, each contribution keeps its
//! instance and input names, and the per-source PSDs add up to the output.

use piperine_device_fixtures::circuit::K_B;
use piperine_device_fixtures::solver_devices::abi_noise::{NoisyR};
use piperine_device_fixtures::circuit;
use piperine_device_fixtures::circuits::CircuitBuilding;

use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, CircuitBuilder, Context, NodeIdentifier, NoiseAnalysisOptions, NoiseKind, Tolerances,
};

#[test]
fn a_resistor_divider_has_its_thermal_noise_on_devices() {
    let (r1, r2) = (1.0e3, 3.0e3);
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
    for (path, r, p, n) in [("r1", r1, &top, &mid), ("r2", r2, &mid, &gnd)] {
        b.device(
            circuit::NoisyResistor::module(),
            path,
            &[("r", ParamValue::Real(r))],
            &[Port::Node(p.clone()), Port::Node(n.clone())],
        );
    }
    let mut circuit = b.build().expect("build");
    let out = CircuitBuilding::node_id(&mid);
    let result = circuit.noise(NoiseOptions::options(out), Context::default()).expect("noise").solve().expect("solve");

    let analytic = 4.0 * K_B * Tolerances::default().temperature * (r1 * r2 / (r1 + r2));
    for (i, density) in result.out_noise_sq.iter().enumerate() {
        assert!((density - analytic).abs() <= 1.0e-9 * analytic, "density {density:e} vs {analytic:e} at point {i}");
    }
    // Each contribution keeps its instance and input names.
    let mut names: Vec<(String, String)> =
        result.contributions.iter().map(|c| (c.element.clone(), c.source.clone())).collect();
    names.sort();
    assert_eq!(names, vec![("r1".into(), "thermal".into()), ("r2".into(), "thermal".into())]);
}

#[test]
fn a_shared_noise_input_cancels_across_the_pair() {
    let density = |k: f64| {
        let mut b = CircuitBuilder::new("pair");
        let gnd = b.ground();
        let node = b.node("node");
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &node, &gnd);
        b.device(circuit::NoisePair::module(), "x1", &[("k", ParamValue::Real(k))], &[Port::Node(node.clone())]);
        let mut circuit = b.build().expect("build");
        let out = CircuitBuilding::node_id(&node);
        let result = circuit.noise(NoiseOptions::options(out), Context::default()).expect("noise").solve().expect("solve");
        result.out_noise_sq[0]
    };
    // Gains 1 and -1 on one source: the same current in and out, nothing reaches the node.
    assert!(density(-1.0) <= 1.0e-30, "a correlated pair leaks {:e}", density(-1.0));
    // One gain alone: 1e-18 A²/Hz into 1 kΩ is 1e-12 V²/Hz.
    let alone = density(0.0);
    assert!((alone - 1.0e-12).abs() <= 1.0e-9 * 1.0e-12, "{alone:e}");
}

#[test]
fn test_per_source_noise_reporting_and_conservation() {
    let mut b = CircuitBuilder::new("test");
    let gnd = b.ground();
    let top = b.node("top");
    for path in ["r1", "r2"] {
        b.device(
            NoisyR::module(),
            path,
            &[("r", ParamValue::Real(1000.0))],
            &[Port::Node(top.clone()), Port::Node(gnd.clone())],
        );
    }
    let mut circuit = b.build().expect("build");
    let sweep = AcSweepAnalysisOptions { start_frequency: 1.0, stop_frequency: 100.0, steps: 10, logarithmic: false };
    let opts = NoiseAnalysisOptions {
        sweep_options: sweep,
        output_node: CircuitBuilding::node_id(&top),
        reference_node: NodeIdentifier::Gnd,
        input_source_name: None,
        output_flow: None,
    };
    let mut solver = circuit.noise(opts, Context::default()).unwrap();
    let res = solver.solve().unwrap();

    let contribs = res.contributions();
    assert_eq!(contribs.len(), 4);

    let mut found_thermal = 0;
    let mut found_flicker = 0;
    for c in contribs {
        if c.source == "thermal" && c.kind == NoiseKind::Thermal {
            found_thermal += 1;
            assert_eq!(c.psd.len(), 10);
        }
        if c.source == "flicker" && c.kind == NoiseKind::Flicker {
            found_flicker += 1;
            assert_eq!(c.psd.len(), 10);
        }
    }
    assert_eq!(found_thermal, 2);
    assert_eq!(found_flicker, 2);

    for i in 0..res.frequencies.len() {
        let sum_psd: f64 = contribs.iter().map(|c| c.psd[i]).sum();
        let total = res.out_noise_sq[i];

        let err = (sum_psd - total).abs();
        let rel_err = if total > 1e-30 { err / total } else { err };
        assert!(rel_err < 1e-9, "freq idx {}, sum {} != total {}", i, sum_psd, total);
    }

    let mut manual_int = 0.0;
    for i in 0..res.frequencies.len() - 1 {
        let df = res.frequencies[i + 1] - res.frequencies[i];
        manual_int += 0.5 * df * (res.out_noise_sq[i] + res.out_noise_sq[i + 1]);
    }
    assert!((res.integrated_noise - manual_int.sqrt()).abs() < 1e-12);
}

/// The options a noise run of an ABI device is asked with.
struct NoiseOptions;

impl NoiseOptions {
    /// The options helper `options`.
    fn options(output: NodeIdentifier) -> NoiseAnalysisOptions {
        NoiseAnalysisOptions {
            sweep_options: AcSweepAnalysisOptions { start_frequency: 1.0e3, stop_frequency: 1.0e5, steps: 5, logarithmic: true },
            output_node: output,
            reference_node: NodeIdentifier::Gnd,
            input_source_name: None,
            output_flow: None,
        }
    }

    // ─── A resistor with a thermal and a flicker source ─────────────────────────
}
