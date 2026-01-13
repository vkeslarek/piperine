use crate::analysis::ac::AcSweepAnalysisOptions;
use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::Circuit;
use crate::circuit::netlist::{BranchIdentifier, CircuitReference};
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

#[cfg(test)]
mod tests;

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
            amplitude: 12.0.V(),
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
                dt: 5e-6,
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();

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
