//! The AC driver on devices (SAI-11): the parallel RLC of
//! `parity_baseline.rs` built from `Device`s reads the same pinned peak, and
//! nothing discrete runs or changes during a sweep.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::abi_ac::{REACTIONS, Spy};
use piperine_device_fixtures::circuits::CircuitBuilding;
use std::sync::atomic::Ordering;

use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{AcSweepAnalysisOptions, CircuitBuilder, Context};

#[test]
fn an_rlc_peak_matches_the_baseline_on_devices() {
    let mut b = CircuitBuilder::new("rlc");
    let gnd = b.ground();
    let node = b.node("node");
    b.device(circuit::AcCurrentSource::module(), "iac", &[("i", ParamValue::Real(1.0))], &[Port::Node(node.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r1", 10_000.0, &node, &gnd);
    CircuitBuilding::add_capacitor(&mut b, "c1", 1e-9, &node, &gnd);
    b.device(circuit::Inductor::module(), "l1", &[("l", ParamValue::Real(1e-3))], &[Port::Node(node.clone()), Port::Node(gnd)]);
    let spy = Spy::module();
    b.device(spy, "spy", &[], &[]);
    let mut circuit = b.build().expect("build");

    let opts = AcSweepAnalysisOptions { start_frequency: 1e4, stop_frequency: 1e6, steps: 200, logarithmic: true };
    // The bias point is an operating point: `START` fires there. The sweep is
    // measured from after it.
    let mut ac = circuit.ac(Context::default()).expect("ac");
    let reactions_before = REACTIONS.load(Ordering::SeqCst);
    let nets_before: Vec<Vec<u8>> = {
        let bound = ac.system.circuit.bound();
        (0..bound.digital.plan().nets.len())
            .map(|i| bound.digital.read(piperine_digital_solver::net::NetId(i as u32)).to_vec())
            .collect()
    };
    let res = ac.solve_sweep(opts).expect("sweep");

    let mut peak_mag = 0.0_f64;
    let mut peak_freq = 0.0_f64;
    for i in 0..res.len() {
        let step = res.get(i).unwrap();
        let v = step.get_node(&CircuitBuilding::node_id(&node)).unwrap();
        if v.norm() > peak_mag {
            peak_mag = v.norm();
            peak_freq = step.frequency;
        }
    }
    assert!((peak_mag - 9_817.187_756_927_691).abs() < 1e-6, "peak_mag = {peak_mag}");
    assert!((peak_freq - 160_705.281_826_163_85).abs() < 1e-3, "peak_freq = {peak_freq}");

    // No reaction ran and no pin changed during the sweep.
    assert_eq!(REACTIONS.load(Ordering::SeqCst), reactions_before, "a reaction ran");
    let bound = circuit.bound();
    for (i, before) in nets_before.iter().enumerate() {
        let now = bound.digital.read(piperine_digital_solver::net::NetId(i as u32));
        assert_eq!(now, before.as_slice(), "net {i} changed");
    }
}

