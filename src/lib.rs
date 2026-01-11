use crate::analysis::transient::TransientAnalysisOptions;
use crate::analysis::transient::TransientSolver;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::devices::voltage_source::Waveform;
use crate::devices::voltage_source::Waveform::{Sine, Step};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use circuit::netlist::GND;
use faer::prelude::Solve;
use faer::{Par, set_global_parallelism};
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
    circuit.voltage_source(
        "VCC",
        "vcc",
        GND,
        Sine {
            amplitude: 5.0.V(),
            frequency: 10.0.kHz(),
            phase: 0.0.deg(),
        },
    );
    circuit.resistor("R1", "vcc", 1, 10.0.Ohms());
    circuit.diode("D1", 1, 2);
    circuit.capacitor("C1", 2, GND, 10.0.uF());
    circuit.resistor("R2", 2, GND, 1.0.kOhms());

    let result = circuit
        .transient(
            TransientAnalysisOptions {
                stop_time: 0.01,
                dt: 5e-7,
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();
    // println!("{:?}", result);

    let times = result.timestamps(); // ArrayView1
    let matrix = result.values(); // ArrayView2
    for (i, row_values) in matrix.outer_iter().enumerate() {
        let t = times[i];
        println!(
            "{:.8} {:.8}",
            t,
            row_values[result.mapping[&CircuitReference::Node(2.into())]]
        );
    }
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
