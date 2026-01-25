use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitVariable, GND};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use faer::{Par, set_global_parallelism};
use std::num::NonZeroUsize;
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

// #[cfg(test)]
// mod tests;

static INIT: Once = Once::new();

pub fn init_config() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
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

    let mut circuit = Circuit::new("Diode DC Bias");

    // 5V -> Resistor -> Diode -> Ground
    circuit.voltage_source("V1", "in", GND, 5.0.V());
    circuit.resistor("R1", "in", "anode", 1.0.kOhms());
    circuit.diode("D1", "anode", GND);

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    let v_d = result
        .get_value(
            circuit
                .netlist()
                .reference_for(&CircuitVariable::Node("anode".into()))
                .unwrap(),
        )
        .unwrap();

    println!("Diode Forward Voltage: {:.4} V", v_d);

    // Expect standard silicon drop ~0.6V - 0.8V
    assert!(
        v_d > 0.6 && v_d < 0.8,
        "Diode voltage outside realistic range"
    );
}
