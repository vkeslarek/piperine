//! Sensitivities through declared partials (SAI-10): the adjoint driver on
//! devices takes `∂F/∂param` from the partials a device declares against its
//! `PARAM` pin (chained through discrete functions), differences by
//! re-evaluation where no declared path exists, and fails loud on a
//! `STRUCTURAL` param.

use std::sync::{Arc, OnceLock};

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{add_resistor, add_vsrc, resistor_opt};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_digital_solver::abi::engine::{Horizon, Mode};
use piperine_digital_solver::abi::tick::Tick;
use piperine_solver::abi::{AnalogReference, ParamValue, Port};
use piperine_solver::prelude::{AdjointOptions, CircuitBuilder, CircuitInstance, Context, Net};

fn cell_of(kind: u32, index: u32) -> Cell {
    Cell { kind, index, part: part::VALUE }
}

fn pin(index: u32) -> Cell {
    cell_of(cell::PIN, index)
}

// ---------------------------------------------------------------------------
// Gd: a conductance derived from a param by a discrete function
// ---------------------------------------------------------------------------

/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT); g=3 (PRIVATE discrete F64)
/// terms: T0 FLOW{p, n}
/// fn 0 "g" (discrete): reads [r] writes [g] partials (0<-0) = -1/r^2
/// fn 1 "i" (analog): reads [p, n, g] writes [T0] partials (0<-p) = g, (0<-n) = -g, (0<-g) = vp - vn
struct Gd;
static GD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static GD_MODULE: OnceLock<Module> = OnceLock::new();
const R_DEFAULT: [u8; 8] = 1.0e3_f64.to_le_bytes();

fn gd_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &R_DEFAULT, ..PinSpec::discrete("r", role::PARAM, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let g = s.pin(PinSpec::discrete("g", role::PRIVATE, &F64));
    let t0 = s.term(TermSpec { name: "i", origin: "abi_sens.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "g",
        origin: "abi_sens.rs",
        reads: vec![pin(0)],
        writes: vec![pin(g)],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
        ..FunctionSpec::default()
    });
    s.function(FunctionSpec {
        name: "i",
        origin: "abi_sens.rs",
        reads: vec![pin(p), pin(n), pin(g)],
        writes: vec![cell_of(cell::TERM, t0)],
        partials: vec![
            Partial { write: 0, to: 0, read: 0, at: 0 },
            Partial { write: 0, to: 0, read: 1, at: 0 },
            Partial { write: 0, to: 0, read: 2, at: 0 },
        ],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Gd {
    fn prototype() -> &'static Shape<'static> {
        GD_SHAPE.get_or_init(gd_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Gd, Arc::clone(GD_SHAPE.get_or_init(gd_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        if k == 0 {
            let r = cx.f64(0);
            cx.set(0, 1.0 / r);
            cx.jac().copy_from_slice(&[-1.0 / (r * r)]);
        } else {
            let (vp, vn, g) = (cx.f64(0), cx.f64(1), cx.f64(2));
            cx.set(0, g * vp - g * vn);
            cx.jac().copy_from_slice(&[g, -g, vp - vn]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn gd() -> &'static Module {
    GD_MODULE.get_or_init(|| module::<Gd>("Gd"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn net_of(circuit: &CircuitInstance, node: &AnalogReference) -> Net {
    let var = node.variable().clone();
    circuit
        .nets()
        .into_iter()
        .find(|n| n.analog_variable().map(|v| **v == *var).unwrap_or(false))
        .expect("net is solved")
}

/// The divider `Top` of the sensitivity targets: 10 V over two 1 kΩ resistors.
fn divider() -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("Top");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 10.0, &top, &gnd);
    add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
    add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    (b.build().expect("build"), mid)
}

fn params(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn sensitivity_uses_declared_partials_and_falls_back_to_a_finite_difference() {
    let wrt = params(&[("r1", "r"), ("r2", "r"), ("v1", "dc")]);
    let run = |difference: bool| {
        let (mut circuit, mid) = divider();
        let output = net_of(&circuit, &mid);
        let label = output.label().to_string();
        let mut options = AdjointOptions::new(output, wrt.clone());
        options.finite_difference_only = difference;
        (label, circuit.adjoint(options, Context::default()).expect("adjoint").solve().expect("solve"))
    };
    let (out, declared) = run(false);
    let (_, differenced) = run(true);

    assert!(declared.fallback_users.is_empty(), "the declared partials cover every param");
    assert_eq!(differenced.fallback_users.len(), 3, "every param is differenced: {:?}", differenced.fallback_users);
    // v(mid) = 10 r2 / (r1 + r2): -2.5e-3 and +2.5e-3 per ohm, 0.5 per volt.
    let analytic = [("r1", "r", -2.5e-3), ("r2", "r", 2.5e-3), ("v1", "dc", 0.5)];
    for (label, param, want) in analytic {
        let a = declared.get(&out, label, param).expect("declared");
        let b = differenced.get(&out, label, param).expect("differenced");
        assert!((a - want).abs() <= 1.0e-9 * want.abs(), "declared d/d{label}.{param} = {a:e} vs {want:e}");
        assert!((a - b).abs() <= 1.0e-6 * a.abs(), "{label}.{param}: declared {a:e} vs difference {b:e}");
    }
}

#[test]
fn a_structural_param_sensitivity_fails_loud() {
    let mut b = CircuitBuilder::new("opt");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 10.0, &top, &gnd);
    b.device(
        resistor_opt(),
        "r1",
        &[("w", ParamValue::Real(2.0e-3))],
        &[Port::Node(top), Port::Node(mid.clone())],
    );
    add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    let mut circuit = b.build().expect("build");
    let options = AdjointOptions::new(net_of(&circuit, &mid), params(&[("r1", "w")]));
    let err = circuit.adjoint(options, Context::default()).expect("adjoint").solve().err().expect("refused");
    assert!(err.to_string().contains("STRUCTURAL"), "{err}");
}

#[test]
fn a_derivative_chains_through_a_discrete_function() {
    let mut b = CircuitBuilder::new("chain");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    add_vsrc(&mut b, "v1", 10.0, &top, &gnd);
    b.device(gd(), "x1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(top), Port::Node(mid.clone())]);
    add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
    let mut circuit = b.build().expect("build");

    // Run the instant at t = 0 by hand (the protocol drives it in a transient) and
    // hand the analog side the derived conductance.
    {
        let bound = circuit.bound_mut().expect("bound");
        bound.digital.start().expect("start");
        let outcome = bound
            .digital
            .run_instant(Tick(0), &[], Horizon::uniform(Tick(0)), Mode { is_static: true, settle: false })
            .expect("instant");
        for (net, bytes) in outcome.boundary {
            let slot = bound.names.slots[&net];
            bound.analog.set_discrete(slot, &bytes);
        }
    }
    let output = net_of(&circuit, &mid);
    let out = output.label().to_string();
    let options = AdjointOptions::new(output, params(&[("x1", "r")]));
    let result = circuit.adjoint(options, Context::default()).expect("adjoint").solve().expect("solve");

    // v(mid) = 10 r2 / (r + r2), so dv/dr = -10 r2 / (r + r2)^2 = -2.5e-3.
    let d = result.get(&out, "x1", "r").expect("derivative");
    assert!((d + 2.5e-3).abs() <= 1.0e-9 * 2.5e-3, "dv/dr = {d:e}");
    assert!(result.fallback_users.is_empty());
}
