use piperine::analysis::transient::TransientAnalysisOptions;
use piperine::circuit::Circuit;
use piperine::circuit::netlist::{CircuitVariable, GND};
use piperine::devices::voltage_source::Waveform::Step;
use piperine::math::unit::UnitExt;
use piperine::solver::Context;

pub fn main() {
    let grid_size = 50;
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
        "Grid Size:      {}x{} ({} nodes + Ground)",
        grid_size,
        grid_size,
        grid_size * grid_size
    );
    println!("Total Time:     {:?}", total_time);
    println!("Total Steps:    {}", steps);
    println!(
        "Time/Step:      {:?}",
        if steps > 0 {
            total_time / steps as u32
        } else {
            std::time::Duration::ZERO
        }
    );
    println!("--------------------------------------------------");

    // 5. Validation
    let last_step = result.values.last().expect("No result steps produced");

    // Retrieve Keys
    let n00_key = circuit
        .netlist()
        .reference_for(&CircuitVariable::Node("n_0_0".into()))
        .expect("Node n_0_0 not found")
        .variable();

    // Optional: Check far corner to see propagation delay
    let far_corner_name = format!("n_{}_{}", grid_size - 1, grid_size - 1);
    let far_corner_key = circuit
        .netlist()
        .reference_for(&CircuitVariable::Node(far_corner_name.clone().into()))
        .expect("Far corner node not found")
        .variable();

    // Get Values
    let v_start = *last_step.values.get(n00_key).unwrap_or(&0.0);
    let v_far = *last_step.values.get(far_corner_key).unwrap_or(&0.0);

    println!("V(n_0_0) final:   {:.4} V", v_start);
    println!("V({}) final: {:.4} V", far_corner_name, v_far);

    assert!(
        v_start > 4.9,
        "Input node did not stabilize to 5V! Got {:.2}V",
        v_start
    );
}
