//! The noise driver on devices (SAI-11): a divider of resistors with thermal
//! noise reads the analytic `4kT·(R1 ‖ R2)`, a noise input injected through
//! two terms is one correlated source and cancels, and each contribution
//! keeps its instance and input names.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{K_B, add_resistor, add_vsrc, noise_pair, resistor_noisy};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, CircuitBuilder, Context, NodeIdentifier, NoiseAnalysisOptions, Tolerances,
};

fn options(output: NodeIdentifier) -> NoiseAnalysisOptions {
    NoiseAnalysisOptions {
        sweep_options: AcSweepAnalysisOptions { start_frequency: 1.0e3, stop_frequency: 1.0e5, steps: 5, logarithmic: true },
        output_node: output,
        reference_node: NodeIdentifier::Gnd,
        input_source_name: None,
    }
}

#[test]
fn a_resistor_divider_has_its_thermal_noise_on_devices() {
    let (r1, r2) = (1.0e3, 3.0e3);
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    for (path, r, p, n) in [("r1", r1, &top, &mid), ("r2", r2, &mid, &gnd)] {
        b.device(
            resistor_noisy(),
            path,
            &[("r", ParamValue::Real(r))],
            &[Port::Node(p.clone()), Port::Node(n.clone())],
        );
    }
    let mut circuit = b.build().expect("build");
    let out = abi_devices::node_id(&mid);
    let result = circuit.noise(options(out), Context::default()).expect("noise").solve().expect("solve");

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
        add_resistor(&mut b, "r1", 1.0e3, &node, &gnd);
        b.device(noise_pair(), "x1", &[("k", ParamValue::Real(k))], &[Port::Node(node.clone())]);
        let mut circuit = b.build().expect("build");
        let out = abi_devices::node_id(&node);
        let result = circuit.noise(options(out), Context::default()).expect("noise").solve().expect("solve");
        result.out_noise_sq[0]
    };
    // Gains 1 and -1 on one source: the same current in and out, nothing reaches the node.
    assert!(density(-1.0) <= 1.0e-30, "a correlated pair leaks {:e}", density(-1.0));
    // One gain alone: 1e-18 A²/Hz into 1 kΩ is 1e-12 V²/Hz.
    let alone = density(0.0);
    assert!((alone - 1.0e-12).abs() <= 1.0e-9 * 1.0e-12, "{alone:e}");
}
