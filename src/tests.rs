use crate::analysis::ac::AcSweepAnalysisOptions;
use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitVariable, GND};
use crate::devices::voltage_source::Waveform::{Sine, Step};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use faer::{Par, set_global_parallelism};
use std::num::NonZeroUsize;
use std::sync::Once;


#[test]
fn test_ac_rc_filter() {
    init_config();
    let mut circuit = Circuit::new("AC Low Pass");

    // R = 1k, C = 159.15nF => Cutoff approx 1kHz
    circuit.voltage_source(
        "V1",
        "in",
        GND,
        Sine {
            amplitude: 1.0.V(),
            frequency: 0.0.Hz(), // Placeholder, overridden by sweep
            phase: 0.0.deg(),
        },
    );
    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.capacitor("C1", "out", GND, 159.15.nF());

    let result = circuit
        .ac(Context::default())
        .unwrap()
        .solve_sweep(AcSweepAnalysisOptions {
            start_frequency: 100.0,
            stop_frequency: 10_000.0,
            steps: 21,
            logarithmic: true,
        })
        .unwrap();

    // 1. Resolve indices via the mapping ONCE
    // Frequency is now just a variable in the vector
    let freq_ref = CircuitVariable::Frequency;
    let out_ref = CircuitVariable::Node("out".into());

    let freq_idx = *result
        .mapping
        .get(&freq_ref)
        .expect("Frequency not in result");
    let out_idx = *result
        .mapping
        .get(&out_ref)
        .expect("Output node not in result");

    let mut found_cutoff = false;

    // 2. Iterate through the snapshots (rows)
    for vector in &result.values {
        // Extract frequency from this snapshot (Real part of Complex)
        let f = vector[freq_idx].re;

        if (f - 1000.0).abs() < 1.0 {
            // Found 1kHz point
            let v_out = vector[out_idx];
            let mag = v_out.norm();

            println!("At {:.1} Hz: Mag = {:.4} V (Expected ~0.707)", f, mag);

            assert!(
                (mag - 0.7071).abs() < 0.001,
                "Filter cutoff magnitude incorrect"
            );
            found_cutoff = true;
            break;
        }
    }

    assert!(found_cutoff, "Sweep did not cover 1kHz correctly.");
}



#[test]
fn test_noise_johnson_nyquist() {
    init_config();
    let mut circuit = Circuit::new("Noise Verification - RC");

    // Theory: Total Integrated Noise V_rms = sqrt(k * T / C)
    // R=100k, C=1nF, T=300.15K
    // Expected: ~2.035 uV
    circuit
        .resistor("R1", "out", GND, 100.0.kOhms())
        .with_noise(true);
    circuit.capacitor("C1", "out", GND, 1.0.nF());

    let result = circuit
        .noise(
            NoiseAnalysisOptions {
                sweep_options: AcSweepAnalysisOptions {
                    start_frequency: 1.0,
                    stop_frequency: 1.0e6,
                    steps: 500,
                    logarithmic: true,
                },
                output_node: "out".into(),
                reference_node: GND.into(),
                input_source_name: None,
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();

    let k_b = 1.380649e-23;
    let temp = 300.15;
    let cap = 1.0e-9;
    let expected_rms = f64::sqrt(k_b * temp / cap);
    let simulated_rms = result.integrated_noise;

    println!(
        "Theory: {:.4} uV | Sim: {:.4} uV",
        expected_rms * 1e6,
        simulated_rms * 1e6
    );

    let error_pct = (simulated_rms - expected_rms).abs() / expected_rms * 100.0;
    assert!(
        error_pct < 2.0,
        "Noise simulation accuracy error: {:.2}%",
        error_pct
    );
}


