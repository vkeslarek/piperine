use crate::analysis::ac::AcSweepAnalysisOptions;
use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, GND};
use crate::devices::voltage_source::Waveform::{Sine, Step};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use faer::{Par, set_global_parallelism};
use std::num::NonZeroUsize;
use std::sync::Once;

static INIT: Once = Once::new();

fn init_config() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_thread_ids(true)
            .with_thread_names(true)
            .init();

        set_global_parallelism(Par::Rayon(NonZeroUsize::new(4).unwrap()));
    });
}


#[test]
fn test_dc_resistive_divider() {
    init_config();
    let mut circuit = Circuit::new("DC Divider");

    // 10V Source
    circuit.voltage_source("V1", "in", GND, 10.0.V());

    // 50/50 Divider
    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.resistor("R2", "out", GND, 1.0.kOhms());

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_out = result
        .get_value(&CircuitReference::Node("out".into()))
        .unwrap();

    println!("DC Divider: V_out = {:.4} V", v_out);
    assert!((v_out - 5.0).abs() < 1e-6, "Divider failed: Expected 5.0V");
}

#[test]
fn test_dc_diode_bias() {
    init_config();
    let mut circuit = Circuit::new("Diode DC Bias");

    // 5V -> Resistor -> Diode -> Ground
    circuit.voltage_source("V1", "in", GND, 5.0.V());
    circuit.resistor("R1", "in", "anode", 1.0.kOhms());
    circuit.diode("D1", "anode", GND);

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    let v_d = result
        .get_value(&CircuitReference::Node("anode".into()))
        .unwrap();

    println!("Diode Forward Voltage: {:.4} V", v_d);

    // Expect standard silicon drop ~0.6V - 0.8V
    assert!(
        v_d > 0.6 && v_d < 0.8,
        "Diode voltage outside realistic range"
    );
}

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
    let freq_ref = CircuitReference::Frequency;
    let out_ref = CircuitReference::Node("out".into());

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
fn test_transient_rc_step() {
    init_config();
    let mut circuit = Circuit::new("RC Step Response");

    circuit.voltage_source(
        "V1",
        "in",
        GND,
        Step {
            initial: 0.0.V(),
            final_value: 1.0.V(),
            delay: 0.0,
            rise_time: 1e-9,
        },
    );

    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.capacitor("C1", "out", GND, 1.0.uF());

    let result = circuit
        .transient(
            TransientAnalysisOptions {
                stop_time: 5.0.ms(), // 5 Tau
                dt: 100.0.us(),
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();

    // 1. Resolve the column index for "out"
    let out_idx = *result
        .mapping
        .get(&CircuitReference::Node("out".into()))
        .expect("Output node not found");

    // 2. Get the last snapshot (Final State) directly from the vector
    let final_snapshot = result.values.last().expect("Simulation returned no data");
    let v_final = final_snapshot[out_idx];

    // Optional: If you wanted to verify the time of this snapshot
    // let time_idx = *result.mapping.get(&CircuitReference::Time).unwrap();
    // let t_final = final_snapshot[time_idx];

    println!("Transient Final Voltage: {:.4} V", v_final);
    assert!(
        (v_final - 1.0).abs() < 0.01,
        "Capacitor did not charge to 1V. Got {}",
        v_final
    );
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

#[test]
fn test_dc_floating_node_crash() {
    init_config();
    let mut circuit = Circuit::new("Floating Node (Series Caps)");

    circuit.voltage_source("V1", "in", GND, 10.0.V());

    circuit.capacitor("C1", "in", "mid", 1.0.uF());
    circuit.capacitor("C2", "mid", GND, 1.0.uF());

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_mid = result
        .get_value(&CircuitReference::Node("mid".into()))
        .unwrap();

    println!("Floating Node Voltage (stabilized by Gmin): {:.4} V", v_mid);

    assert!(
        (v_mid - 5.0).abs() < 1e-3,
        "Gmin failed to stabilize floating node! Expected 5.0V, got {}",
        v_mid
    );
}

#[test]
fn test_transient_rc_charging() {
    init_config();
    let mut circuit = Circuit::new("RC Transient Demo");

    // 1. A Step Source: 0V -> 5V at t=0
    circuit.voltage_source(
        "V1",
        "in",
        GND,
        Step {
            initial: 0.0.V(),
            final_value: 5.0.V(),
            delay: 0.0,
            rise_time: 1.0.us(), // Sharp rise
        }
    );

    // 2. RC Network (tau = 1ms)
    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.capacitor("C1", "out", GND, 1.0.uF());

    // 3. Run Transient Analysis
    let options = TransientAnalysisOptions {
        stop_time: 5.0.ms(),
        dt: 100.0.us(), // Sample 10 times per time constant
    };

    let result = circuit
        .transient(options, Context::default())
        .unwrap()
        .solve()
        .unwrap();

    let out_idx = *result.mapping.get(&CircuitReference::Node("out".into())).unwrap();
    let time_idx = *result.mapping.get(&CircuitReference::Time).unwrap();

    let one_tau_step = result.values.iter().find(|v| (v[time_idx] - 0.001).abs() < 1e-6).unwrap();
    let v_at_1ms = one_tau_step[out_idx];

    println!("At 1ms (1 Tau): {:.4} V", v_at_1ms);
    assert!((v_at_1ms - 3.16).abs() < 0.1);

    // Check at t = 5ms (End)
    let final_v = result.values.last().unwrap()[out_idx];
    println!("At 5ms (Final): {:.4} V", final_v);
    assert!((final_v - 5.0).abs() < 0.05);
}