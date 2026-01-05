use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::Circuit;
use crate::component::StandardComponentsSpec;
use crate::math::unit::UnitExt;
use crate::model::ModelResolver;
use crate::netlist::{CircuitReference, GND, NodeIdentifier};
use crate::solver::{Context, Solver};
use num_traits::Zero;

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
        ctx.resistor("R1", "vcc", 1, 10.0.Ohms());
        ctx.diode("D1", 1, GND);
        ctx.capacitor("C1", 2, GND, 10.0.uF());
        ctx.resistor("R2", 2, GND, 10.0.Ohms());
    })
    .instantiate(&mut model_resolver)
    .unwrap();

    let stop_time = 0.012;
    let dt = 0.000001;
    let solution = Solver::build(circuit, Context::default())
        .unwrap()
        .transient(TransientAnalysisOptions { stop_time, dt })
        .unwrap();

    // println!("{:?}", solution);
    solution.iter().for_each(|(freq, vec)| {
        let cap = vec
            .get(&CircuitReference::Node(NodeIdentifier::Indexed(2)))
            .cloned()
            .unwrap_or(0.0);

        println!("{:0.9} {:0.9}", freq, cap);
    });
}
