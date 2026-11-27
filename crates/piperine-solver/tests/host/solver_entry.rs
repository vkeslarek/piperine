//! The solver's entry points, from the outside: build a circuit from a
//! hand-written `Device` and drive each analysis through its public entry,
//! including the typed `SolverDomain` failures.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::partials::PartialsCheck;
use std::result::Result;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, Port};
use piperine_solver::prelude::*;

#[test]
fn test_solver_entry_point() {
    let (circuit1, node1) = SolverEntryCases::create_divider(1000.0, 1000.0);
    let (circuit2, node2) = SolverEntryCases::create_divider(1000.0, 3000.0);
    let mut solver1 = Solver::new(circuit1).build();
    let mut solver2 = Solver::new(circuit2).build(); // test AC2

    let mut dc1 = solver1.dc().unwrap();
    let res1 = dc1.solve().unwrap();
    let mid1 = res1.get_node(&CircuitBuilding::node_id(&node1)).unwrap();
    assert!((mid1 - 5.0).abs() < 1e-9);

    let mut dc2 = solver2.dc().unwrap();
    let res2 = dc2.solve().unwrap();
    let mid2 = res2.get_node(&CircuitBuilding::node_id(&node2)).unwrap();
    assert!((mid2 - 7.5).abs() < 1e-9);
}

/// A device whose creation fails: the error reaches the host at build.
struct BadElement;
static BAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static BAD: OnceLock<Module> = OnceLock::new();

impl Device for BadElement {
    fn prototype() -> &'static Shape<'static> {
        BAD_SHAPE.get_or_init(SolverEntryCases::bad_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let _ = Arc::clone(BAD_SHAPE.get_or_init(SolverEntryCases::bad_shape));
        Err(Fault::new(1, "setup error"))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn test_solver_setup_error_propagation() {
    let mut b = CircuitBuilder::new("bad");
    let gnd = b.ground();
    b.device(BAD.get_or_init(|| module::<BadElement>("BadElement")), "bad", &[], &[Port::Node(gnd)]);
    let result = b.build();
    assert!(result.is_err());
    let err_str = result.err().unwrap().to_string();
    assert!(err_str.contains("setup error"));
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("BadElement", BAD.get_or_init(|| module::<BadElement>("BadElement")));
}

/// The helpers of the solver_entry tests.
pub struct SolverEntryCases;

impl SolverEntryCases {
    /// The create divider helper `create_divider`.
    fn create_divider(r1_val: f64, r2_val: f64) -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("test");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", r1_val, &top, &mid);
        CircuitBuilding::add_resistor(&mut b, "r2", r2_val, &mid, &gnd);
        (b.build().expect("build"), mid)
    }

    /// The bad shape helper `bad_shape`.
    fn bad_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec::continuous("p", role::PORT));
        s.freeze()
    }
}
