//! Temperature protocol contract tests (ABI-19/20) on the `Device` trait: the
//! `TEMPERATURE` global holds the run's temperature before the first
//! evaluation, a host running a temperature sweep re-seeds it through
//! `CircuitInstance::set_temperature`, and a device with no `TEMPERATURE` pin
//! is untouched by either.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use std::sync::{Arc, OnceLock};

use abi_devices::{add_resistor, add_vsrc, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, cell, global, op, part, role};
use piperine_abi::layout::F64;
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{AnalogReference, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, Solver, Tolerances};

const ORIGIN: &str = "temperature.rs";

/// A current of `T · 1 µA` drawn out of `p` into `n`, `T` the `TEMPERATURE`
/// global.
/// pins: t=0 (GLOBAL TEMPERATURE F64); p=1, n=2 (PORT)
/// terms: T0 FLOW{p, n}; fn 0: reads [t] writes [T0], no partial (constant in the unknowns)
struct Thermo;
static THERMO_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static THERMO: OnceLock<Module> = OnceLock::new();

fn thermo_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("t", role::GLOBAL, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![Cell { kind: cell::PIN, index: 0, part: part::VALUE }],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Thermo {
    fn prototype() -> &'static Shape<'static> {
        THERMO_SHAPE.get_or_init(thermo_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Thermo, Arc::clone(THERMO_SHAPE.get_or_init(thermo_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let t = cx.f64(0);
        cx.set(0, -t * 1.0e-6);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// 1 kΩ from `out` to ground, a `Thermo` pulling `T` µA out of `out`:
/// `v(out) = T` millivolts. Also a source and a resistor the device does not touch.
fn circuit() -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("temp");
    let gnd = b.ground();
    let out = b.node("out");
    let src = b.node("src");
    add_resistor(&mut b, "r1", 1.0e3, &out, &gnd);
    b.device(THERMO.get_or_init(|| module::<Thermo>("Thermo")), "t1", &[], &[Port::Node(out.clone()), Port::Node(gnd.clone())]);
    add_vsrc(&mut b, "v1", 1.0, &src, &gnd);
    add_resistor(&mut b, "r2", 1.0e3, &src, &gnd);
    (b.build().expect("build"), out)
}

fn v_out(circuit: &mut CircuitInstance, out: &AnalogReference) -> f64 {
    circuit.dc(Context::default()).unwrap().solve().unwrap().get_node(&node_id(out)).unwrap()
}

/// ABI-19: when the solver enters an analysis, the `TEMPERATURE` global
/// holds the run's temperature before the first evaluation: the very first
/// operating point already reads it.
#[test]
fn set_temperature_called_before_first_load() {
    let (circuit, out) = circuit();
    let run_temperature = 325.0;
    let ctx = Context {
        tolerances: Tolerances { temperature: run_temperature, ..Tolerances::default() },
        ..Context::default()
    };
    let mut solver = Solver::new(circuit).with_context(ctx).build();
    let result = solver.dc().unwrap().solve().unwrap();
    let v = result.get_node(&node_id(&out)).unwrap();
    assert!(
        (v - run_temperature * 1.0e-3).abs() < 1e-9,
        "the first solve read the run temperature {run_temperature}: v(out) = {v}"
    );
}

/// ABI-20: a temperature sweep point change re-seeds the global; the next
/// solve uses the new temperature.
#[test]
fn temperature_sweep_reseeds_every_element() {
    let (mut circuit, out) = circuit();
    let nominal = Tolerances::default().temperature;
    assert!((v_out(&mut circuit, &out) - nominal * 1.0e-3).abs() < 1e-9);

    let sweep_t = 380.0;
    circuit.set_temperature(sweep_t);
    let v = v_out(&mut circuit, &out);
    assert!((v - sweep_t * 1.0e-3).abs() < 1e-9, "sweep temperature {sweep_t} reached the device: v(out) = {v}");

    // Back to the seeded temperature: nothing stays written.
    circuit.set_temperature(nominal);
    assert!((v_out(&mut circuit, &out) - nominal * 1.0e-3).abs() < 1e-9);
}

/// ABI-19 edge case: a device with no `TEMPERATURE` pin is unaffected.
#[test]
fn default_set_temperature_is_noop() {
    let mut b = CircuitBuilder::new("default-noop");
    let gnd = b.ground();
    let src = b.node("src");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 1.0, &src, &gnd);
    add_resistor(&mut b, "r1", 1.0e3, &src, &mid);
    add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    let mut circuit = b.build().expect("build");
    let before = v_out(&mut circuit, &mid);
    circuit.set_temperature(350.0);
    assert_eq!(v_out(&mut circuit, &mid), before);
    assert!((before - 0.5).abs() < 1e-12);
}
