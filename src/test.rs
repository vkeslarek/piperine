use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::netlist::{CircuitVariable, GND};
use crate::circuit::Circuit;
use crate::devices::builder::CircuitBuilderExt;
use crate::devices::voltage_source::Waveform::Step;
use crate::math::unit::UnitExt;
use crate::solver::Context;
use tracing::debug;

pub fn titan_test(grid_size: i32) {
    let mut circuit = Circuit::new("Titan RC Grid");

    println!(
        "Generating {}x{} RC Grid ({} nodes)...",
        grid_size,
        grid_size,
        grid_size * grid_size
    );
    let gen_start = std::time::Instant::now();

    for x in 0..grid_size {
        for y in 0..grid_size {
            let node_name = format!("n_{}_{}", x, y);

            circuit.capacitor(format!("C_{}_{}", x, y), node_name.clone(), GND, 1.0.nF());

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
    let steps = result.len();

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

    let last_step = result.last().expect("No result steps produced");

    let far_corner_name = format!("n_{}_{}", grid_size - 1, grid_size - 1);

    // Get Values
    let v_start = last_step.get_node("n_0_0").unwrap_or(0.0);
    let v_far = last_step.get_node(far_corner_name.clone()).unwrap_or(0.0);

    println!("V(n_0_0) final:   {:.4} V", v_start);
    println!("V({}) final: {:.4} V", far_corner_name, v_far);

    assert!(
        v_start > 4.9,
        "Input node did not stabilize to 5V! Got {:.2}V",
        v_start
    );
}

#[ignore]
#[test]
pub fn full_titan_test() {
    titan_test(2);
    titan_test(3);
    titan_test(4);
    titan_test(5);
    titan_test(6);
    titan_test(7);
    titan_test(8);
    titan_test(9);
    titan_test(10);
}

#[test]
#[cfg(test)]
pub fn test() {
    debug!("Starting test circuit simulation...");

    let mut circuit = Circuit::new("Diode DC Bias");

    // 5V -> Resistor -> Diode -> Ground
    circuit.voltage_source("V1", "in", GND, 5.0.V());
    circuit.resistor("R1", "in", "anode", 1.0.kOhms());
    circuit.diode("D1", "anode", GND);

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    let v_d = result.get_node("anode").unwrap();

    println!("Diode Forward Voltage: {:.4} V", v_d);

    // Expect standard silicon drop ~0.6V - 0.8V
    assert!(
        v_d > 0.6 && v_d < 0.8,
        "Diode voltage outside realistic range"
    );
}
