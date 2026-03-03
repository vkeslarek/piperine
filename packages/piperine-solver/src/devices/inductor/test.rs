use crate::analysis::ac::AcSweepAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::GND;
use crate::circuit::Circuit;
use crate::devices::source::Waveform::{Sine, Step};
use crate::math::unit::UnitExt;
use crate::solver::Context;

#[test]
fn test_dc_inductor_short() {
    let mut v_out = GND;

    let mut circuit: CircuitInstance = Circuit::builder("DC Inductor Short", |b| {
        let v_in = b.port();
        v_out = b.port();

        b.voltage_source("V1", v_in.clone(), GND, 10.0.V());
        b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
        b.inductor("L1", v_out.clone(), GND, 1.0.mH());
    })
    .into();

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_out_value = result.get_node(&v_out).unwrap();

    println!("Inductor DC Voltage: {:.4} V", v_out_value);
    assert!(v_out_value.abs() < 1e-9, "Inductor is not a short in DC!");
}

#[test]
fn test_ac_lc_resonance() {
    let mut v_tank = GND;

    let mut circuit: CircuitInstance = Circuit::builder("AC LC Resonance", |b| {
        let v_in = b.port();
        v_tank = b.port();

        b.voltage_source(
            "V1",
            v_in.clone(),
            GND,
            Sine {
                amplitude: 1.0.V(),
                frequency: 0.0.Hz(),
                phase: 0.0.deg(),
            },
        );
        b.resistor("R_limit", v_in, v_tank.clone(), 1.0.Ohms());
        b.inductor("L1", v_tank.clone(), GND, 1.0.mH());
        b.capacitor("C1", v_tank.clone(), GND, 1.0.uF());
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
        let mag = vector.get_node(&v_tank).unwrap().norm();
        if mag > max_mag {
            max_mag = mag;
        }
    }

    println!("Max Tank Voltage at Resonance: {:.4} V", max_mag);
    assert!(max_mag > 0.9, "LC Tank did not show resonance peak");
}

#[test]
fn test_transient_rl_current_rise() {
    let mut circuit: CircuitInstance = Circuit::builder("RL Step Response", |b| {
        let v_in = b.port();
        let v_out = b.port();

        b.voltage_source(
            "V1",
            v_in.clone(),
            GND,
            Step {
                initial: 0.0.V(),
                final_value: 10.0.V(),
                delay: 0.0,
                rise_time: 1.0.us(),
            },
        );

        b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
        b.inductor("L1", v_out, GND, 1.0.H());
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
