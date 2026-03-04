use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::{NodeIdentifier, GND};
use crate::circuit::Circuit;
use crate::devices::source::Waveform::Step;
use crate::math::unit::UnitExt;
use crate::solver::Context;
use tracing::debug;

pub fn titan_test(grid_size: i32) {
    let gen_start = std::time::Instant::now();
    println!(
        "Generating {}x{} RC Grid ({} nodes)...",
        grid_size,
        grid_size,
        grid_size * grid_size
    );

    // We need to track two nodes: start and far corner
    let mut n_start = GND;
    let mut n_far_corner = GND;

    let mut circuit: CircuitInstance = Circuit::builder("Titan RC Grid", |builder| {
        // Create a grid of nodes
        let mut nodes = vec![vec![NodeIdentifier::Gnd; grid_size as usize]; grid_size as usize];

        // Create all nodes first
        for x in 0..grid_size {
            for y in 0..grid_size {
                nodes[x as usize][y as usize] = builder.port();
            }
        }

        // Store the nodes we need to track
        n_start = nodes[0][0].clone();
        n_far_corner = nodes[(grid_size - 1) as usize][(grid_size - 1) as usize].clone();

        // Now connect components
        for x in 0..grid_size {
            for y in 0..grid_size {
                let node = nodes[x as usize][y as usize].clone();

                builder.capacitor(format!("C_{}_{}", x, y), node.clone(), GND, 1.0.nF());

                if x < grid_size - 1 {
                    builder.resistor(
                        format!("R_h_{}_{}", x, y),
                        node.clone(),
                        nodes[(x + 1) as usize][y as usize].clone(),
                        1.0.kOhms(),
                    );
                }

                // Vertical Resistor (connect to y+1)
                if y < grid_size - 1 {
                    builder.resistor(
                        format!("R_v_{}_{}", x, y),
                        node,
                        nodes[x as usize][(y + 1) as usize].clone(),
                        1.0.kOhms(),
                    );
                }
            }
        }

        builder.voltage_source(
            "V_Input",
            n_start.clone(),
            GND,
            Step {
                initial: 0.0.V(),
                final_value: 5.0.V(),
                delay: 0.1.ms(),
                rise_time: 1.0.us(),
            },
        );
    })
    .into();

    println!("Netlist Generation: {:?}", gen_start.elapsed());

    println!("Starting Transient Analysis...");
    let sim_start = std::time::Instant::now();

    let options = TransientAnalysisOptions::new(2.0.ms(), 20.0.us());

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

    // Get Values using NodeIdentifier references
    let v_start = last_step.get_node(&n_start).unwrap_or(0.0);
    let v_far = last_step.get_node(&n_far_corner).unwrap_or(0.0);

    println!("V({}) final:   {:.4} V", n_start, v_start);
    println!("V({}) final: {:.4} V", n_far_corner, v_far);

    assert!(
        v_start > 4.9,
        "Input node did not stabilize to 5V! Got {:.2}V",
        v_start
    );
}

#[ignore]
#[test]
pub fn full_titan_test() {
    titan_test(50);
    titan_test(100);
    titan_test(150);
    titan_test(200);
}

#[test]
#[cfg(test)]
pub fn test() {
    debug!("Starting test circuit simulation...");

    let mut anode = GND;

    let mut circuit: CircuitInstance = Circuit::builder("Diode DC Bias", |b| {
        let iin = b.port();
        anode = b.port();
        b.voltage_source("V1", iin.clone(), GND, 5.0.V());
        b.resistor("R1", iin, anode.clone(), 1.0.kOhms());
        b.diode("D1", anode.clone(), GND);
    })
    .into();

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    let v_d = result.get_node(&anode).unwrap();

    println!("Diode Forward Voltage: {:.4} V", v_d);

    assert!(
        v_d > 0.6 && v_d < 0.8,
        "Diode voltage outside realistic range"
    );
}

// fn controller(
//     shunt_voltage_plus: impl Into<VoltageMeasure>,
//     shunt_voltage_minus: impl Into<VoltageMeasure>,
//     shunt_resistance: Ohm,
//     mosfet_gate_drive: impl Into<NodeIdentifier>,
// ) -> impl IntoCircuit {
//     // Mock for now!
//     circuit("Controller", |builder: &mut CircuitBuilder| {
//         builder.proc_voltage_source("Mock Proc Source", 1, GND, |proc| {
//             let mosfet_gate_drive = mosfet_gate_drive.into();
//
//             proc.change_list(&[&shunt_voltage_plus, &shunt_voltage_minus]);
//             let duty_cycle =
//                 proc.measure(shunt_voltage_plus - shunt_voltage_minus) / shunt_resistance;
//             proc.pwm(mosfet_gate_drive.clone(), 12.0.MHz(), duty_cycle);
//         });
//         builder.resistor("R1", 1, mosfet_gate_drive, 220.0.Ohms())
//     })
// }
//
// fn buck_stage(
//     input: impl Into<NodeIdentifier>,
//     output: impl Into<NodeIdentifier>,
// ) -> impl IntoCircuit {
//     circuit("Buck Stage", |builder: &mut CircuitBuilder| {
//         // Fetch the model from the web. Can be specified manually
//         let transistor_model = builder.model_from_http("IFR520").unwrap();
//
//         // The MOSFET driver
//         builder.mosfet("Q1", input, "gate_drive", 1, transistor_model);
//
//         // The inductor and diode
//         builder.diode("D1", GND, 1);
//         builder.inductor("L1", 1, 2);
//
//         // Sensing and control
//         let shunt_resistor = 1.0.Ohms();
//         builder.resistor("Rsense", 2, output, 1.0.Ohms());
//         builder.subcircuit(controller(
//             Vp!("Rsense"),
//             Vm!("Rsense"),
//             shunt_resistor,
//             "gate_drive",
//         ));
//     })
// }
//
// #[test]
// #[cfg(test)]
// pub fn buck_design() {
//     let circuit = circuit("Buck design", |builder: &mut CircuitBuilder| {
//         builder.source("Vin", "in", GND, 5.0.V());
//
//         // Instance a buck stage
//         builder.subcircuit("Buck stage1", buck_stage("Vin", "Vout"));
//
//         // Simulate a range of loads
//         builder.resistor("Rload", "Vout", GND, 200.0.Ohms()..10.0.kOhms().uniform());
//     });
//
//     // ... The other simulations stuff
// }
