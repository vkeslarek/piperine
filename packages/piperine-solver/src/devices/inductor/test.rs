use crate::analysis::ac::AcSweepAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::builder;
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::GND;
use crate::devices::source::Waveform::{Sine, Step};
use crate::math::unit::UnitExt;
use crate::solver::Context;

#[test]
fn test_dc_inductor_short() {
    let mut circuit: CircuitInstance = builder("DC Inductor Short", |builder| {
        builder.voltage_source("V1", "in", GND, 10.0.V());
        builder.resistor("R1", "in", "out", 1.0.kOhms());
        builder.inductor("L1", "out", GND, 1.0.mH());
    })
    .into();

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_out = result.get_node("out").unwrap();

    println!("Inductor DC Voltage: {:.4} V", v_out);
    assert!(v_out.abs() < 1e-9, "Inductor is not a short in DC!");
}

#[test]
fn test_ac_lc_resonance() {
    let mut circuit: CircuitInstance = builder("AC LC Resonance", |builder| {
        builder.voltage_source(
            "V1",
            "in",
            GND,
            Sine {
                amplitude: 1.0.V(),
                frequency: 0.0.Hz(),
                phase: 0.0.deg(),
            },
        );
        builder.resistor("R_limit", "in", "tank", 1.0.Ohms());
        builder.inductor("L1", "tank", GND, 1.0.mH());
        builder.capacitor("C1", "tank", GND, 1.0.uF());
    })
    .into();

    let sweep = AcSweepAnalysisOptions {
        start_frequency: 1000.0,
        stop_frequency: 10000.0,
        steps: 100,
        logarithmic: true,
    };

    let result = circuit
        .ac(Context::default())
        .unwrap()
        .solve_sweep(sweep)
        .unwrap();

    let mut max_mag = 0.0;
    for vector in result.iter() {
        let mag = vector.get_node("tank").unwrap().norm();
        if mag > max_mag {
            max_mag = mag;
        }
    }

    println!("Max Tank Voltage at Resonance: {:.4} V", max_mag);
    assert!(max_mag > 0.9, "LC Tank did not show resonance peak");
}

#[test]
fn test_transient_rl_current_rise() {
    let mut circuit: CircuitInstance = builder("RL Step Response", |builder| {
        builder.voltage_source(
            "V1",
            "in",
            GND,
            Step {
                initial: 0.0.V(),
                final_value: 10.0.V(),
                delay: 0.0,
                rise_time: 1.0.us(),
            },
        );

        builder.resistor("R1", "in", "out", 1.0.kOhms());
        builder.inductor("L1", "out", GND, 1.0.H());
    })
    .into();

    let options = TransientAnalysisOptions {
        stop_time: 5.0.ms(),
        dt: 50.0.us(),
    };

    let result = circuit
        .transient(options, Context::default())
        .unwrap()
        .solve()
        .unwrap();

    let one_ms_step = result
        .iter()
        .find(|s| (s.time() - 0.001).abs() < 1e-6)
        .expect("1ms point not found");

    let i_l = one_ms_step.get_branch("L1").unwrap();

    println!("Current at 1ms (1 Tau): {:.4} mA", i_l * 1000.0);
    assert!((i_l - 0.00632).abs() < 0.0005, "RL rise time incorrect!");
}
