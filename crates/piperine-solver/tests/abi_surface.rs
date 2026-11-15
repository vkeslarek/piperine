//! `piperine_solver::abi` is enough on its own: a device implements the
//! `Device` contract — a shape, evaluation with its partials, reactions —
//! importing nothing but `abi` (MD-17's two-tier surface), and it builds into
//! a circuit and solves.

use std::result::Result;
use std::sync::{Arc, OnceLock};

use piperine_solver::abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_solver::abi::layout::F64;
use piperine_solver::abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, pin_flag, role};
use piperine_solver::abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_solver::abi::thunk::module;
use piperine_solver::abi::{CircuitBuilder, Context, ParamValue, Port};

/// A resistor `I(p, n) = V(p, n) / r`.
/// pins: r=0 (PARAM F64, default 1 kΩ); p=1, n=2 (PORT); term T0 FLOW{p, n}
/// fn 0: reads [p, n, r] writes [T0] partials (0<-0) (0<-1) (0<-2)
struct Resistor;
static SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static MODULE: OnceLock<Module> = OnceLock::new();

fn cell_of(kind: u32, index: u32) -> Cell {
    Cell { kind, index, part: part::VALUE }
}

fn shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &1000.0_f64.to_le_bytes(), ..PinSpec::discrete("r", role::PARAM, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: "abi_surface.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        origin: "abi_surface.rs",
        reads: vec![cell_of(cell::PIN, p), cell_of(cell::PIN, n), cell_of(cell::PIN, 0)],
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

impl Device for Resistor {
    fn prototype() -> &'static Shape<'static> {
        SHAPE.get_or_init(shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resistor, Arc::clone(SHAPE.get_or_init(shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, r) = (cx.f64(0), cx.f64(1), cx.f64(2));
        let g = 1.0 / r;
        cx.set(0, g * vp - g * vn);
        cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r)]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn abi_compiles_a_device() {
    let mut b = CircuitBuilder::new("test");
    let gnd = b.ground();
    let n1 = b.node("n1");
    // A 1 kΩ resistor to ground alone: nothing drives the node.
    b.device(
        MODULE.get_or_init(|| module::<Resistor>("Resistor")),
        "r1",
        &[("r", ParamValue::Real(1000.0))],
        &[Port::Node(n1.clone()), Port::Node(gnd)],
    );
    let mut circuit = b.build().expect("build");
    let res = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert_eq!(res.get(n1.variable().clone()), Some(0.0));
}
