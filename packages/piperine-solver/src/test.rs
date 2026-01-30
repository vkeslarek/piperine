use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::builder::{builder, CircuitBuilder, IntoCircuit};
use crate::circuit::netlist::{NodeIdentifier, GND};
use crate::circuit::Circuit;
use crate::devices::source::Waveform::Step;
use crate::math::unit::{Ohm, UnitExt};
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

    let mut circuit: Circuit = builder("Titan RC Grid", |builder| {
        for x in 0..grid_size {
            for y in 0..grid_size {
                let node_name = format!("n_{}_{}", x, y);

                builder.capacitor(format!("C_{}_{}", x, y), node_name.clone(), GND, 1.0.nF());

                if x < grid_size - 1 {
                    builder.resistor(
                        format!("R_h_{}_{}", x, y),
                        node_name.clone(),
                        format!("n_{}_{}", x + 1, y),
                        1.0.kOhms(),
                    );
                }

                // Vertical Resistor (connect to y+1)
                if y < grid_size - 1 {
                    builder.resistor(
                        format!("R_v_{}_{}", x, y),
                        node_name,
                        format!("n_{}_{}", x, y + 1),
                        1.0.kOhms(),
                    );
                }
            }
        }

        builder.voltage_source(
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
    })
    .into();

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
    titan_test(50);
    titan_test(100);
    titan_test(150);
    titan_test(200);
}

#[test]
#[cfg(test)]
pub fn test() {
    debug!("Starting test circuit simulation...");

    let mut circuit: Circuit = builder("Diode DC Bias", |builder: &mut CircuitBuilder| {
        builder.voltage_source("V1", "in", GND, 5.0.V());
        builder.resistor("R1", "in", "anode", 1.0.kOhms());
        builder.diode("D1", "anode", GND);
    })
    .into();

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
    let v_d = result.get_node("anode").unwrap();

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
