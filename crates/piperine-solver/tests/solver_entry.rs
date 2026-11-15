//! The solver's entry points, from the outside: build a circuit from a
//! hand-written `Device` and drive each analysis through its public entry,
//! including the typed `SolverDomain` failures.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use std::result::Result;
use std::sync::{Arc, OnceLock};

use abi_devices::{add_resistor, add_vsrc, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, Port};
use piperine_solver::prelude::*;

fn create_divider(r1_val: f64, r2_val: f64) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("test");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 10.0, &top, &gnd);
    add_resistor(&mut b, "r1", r1_val, &top, &mid);
    add_resistor(&mut b, "r2", r2_val, &mid, &gnd);
    (b.build().expect("build"), mid)
}

#[test]
fn test_solver_entry_point() {
    let (circuit1, node1) = create_divider(1000.0, 1000.0);
    let (circuit2, node2) = create_divider(1000.0, 3000.0);
    let mut solver1 = Solver::new(circuit1).build();
    let mut solver2 = Solver::new(circuit2).build(); // test AC2

    let mut dc1 = solver1.dc().unwrap();
    let res1 = dc1.solve().unwrap();
    let mid1 = res1.get_node(&node_id(&node1)).unwrap();
    assert!((mid1 - 5.0).abs() < 1e-9);

    let mut dc2 = solver2.dc().unwrap();
    let res2 = dc2.solve().unwrap();
    let mid2 = res2.get_node(&node_id(&node2)).unwrap();
    assert!((mid2 - 7.5).abs() < 1e-9);
}

/// A device whose creation fails: the error reaches the host at build.
struct BadElement;
static BAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static BAD: OnceLock<Module> = OnceLock::new();

fn bad_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec::continuous("p", role::PORT));
    s.freeze()
}

impl Device for BadElement {
    fn prototype() -> &'static Shape<'static> {
        BAD_SHAPE.get_or_init(bad_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let _ = Arc::clone(BAD_SHAPE.get_or_init(bad_shape));
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
