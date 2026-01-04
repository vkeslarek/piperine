use crate::analysis::transient::TransientAnalysisContext;
use crate::circuit::Circuit;
use crate::component::StandardComponentsSpec;
use crate::math::unit::UnitExt;
use crate::model::ModelResolver;
use crate::model::cap::CapacitorIdealModel;
use crate::model::res::ResistorIdealModel;
use crate::model::vsrc::VoltageSourceIdealModel;
use crate::netlist::{BranchIdentifier, CircuitReference, GND, NodeIdentifier};
use crate::solver::{Context, Solver};

mod analysis;
mod circuit;
mod component;
mod error;
mod math;
mod model;
mod netlist;
mod solver;
mod state;

#[test]
pub fn test() {
    let mut model_resolver = ModelResolver::new();
    let circuit = Circuit::build("Test Circuit", |ctx| {
        ctx.voltage_source("VCC", "vcc", GND, 5.0.V());
        ctx.resistor("R1", "vcc", "cap", 10.0.Ohms());
        ctx.capacitor("C1", "cap", GND, 10.0.uF());
    })
    .instantiate(&mut model_resolver)
    .unwrap();

    let stop_time = 0.0012;
    let dt = 0.0000001;
    let solution = Solver::build(circuit, Context::default())
        .unwrap()
        .transient(TransientAnalysisContext {
            time: stop_time,
            dt,
        })
        .unwrap();

    solution.iter().for_each(|(time, vec)| {
        println!("{:0.9} {:?}", time, vec.get(&CircuitReference::Node(NodeIdentifier::Named("cap".to_string()))).cloned().unwrap_or(0.0));
    });
}
