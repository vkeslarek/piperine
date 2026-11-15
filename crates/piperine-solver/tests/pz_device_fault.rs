//! A device that cannot be evaluated at the pole-zero assembly fails the
//! analysis with an error naming the instance (never a poisoned matrix).

#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_analog_solver::netlist::{AnalogVariable, BranchIdentifier, NodeIdentifier};
use piperine_solver::abi::Port;
use piperine_solver::prelude::{CircuitBuilder, Context, PoleZeroOptions};

/// A conductance to ground that faults once `FREQ` is non-zero (an AC probe).
struct AcFault;
static SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static MODULE: OnceLock<Module> = OnceLock::new();

fn shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let _ = F64;
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: "pz_device_fault.rs", op: op::FLOW, plus: p, minus: piperine_abi::raw::GROUND, abstol: DEFAULT, max: DEFAULT });
    let pin = Cell { kind: cell::PIN, index: p, part: part::VALUE };
    let freq = Cell { kind: cell::AMBIENT, index: ambient::FREQ, part: part::VALUE };
    s.function(FunctionSpec {
        name: "i",
        origin: "pz_device_fault.rs",
        reads: vec![pin, freq],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for AcFault {
    fn prototype() -> &'static Shape<'static> {
        SHAPE.get_or_init(shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((AcFault, Arc::clone(SHAPE.get_or_init(shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if cx.f64(1) != 0.0 {
            return Err(Fault::new(9, "no small-signal model"));
        }
        cx.set(0, 1.0e-3 * cx.f64(0));
        cx.jac().copy_from_slice(&[1.0e-3]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn a_device_fault_in_the_pole_zero_assembly_is_an_error_naming_the_instance() {
    let mut b = CircuitBuilder::new("fault");
    let gnd = b.ground();
    let n = b.node("n");
    // A source-driven node so the analysis reaches the AC probes.
    b.device(
        piperine_abi_test_vsrc(),
        "v1",
        &[],
        &[Port::Node(n.clone()), Port::Node(gnd)],
    );
    b.device(module_fault(), "bad1", &[], &[Port::Node(n)]);
    let mut circuit = b.build().expect("build");
    let options = PoleZeroOptions {
        input_source: BranchIdentifier::new("v1", "f"),
        output: AnalogVariable::Node(NodeIdentifier::Anonymous(0)),
        output_ref: None,
    };
    let err = circuit.pz(options, Context::default()).err().expect("the fault surfaces");
    assert!(err.to_string().contains("bad1"), "{err}");
}

fn module_fault() -> &'static Module {
    MODULE.get_or_init(|| module::<AcFault>("AcFault"))
}

fn piperine_abi_test_vsrc() -> &'static Module {
    abi_vsrc::vsrc()
}

#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_vsrc;

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    // FREQ (read 1) at 0: the operating point, where the fixture has a model.
    partials::verify_every_function_with("AcFault", MODULE.get_or_init(|| module::<AcFault>("AcFault")), &[(0, 1, 0.0)]);
}
