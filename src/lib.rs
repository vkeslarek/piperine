use crate::analysis::ac::AcSweepAnalysisOptions;
use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::pss::PssAnalysisOptions;
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
                dt: 5e-7,
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

#[test]
pub fn pss_test() {
    init_config();
    let mut circuit = Circuit::new("Test Rectifier PSS");

    let freq = 10.0.kHz(); // 10 kHz
    let period_val = 1.0 / 10000.0; // 100 microseconds

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
    circuit.resistor("R1", "vcc", "1", 10.0.Ohms());
    circuit.diode("D1", "1", "2");
    circuit.capacitor("C1", "2", GND, 10.0.uF());
    circuit.resistor("R2", "2", GND, 1.0.kOhms());

    let options = PssAnalysisOptions {
        period: period_val,    // 0.0001s
        dt: period_val / 50.0, // 50 points per period (2us)
        max_pss_iter: 20,      // PSS Newton usually converges in < 10 iters
        pss_reltol: 1e-4,
        t_stab: period_val * 500.0,
    };

    let result = circuit
        .pss(Context::default())
        .unwrap()
        .solve(options)
        .unwrap();

    println!("PSS Converged State: {:?}", result);
}

#[test]
pub fn pss_tran_val_test() {
    init_config();
    let mut circuit = Circuit::new("Test Rectifier PSS");

    let freq = 10.0.kHz(); // 10 kHz
    let period_val = 1.0 / 10000.0; // 100 microseconds

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
    circuit.resistor("R1", "vcc", "1", 10.0.Ohms());
    circuit.diode("D1", "1", "2");
    circuit.capacitor("C1", "2", GND, 10.0.uF());
    circuit.resistor("R2", "2", GND, 1.0.kOhms());

    let result = circuit
        .transient(
            TransientAnalysisOptions {
                stop_time: period_val * 10.0,
                dt: period_val,
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
            row_values[result.mapping[&CircuitReference::Node("2".into())]]
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

#[test]
pub fn test_torture_circuit() {
    init_config(); // Assuming you have logging setup
    let mut circuit = Circuit::new("Power Supply Torture Test");

    // 1. The Mains (325V Peak, 50Hz)
    // Node 1: AC Hot, Node 0: GND
    circuit.voltage_source(
        "V_MAINS",
        "ac_hot", // Node 1
        GND,      // Node 0
        Sine {
            amplitude: 325.0.V(),
            frequency: 50.0.Hz(),
            phase: 0.0.deg(),
        },
    );

    // 2. The Bridge Rectifier (Nodes: 1=AC, 0=GND, 2=DC+, 3=DC-)
    // Note: We need a transformer or floating source for a true bridge,
    // but here we can just rectify relative to ground for simplicity,
    // OR build a true bridge if we treat V_MAINS as floating.
    // Let's do a Half-Wave Precision stress test instead (Simpler topology, same math stress).
    // Source -> Low Res -> Diode -> Cap -> Ground

    // Series Resistance (The "Stiffness" Creator)
    circuit.resistor("R_SERIES", "ac_hot", "anode", 0.01.Ohms());

    // The Diode (The Switch)
    circuit.diode("D1", "anode", "cathode");

    // The Massive Capacitor
    circuit.capacitor("C_BIG", "cathode", GND, 0.1.F()); // 100mF

    // The Bleeder Resistor (Leakage)
    circuit.resistor("R_LOAD", "cathode", GND, 10_000.0.Ohms());

    // Simulation: Run for 100ms (5 cycles)
    // CRITICAL: Try with your fixed 5e-7 timestep.
    // It might take a while to run, but look at the PEAKS.
    let result = circuit
        .transient(
            TransientAnalysisOptions {
                stop_time: 0.01,
                dt: 1e-6, // 1 microsecond
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
            row_values[result.mapping[&CircuitReference::Branch(BranchIdentifier {
                component: "V_MAINS".into(),
                name: None
            })]]
        );
    }
}

#[test]
pub fn ac_testing_circuit() {
    init_config();
    let mut circuit = Circuit::new("RC Low Pass Test");

    // 1V AC Source (Phasor 1 + 0j)
    circuit.voltage_source(
        "V1",
        "in",
        GND,
        Sine {
            amplitude: 1.0.V(),
            frequency: 1.0.kHz(),
            phase: 0.0.deg(),
        },
    );

    circuit.resistor("R1", "in", "out", 1.0.kOhms());
    circuit.capacitor("C1", "out", GND, 159.15.nF()); // Cutoff ~1kHz

    let result = circuit
        .ac(Context::default())
        .unwrap()
        .solve_sweep(AcSweepAnalysisOptions {
            start_frequency: 10.0,
            stop_frequency: 100_000.0,
            steps: 50,
            logarithmic: true,
        })
        .unwrap();

    for (i, &f) in result.frequencies.iter().enumerate() {
        let v_out = result
            .get_phasor(&CircuitReference::Node("out".into()), i)
            .unwrap();
        println!(
            "Complex: {:?}, Freq: {:.2} Hz, Mag: {:.4} V, Phase: {:.2} deg",
            v_out,
            f,
            v_out.norm(),
            v_out.arg().to_degrees()
        );
    }
}

#[test]
pub fn test_diode_ac_bias_dependency() {
    init_config();

    // We will run the same circuit with two different DC bias voltages
    let biases = vec![0.0, 0.7]; // 0V (OFF) and 0.7V (ON)

    for dc_bias in biases {
        let mut circuit = Circuit::new(format!("Diode AC Test - Bias {}V", dc_bias));

        // 1. DC Bias + AC Signal
        // We use a Sine source for AC, but the DC solver sees the 'bias'
        circuit.voltage_source(
            "V1",
            "in",
            GND,
            Sine {
                amplitude: 1.0.V(),
                frequency: 1.0.kHz(),
                phase: 0.0.deg(),
            },
        );

        // Offset the input to bias the diode
        // (Assuming your VoltageSource supports a DC offset or we use a separate DC source)
        circuit.voltage_source("VBIAS", "bias_node", "in", dc_bias.V());

        // 2. The Diode under test
        // Connects from the biased input to the output
        circuit.diode("D1", "bias_node", "out");

        // 3. Load Resistor
        circuit.resistor("R_LOAD", "out", GND, 10.0.kOhms());

        // Run AC Sweep at 1kHz
        let result = circuit
            .ac(Context::default())
            .unwrap()
            .solve_sweep(AcSweepAnalysisOptions {
                start_frequency: 1000.0,
                stop_frequency: 1000.0,
                steps: 1,
                logarithmic: false,
            })
            .unwrap();

        let v_out = result
            .get_phasor(&CircuitReference::Node("out".into()), 0)
            .unwrap();

        println!(
            "Bias: {}V | Vout Mag: {:.4}V | Phase: {:.2} deg | g_d: {:.4e} S",
            dc_bias,
            v_out.norm(),
            v_out.arg().to_degrees(),
            v_out.norm() / (1.0 - v_out.norm()) / 10000.0 // rough g_d estimation
        );
    }
}

#[test]
pub fn test_noise_verification() {
    init_config();
    // 1. Setup - The "Johnson-Nyquist" Test
    // Theory: A resistor R in parallel with a capacitor C produces total integrated noise voltage
    // V_rms = sqrt(k * T / C), independent of Resistance!
    //
    // Constants:
    // k = 1.380649e-23
    // T = 300.15 K (Default SPICE temp 27C)
    // C = 1.0 nF
    // Expected V_rms = sqrt(1.38e-23 * 300.15 / 1e-9) = 2.035 uV

    let mut circuit = Circuit::new("Noise Verification - RC");

    // We rely on the resistor's internal thermal noise model.
    // The Voltage Source is a short to ground (0V) effectively.
    circuit
        .resistor("R1", "out", GND, 100.0.kOhms())
        .with_noise(true);
    circuit.capacitor("C1", "out", GND, 1.0.nF());

    // 2. Configure Noise Analysis
    // Bandwidth: RC Cutoff is 1 / (2*pi*100k*1n) ≈ 1.59 kHz
    // We need to sweep well beyond this to capture the total energy (integrate to infinity).
    let result = circuit
        .noise(
            NoiseAnalysisOptions {
                sweep_options: AcSweepAnalysisOptions {
                    start_frequency: 1.0,  // 1 Hz
                    stop_frequency: 1.0e6, // 1 MHz (>> 1.59 kHz)
                    steps: 500,            // High resolution for integration accuracy
                    logarithmic: true,
                },
                output_node: "out".into(),
                reference_node: GND,
                input_source_name: None,
            },
            Context::default(),
        )
        .unwrap()
        .solve()
        .unwrap();

    // 3. Validation
    let k_b = 1.380649e-23;
    let temp = 300.15;
    let cap = 1.0e-9;

    let expected_rms = f64::sqrt(k_b * temp / cap);
    let simulated_rms = result.integrated_noise;

    println!("--- Noise Simulation Results ---");
    println!("Circuit: R=100k, C=1nF");
    println!("Theory (sqrt(kT/C)): {:.4} uV", expected_rms * 1e6);
    println!("Simulated:           {:.4} uV", simulated_rms * 1e6);

    let error_pct = (simulated_rms - expected_rms).abs() / expected_rms * 100.0;
    println!("Error: {:.2}%", error_pct);

    // Allow small error due to finite integration range (1Hz-1MHz vs 0-Inf)
    assert!(
        error_pct < 2.0,
        "Noise simulation deviated significantly from theory!"
    );
}
