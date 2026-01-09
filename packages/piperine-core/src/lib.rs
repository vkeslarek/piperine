use crate::analysis::dc::DcSolver;
use crate::circuit::Circuit;
use crate::math::unit::UnitExt;
use crate::netlist::GND;
use crate::solver::Context;
use crate::solver::dc::DcSolverImpl;
use faer::prelude::Solve;
use faer::{Par, set_global_parallelism};
use num_traits::Zero;
use std::num::NonZeroUsize;

mod analysis;
mod circuit;
mod devices;
mod error;
mod math;
mod netlist;
mod solver;
mod state;
mod util;

#[test]
pub fn test() {
    set_global_parallelism(Par::Rayon(NonZeroUsize::new(16).unwrap()));

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