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
use std::rc::Rc;
use std::sync::Once;
use tracing::debug;

pub mod analysis;
pub mod circuit;
pub mod devices;
pub mod error;
pub mod math;
pub mod result;
pub mod solver;
pub mod spice;
pub mod util;

#[cfg(test)]
mod tests;

static INIT: Once = Once::new();

pub fn init_config() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_thread_ids(true)
            .with_thread_names(true)
            .init();

        set_global_parallelism(Par::Rayon(NonZeroUsize::new(1).unwrap()));
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
        Step {
            initial: 0.0,
            final_value: 12.0,
            delay: 1e-6,
            rise_time: 5e-7,
        },
    );
    circuit.resistor("R1", "vcc", 1, 10.0.Ohms());
    circuit.diode("D1", 1, 2);
    circuit.capacitor("C1", 2, GND, 10.0.uF());
    circuit.resistor("R2", 2, GND, 1.0.kOhms());

    let result = circuit
        .transient(
            TransientAnalysisOptions {
                stop_time: 0.0006,
                dt: 5e-7,
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();

    result.values.iter().for_each(|val| {
        let t = val
            .get(*result.mapping.get(&CircuitReference::Time).unwrap())
            .unwrap();
        let v_out = val
            .get(
                *result
                    .mapping
                    .get(&CircuitReference::Node(2.into()))
                    .unwrap(),
            )
            .unwrap();

        println!("{:.8} {:.8}", t, v_out);
    });
}
