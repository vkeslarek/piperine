use piperine::analysis::transient::TransientAnalysisOptions;
use piperine::circuit::Circuit;
use piperine::circuit::netlist::{CircuitReference, GND};
use piperine::devices::voltage_source::Waveform::Step;
use piperine::init_config;
use piperine::math::unit::UnitExt;
use piperine::solver::Context;

pub fn main() {
    init_config();
    let grid_size = 150; // 50x50 = 2500 Nodes + Gnd
    let mut circuit = Circuit::new("Titan RC Grid");

    println!(
        "Generating {}x{} RC Grid ({} nodes)...",
        grid_size,
        grid_size,
        grid_size * grid_size
    );
    let gen_start = std::time::Instant::now();

    // 1. Generate Grid
    for x in 0..grid_size {
        for y in 0..grid_size {
            let node_name = format!("n_{}_{}", x, y);

            // Capacitor to Ground at every node
            circuit.capacitor(format!("C_{}_{}", x, y), node_name.clone(), GND, 1.0.nF());

            // Horizontal Resistor (connect to x+1)
            if x < grid_size - 1 {
                circuit.resistor(
                    format!("R_h_{}_{}", x, y),
                    node_name.clone(),
                    format!("n_{}_{}", x + 1, y),
                    1.0.kOhms(),
                );
            }

            // Vertical Resistor (connect to y+1)
            if y < grid_size - 1 {
                circuit.resistor(
                    format!("R_v_{}_{}", x, y),
                    node_name,
                    format!("n_{}_{}", x, y + 1),
                    1.0.kOhms(),
                );
            }
        }
    }

    // 2. Stimulus (Step Input at Corner 0,0)
    circuit.voltage_source(
        "V_Input",
        "n_0_0",
        GND,
        Step {
            initial: 0.0.V(),
            final_value: 5.0.V(),
            delay: 0.1.ms(),
            rise_time: 1.0.us(),
        },
    );

    println!("Netlist Generation: {:?}", gen_start.elapsed());

    // 3. Transient Analysis
    println!("Starting Transient Analysis...");
    let sim_start = std::time::Instant::now();

    let options = TransientAnalysisOptions {
        stop_time: 2.0.ms(),
        dt: 20.0.us(), // 100 Steps
    };

    // Note: We unwrap here to panic if simulation fails (which fails the test)
    let result = circuit
        .transient(options, Context::default())
        .expect("Analysis configuration failed")
        .solve()
        .expect("Simulation failed to converge");

    let total_time = sim_start.elapsed();
    let steps = result.values.len();

    // 4. Report
    println!("--------------------------------------------------");
    println!("TITAN BENCHMARK RESULTS (Release Mode Recommended)");
    println!("--------------------------------------------------");
    println!(
        "Grid Size:      {}x{} ({} nodes)",
        grid_size,
        grid_size,
        grid_size * grid_size
    );
    println!("Total Time:     {:?}", total_time);
    println!("Total Steps:    {}", steps);
    println!("Time/Step:      {:?}", total_time / steps as u32);
    println!("--------------------------------------------------");

    // Validation (Check if corner node charged)
    let last_step = result.values.last().unwrap();
    // We need to look up the index for n_0_0 and the far corner
    let n00_idx = *result
        .mapping
        .get(&CircuitReference::Node("n_0_0".into()))
        .unwrap();
    let v_start = last_step[n00_idx];

    assert!(
        v_start > 4.0,
        "Input node did not rise! Got {:.2}V",
        v_start
    );
}
