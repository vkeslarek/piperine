//! Lifecycle contract test (ABI-16) on the `Device` trait: an instance is
//! created once per placement when the circuit is bound and destroyed (the
//! device dropped) once when the circuit is dropped; a failing `create`
//! reaches the host at build.
//!
//! The hook-ordering charts of Part VII §19 over the removed per-hook contract
//! (`setup`, `set_temperature`, `update`, `load_*`, `limiting_report`,
//! `accept_timestep`, `set_param`) are gone with them; the order the solver
//! evaluates a device in is the protocol's, proven by `protocol_*.rs`.

#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::result::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_solver::abi::Port;
use piperine_solver::prelude::{CircuitBuilder, Context};

fn shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec::continuous("p", role::PORT));
    s.freeze()
}

static SHAPE: OnceLock<ShapeRef> = OnceLock::new();

fn shared() -> &'static ShapeRef {
    SHAPE.get_or_init(shape)
}

static CREATES: AtomicUsize = AtomicUsize::new(0);
static DESTROYS: AtomicUsize = AtomicUsize::new(0);
static COUNTED: OnceLock<Module> = OnceLock::new();

/// Counts its creations and its destruction.
struct Counted;

impl Drop for Counted {
    fn drop(&mut self) {
        DESTROYS.fetch_add(1, Ordering::SeqCst);
    }
}

impl Device for Counted {
    fn prototype() -> &'static Shape<'static> {
        shared().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        CREATES.fetch_add(1, Ordering::SeqCst);
        Ok((Counted, Arc::clone(shared())))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn test_lifecycle_hooks_called() {
    let mut b = CircuitBuilder::new("test_circuit");
    let gnd = b.ground();
    b.device(COUNTED.get_or_init(|| module::<Counted>("Counted")), "dev", &[], &[Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");
    assert_eq!(CREATES.load(Ordering::SeqCst), 1, "created once, at bind");

    let context = Context::default();
    let mut dc = circuit.dc(context.clone()).unwrap();
    let _ = dc.solve().unwrap();
    let _ac = circuit.ac(context.clone()).unwrap();
    assert_eq!(CREATES.load(Ordering::SeqCst), 1, "an analysis does not create it again");
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 0);

    drop(circuit);
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 1);
}

static FAIL_CREATES: AtomicUsize = AtomicUsize::new(0);
static FAILING: OnceLock<Module> = OnceLock::new();

/// Fails at creation.
struct Failing;

impl Device for Failing {
    fn prototype() -> &'static Shape<'static> {
        shared().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        FAIL_CREATES.fetch_add(1, Ordering::SeqCst);
        Err(Fault::new(1, "test setup error"))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn test_setup_error_propagates() {
    let mut b = CircuitBuilder::new("test_circuit");
    let gnd = b.ground();
    b.device(FAILING.get_or_init(|| module::<Failing>("Failing")), "dev", &[], &[Port::Node(gnd)]);
    let result = b.build();
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("test setup error"));
    assert_eq!(FAIL_CREATES.load(Ordering::SeqCst), 1);
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Counted", COUNTED.get_or_init(|| module::<Counted>("Counted")));
    partials::verify_every_function("Failing", FAILING.get_or_init(|| module::<Failing>("Failing")));
}
