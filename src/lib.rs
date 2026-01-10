use crate::analysis::dc::DcSolver;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::analysis::transient::TransientSolver;
use crate::circuit::Circuit;
use crate::devices::voltage_source::Waveform;
use crate::math::unit::UnitExt;
use circuit::netlist::GND;
use crate::solver::Context;
use faer::prelude::Solve;
use faer::{set_global_parallelism, Par};
use num_traits::Zero;
use std::num::NonZeroUsize;
use std::sync::Once;
use tracing::debug;

mod analysis;
mod circuit;
mod devices;
mod error;
mod math;
mod result;
mod solver;
mod spice;
mod util;

static INIT: Once = Once::new();

pub fn init_config() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_thread_ids(true)
            .with_thread_names(true)
            .init();

        set_global_parallelism(Par::Rayon(NonZeroUsize::new(16).unwrap()));
    });
}

#[test]
pub fn test() {
    init_config();
    debug!("Starting test circuit simulation...");

    let mut circuit = Circuit::new("Test Circuit");
    circuit.voltage_source("VCC", "vcc", GND, 5.0.V());
    circuit.resistor("R1", "vcc", 1, 10.0.Ohms());
    circuit.diode("D1", 1, 2);
    circuit.capacitor("C1", 2, GND, 10.0.uF());
    circuit.resistor("R2", 2, GND, 10.0.Ohms());

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    println!("{:?}", result);

    // let stop_time = 0.012;
    // let dt = 0.000001;
    // let solution = Solver::build(circuit, Context::default())
    //     .unwrap()
    //     .transient(TransientAnalysisOptions { stop_time, dt })
    //     .unwrap();
    //
    // // println!("{:?}", solution);
    // solution.iter().for_each(|(freq, vec)| {
    //     let cap = vec
    //         .get(&CircuitReference::Node(NodeIdentifier::Indexed(2)))
    //         .cloned()
    //         .unwrap_or(0.0);
    //
    //     println!("{:0.9} {:0.9}", freq, cap);
    // });
}

#[test]
pub fn test_complex_nonlinear() {
    init_config();
    let mut circuit = Circuit::new("Nonlinear Stress Test");

    // 5V Supply
    circuit.voltage_source("VCC", "vcc", GND, 5.0.V());

    // Two paths merging into one node
    // Path A: Resistor -> Diode
    circuit.resistor("R1", "vcc", "node_a", 100.0.Ohms());
    circuit.diode("D1", "node_a", "merge");

    // Path B: Direct Diode
    circuit.diode("D2", "vcc", "merge");

    // The "Level Shifter": Diode in series
    circuit.diode("D3", "merge", "output");

    // Load to ground
    circuit.resistor("R_LOAD", "output", GND, 1.0.kOhms());

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    println!("{:?}", result);
}
