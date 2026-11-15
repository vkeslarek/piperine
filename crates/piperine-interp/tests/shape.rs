//! The shape of a body (feature `ir-interp`, INT-09, INT-01 AC 5, INT-05 AC 1): pins in the
//! declared order with their defaults, tolerances and holds; the terms of flow and potential
//! branches; one input per site; a value of the wrong layout refused naming the param; the two
//! edge-case shapes (an `analog` block that contributes nothing, a held node with no `analog`).

use std::sync::Arc;

use piperine_abi::caller::Handle;
use piperine_abi::raw::{global, input, kind, op, pin_flag, role};
use piperine_interp::Interpreter;
use piperine_ir::{
    BodyNode, BodySpec, InstanceSpec, IrBuilder, NodeRef, NodeRole, NodeSpec, Origin, Real, Stmt,
};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/circuits.rs"]
mod circuits;
#[allow(dead_code)]
#[path = "support/models.rs"]
mod models;

use circuits::Circuits;
use devices::{Devices, Ex};
use models::{Models, View};

/// Hand-built bodies the five devices do not cover.
struct Odd;

impl Odd {
    fn at(line: u32) -> Origin {
        Origin { file: "odd.msl".into(), line, col: 3 }
    }

    /// A top body `Quiet` with one held signal `s = 0.25` and an `analog` block holding one `let`
    /// and no contribution.
    fn quiet() -> Models {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "odd.msl", 4, 9);
        let one = e.real(1.0);
        let let_x = b.stmt(Stmt::Let { local: 0, init: one });
        let body = b.body(BodySpec {
            name: "Quiet".into(),
            origin: Self::at(1),
            params: vec![],
            nodes: vec![BodyNode { name: "s".into(), nature: el, role: NodeRole::Signal, init: Some(Real::new(0.25)), origin: Self::at(2) }],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![piperine_ir::Local { name: "x".into(), ty: piperine_ir::ValueType::F64 }],
            analog: Some(vec![let_x]),
            children: vec![],
        });
        let top = b.next_instance_id();
        let s = b.node(NodeSpec { path: "Quiet.s".into(), nature: el, declared_by: top, init: Some(Real::new(0.25)), origin: Self::at(2) });
        let top = b.instance(InstanceSpec {
            path: "Quiet".into(),
            name: "Quiet".into(),
            body,
            parent: None,
            nodes: vec![NodeRef::Node(s)],
            params: vec![],
            origin: Self::at(1),
        });
        Models::of(b.finish(top))
    }
}

#[test]
fn a_held_node_gives_has_init() {
    // `RcHeld.out` is held at 0 V; `Quiet.s` at 0.25 V.
    let held = Interpreter::new(Arc::new(Circuits::rc_held())).expect("checked");
    let top = held.ir().top().body().id();
    let shape = held.module(top).expect("a body with a held node has a module").prototype();
    let view = View { shape };
    assert_eq!(view.pin_names(), ["vin", "out"]);
    let (vin, out) = (shape.pins.get(0).unwrap(), shape.pins.get(1).unwrap());
    assert_eq!(vin.flags & pin_flag::HAS_INIT, 0, "vin has no initial potential");
    assert!(vin.init.is_empty());
    assert_eq!(out.flags & pin_flag::HAS_INIT, pin_flag::HAS_INIT);
    assert_eq!(out.init.as_slice(), 0.0f64.to_le_bytes());

    let quiet = Odd::quiet();
    let s = quiet.module("Quiet").prototype().pins.get(0).copied().unwrap();
    assert_eq!(s.flags & pin_flag::HAS_INIT, pin_flag::HAS_INIT);
    assert_eq!(s.init.as_slice(), 0.25f64.to_le_bytes());
}

#[test]
fn pins_come_in_the_declared_order() {
    let m = Models::all();
    let names = |body: &str| View { shape: m.module(body).prototype() }.pin_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(names("Resistor"), ["r", "p", "n"]);
    assert_eq!(names("Capacitor"), ["c", "p", "n"]);
    assert_eq!(names("Vsource"), ["dc", "ac", "p", "n", "b0.flow"]);
    assert_eq!(names("Isource"), ["dc", "ac", "p", "n"]);
    assert_eq!(names("Diode"), ["is", "n", "cj0", "a", "k", "temperature"]);

    let diode = m.module("Diode").prototype();
    let pin = |i: usize| diode.pins.get(i).copied().unwrap();
    // params: discrete PARAM F64 with their defaults
    for (i, default) in [(0, 1e-14), (1, 1.0), (2, 0.0)] {
        let p = pin(i);
        assert_eq!((p.kind, p.role, p.layout.size), (kind::DISCRETE, role::PARAM, 8));
        assert_eq!(p.flags & pin_flag::HAS_INIT, pin_flag::HAS_INIT);
        assert_eq!(p.init.as_slice(), f64::to_le_bytes(default));
    }
    // ports: continuous with the Electrical tolerances
    for i in [3, 4] {
        let p = pin(i);
        assert_eq!((p.kind, p.role), (kind::CONTINUOUS, role::PORT));
        assert_eq!(p.abstol, [1e-6, 1e-12]);
        assert_eq!(p.flags & pin_flag::HAS_INIT, 0);
    }
    let t = pin(5);
    assert_eq!((t.kind, t.role, t.global, t.layout.size), (kind::DISCRETE, role::GLOBAL, global::TEMPERATURE, 8));

    // a param with no default has no init
    let r = m.module("Resistor").prototype().pins.get(0).copied().unwrap();
    assert_eq!((r.kind, r.role, r.flags & pin_flag::HAS_INIT), (kind::DISCRETE, role::PARAM, 0));
    assert!(r.init.is_empty());

    // the private flow pin of a potential branch
    let f = m.module("Vsource").prototype().pins.get(4).copied().unwrap();
    assert_eq!((f.kind, f.role), (kind::CONTINUOUS, role::PRIVATE));
}

#[test]
fn a_flow_branch_declares_flow_and_ddt_terms() {
    let m = Models::all();
    let terms = |body: &str| View { shape: m.module(body).prototype() }.terms();
    let t = |name: &str, o: u32, p: &str, n: &str| (name.to_string(), o, p.to_string(), n.to_string());
    assert_eq!(terms("Resistor"), [t("b0.flow", op::FLOW, "p", "n")]);
    assert_eq!(terms("Capacitor"), [t("b0.charge", op::DDT, "p", "n")]);
    assert_eq!(terms("Isource"), [t("b0.flow", op::FLOW, "p", "n")]);
    assert_eq!(terms("Diode"), [t("b0.flow", op::FLOW, "a", "k"), t("b0.charge", op::DDT, "a", "k")]);
}

#[test]
fn a_potential_branch_declares_its_pin_and_two_terms() {
    let m = Models::all();
    let view = View { shape: m.module("Vsource").prototype() };
    let t = |name: &str, p: &str, n: &str| (name.to_string(), op::FLOW, p.to_string(), n.to_string());
    assert_eq!(view.terms(), [t("b0.flow", "p", "n"), t("b0.eq", "b0.flow", "GROUND")]);
}

#[test]
fn sites_declare_limit_and_stim_inputs() {
    let m = Models::all();
    let inputs = |body: &str| View { shape: m.module(body).prototype() }.inputs();
    assert_eq!(inputs("Diode"), [("vd".to_string(), input::LIMIT)]);
    assert_eq!(inputs("Vsource"), [("ac".to_string(), input::STIM)]);
    assert_eq!(inputs("Isource"), [("ac".to_string(), input::STIM)]);
    assert_eq!(inputs("Resistor"), []);
    assert_eq!(inputs("Capacitor"), []);
}

#[test]
fn a_value_of_the_wrong_layout_faults_naming_the_param() {
    let m = Models::all();
    let err = Handle::create(m.module("Resistor"), &[(0, &[0u8; 4][..])]).expect_err("4 bytes for an F64 param");
    assert!(err.message.contains("(r)"), "the fault names the param: {}", err.message);
    let err = Handle::create(m.module("Diode"), &[(2, &[0u8; 4][..])]).expect_err("4 bytes for an F64 param");
    assert!(err.message.contains("(cj0)"), "the fault names the param: {}", err.message);
    // the right layout binds
    assert!(Handle::create(m.module("Resistor"), &[(0, &1e3f64.to_le_bytes()[..])]).is_ok());
}

#[test]
fn a_block_without_contributions_gives_no_term() {
    let quiet = Odd::quiet();
    let module = quiet.module("Quiet");
    let shape = module.prototype();
    assert_eq!((shape.terms.len(), shape.functions.len(), shape.inputs.len()), (0, 0, 0));
    assert!(Handle::create(module, &[]).is_ok(), "the instance binds");
}

#[test]
fn a_held_node_without_analog_is_ports_only() {
    let held = Interpreter::new(Arc::new(Circuits::rc_held())).expect("checked");
    let top = held.ir().top().body();
    assert!(top.analog().is_none());
    let shape = held.module(top.id()).expect("a held node gives a module").prototype();
    assert_eq!(shape.pins.len(), 2);
    assert!(shape.pins.iter().all(|p| p.kind == kind::CONTINUOUS && p.role == role::PORT));
    assert_eq!((shape.terms.len(), shape.inputs.len(), shape.functions.len(), shape.reactions.len()), (0, 0, 0, 0));
    // a top with no held node and no analog block has no module
    let divider = Interpreter::new(Arc::new(Circuits::divider())).expect("checked");
    assert!(divider.module(divider.ir().top().body().id()).is_none());
}
