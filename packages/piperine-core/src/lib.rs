use crate::circuit::{BranchIdentifier, Circuit, Netlist, NodeIdentifier};
use crate::component::cap::{Capacitor, CapacitorParameters};
use crate::component::dio::{Diode, DiodeParameters};
use crate::component::ind::{Inductor, InductorParameters};
use crate::component::res::{Resistor, ResistorParameters};
use crate::component::vsrc::{VoltageSource, VoltageSourceParameters};
use crate::component::{Components, Context};
use crate::measure::Measure;
use crate::model::cap::CapacitorIdealModel;
use crate::model::dio::DiodeShockleyModel;
use crate::model::ind::InductorIdealModel;
use crate::model::res::{ResistorCompleteModel, ResistorCompleteModelParameters};
use crate::model::vsrc::VoltageSourceIdealModel;
use crate::solver::CircuitSolver;
use std::sync::Arc;

mod analysis;
mod circuit;
mod component;
mod error;
mod measure;
mod model;
mod solver;
mod state;
mod numerical_method;

#[test]
pub fn test() {
    let resistor_model = Arc::new(ResistorCompleteModel::new(
        ResistorCompleteModelParameters {
            name: "DefaultIdealResistor".to_string(),
            ..Default::default()
        },
    ));

    let voltage_source_model = Arc::new(VoltageSourceIdealModel::new(
        "DefaultIdealVoltageSource".to_string(),
    ));
    let capacitor_model = Arc::new(CapacitorIdealModel::new(
        "DefaultIdealCapacitor".to_string(),
    ));
    let inductor_model = Arc::new(InductorIdealModel::new("DefaultInductor".to_string()));
    let diode_model = Arc::new(DiodeShockleyModel::new("DefaultDiodeShockley".to_string()));

    let mut netlist = Netlist::new();

    let resistor = Resistor::new(
        &mut netlist,
        ResistorParameters {
            name: "R1".to_string(),
            model: resistor_model.clone(),
            node_plus: NodeIdentifier::Indexed(0),
            node_minus: NodeIdentifier::Indexed(1),
            resistance: Some(10.0),
            ..Default::default()
        },
    )
    .expect("Failed to instantiate a resistor");

    let voltage_source = VoltageSource::new(
        &mut netlist,
        VoltageSourceParameters {
            name: "V1".to_string(),
            model: voltage_source_model,
            node_plus: NodeIdentifier::Indexed(0),
            node_minus: NodeIdentifier::Gnd,
            voltage: 5.0,
        },
    )
    .expect("Failed to instantiate a voltage source");

    let capacitor = Capacitor::new(
        &mut netlist,
        CapacitorParameters {
            name: "C1".to_string(),
            model: capacitor_model,
            node_plus: NodeIdentifier::Indexed(1),
            node_minus: NodeIdentifier::Indexed(2),
            capacitance: 10e-6,
        },
    )
    .expect("Failed to instantiate a capacitor");

    let inductor = Inductor::new(
        &mut netlist,
        InductorParameters {
            name: "L1".to_string(),
            model: inductor_model,
            node_plus: NodeIdentifier::Indexed(2),
            node_minus: NodeIdentifier::Gnd,
            inductance: 1e-3,
        },
    )
    .expect("Failed to instantiate an inductor");

    let diode = Diode::new(
        &mut netlist,
        DiodeParameters {
            name: "D1".to_string(),
            model: diode_model,
            node_plus: NodeIdentifier::Indexed(2),
            node_minus: NodeIdentifier::Gnd,
            saturation_current: 1e-9,
            emission_coefficient: 1.3,
        },
    )
    .expect("Failed to instantiate an diode");

    let mut components = Components::new();

    components.add_component(Box::new(resistor));
    components.add_component(Box::new(voltage_source));
    components.add_component(Box::new(capacitor));
    components.add_component(Box::new(inductor));
    components.add_component(Box::new(diode));

    let mut measures = Vec::new();
    measures.push(Measure::Current(BranchIdentifier {
        component: "D1".to_string(),
        name: None,
    }));
    measures.push(Measure::Voltage(NodeIdentifier::Indexed(1)));

    let circuit = Circuit::new(components, netlist, measures);
    let context = Context::default();

    let mut solver =
        CircuitSolver::new(circuit, &context).expect("Failed to create circuit solver");

    let stop_time = 0.005;
    let dt = 0.00001;
    let solution = solver.solve_transient(stop_time, dt, &context);
    // println!("{:?}", solution);
    let mut idx = 0;
    solution.unwrap().iter().for_each(|vec| {
        println!("{} {:0.8}", idx, vec[4]);
        idx += 1;
    });

    // let results = solver.solve_ac_sweep(10.0, 1e6, 100, true).unwrap();
    //
    // for (freq, sol) in results {
    //     // sol[1] might be your output node voltage
    //     let magnitude = sol[1].norm();
    //     let phase = sol[1].arg() * 180.0 / std::f64::consts::PI;
    //     println!(
    //         "Freq: {} Hz, Mag: {} V, Phase: {} deg",
    //         freq, magnitude, phase
    //     );
    // }
}
