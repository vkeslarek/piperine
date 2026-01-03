use crate::circuit::{Circuit, GND};
use crate::math::unit::{
    DimensionLessExt, FaradsExt, HenrysExt, OhmsExt, TemperatureExt, VoltsExt,
};
use crate::solver::{Context, Solver};

mod analysis;
mod circuit;
mod component;
mod error;
mod math;
mod measure;
mod model;
mod numerical_method;
mod solver;
mod state;

#[test]
pub fn test() {
    let circuit = Circuit::build("Test Circuit", |ctx| {
        ctx.resistor("R1", 0, 1, 10.0.Ohms());
        ctx.voltage_source("VCC", 0, GND, 5.0.V());
        ctx.capacitor("C1", 1, 2, 10.0.uF());
        ctx.inductor("L1", 2, GND, 1.0.uH());
        ctx.diode("D1", 2, GND, 1.0.nV(), 1.3.ratio());
    });
    let stop_time = 0.005;
    let dt = 0.00001;

    let solution = Solver::build(circuit, Context::default()).transient(stop_time, dt);
    let mut idx = 0;
    solution.unwrap().iter().for_each(|vec| {
        println!("{} {:0.8}", idx, vec[4]);
        idx += 1;
    });
}
