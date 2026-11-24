//! The shape of a body (feature `ir-interp`, INT-09, INT-01 AC 5, INT-05 AC 1): pins in the
//! declared order with their defaults, tolerances and holds; the terms of flow and potential
//! branches; one input per site; a value of the wrong layout refused naming the param; the two
//! edge-case shapes (an `analog` block that contributes nothing, a held node with no `analog`);
//! one function per site and one for the terms, each reading exactly its dependency set with
//! partials against its differentiable reads, every entry with its origin, every shape valid
//! and no function writing both a limit and a term (INT-10 AC 5). A param with a `where` has a
//! private `BIT` check pin, a function that writes it and the `HAS_CHECK` flag (INT-14 AC 1).

use std::sync::Arc;

use piperine_abi::caller::Handle;
use piperine_abi::raw::{Cell, Partial, Shape, ambient, cell, global, input, kind, op, part, pin_flag, role};
use piperine_abi::validate::validate;
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
            locals: vec![piperine_ir::Local { name: "x".into(), ty: piperine_ir::ValueType::F64, mutable: false }],
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
    assert_eq!(names("Resistor"), ["r", "p", "n", "check:r"]);
    assert_eq!(names("Capacitor"), ["c", "p", "n", "check:c"]);
    assert_eq!(names("Vsource"), ["dc", "ac", "p", "n", "b0.flow"]);
    assert_eq!(names("Isource"), ["dc", "ac", "p", "n"]);
    assert_eq!(names("Diode"), ["is", "n", "cj0", "a", "k", "temperature", "check:is", "check:n", "check:cj0"]);

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

/// Cells and partials written short.
struct Cells;

impl Cells {
    fn pin(i: u32) -> Cell {
        Cell { kind: cell::PIN, index: i, part: part::VALUE }
    }

    fn input(i: u32, p: u32) -> Cell {
        Cell { kind: cell::INPUT, index: i, part: p }
    }

    fn term(i: u32) -> Cell {
        Cell { kind: cell::TERM, index: i, part: part::VALUE }
    }

    fn ambient(a: u32) -> Cell {
        Cell { kind: cell::AMBIENT, index: a, part: part::VALUE }
    }

    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// Every shape these tests plan: the five devices and the two edge-case shapes.
    fn every_shape(m: &Models, quiet: &Models, held: &Interpreter) -> Vec<(String, &'static Shape<'static>)> {
        let mut shapes: Vec<(String, &'static Shape<'static>)> = ["Resistor", "Capacitor", "Vsource", "Isource", "Diode"]
            .iter()
            .map(|n| (n.to_string(), m.module(n).prototype()))
            .collect();
        shapes.push(("Quiet".into(), quiet.module("Quiet").prototype()));
        let top = held.ir().top().body().id();
        shapes.push(("RcHeld".into(), held.module(top).expect("held top").prototype()));
        shapes
    }
}

#[test]
fn one_function_per_site_and_one_for_terms() {
    let m = Models::all();
    let names = |body: &str| View { shape: m.module(body).prototype() }.function_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(names("Diode"), ["limit:vd", "terms", "check:is", "check:n", "check:cj0"]);
    assert_eq!(names("Vsource"), ["stim:0", "terms"]);
    assert_eq!(names("Isource"), ["stim:0", "terms"]);
    assert_eq!(names("Resistor"), ["terms", "check:r"]);
    assert_eq!(names("Capacitor"), ["terms", "check:c"]);
    // what each writes
    let diode = View { shape: m.module("Diode").prototype() };
    assert_eq!(diode.function("limit:vd").1, [Cells::input(0, part::PROPOSED), Cells::input(0, part::LIMITED)]);
    assert_eq!(diode.function("terms").1, [Cells::term(0), Cells::term(1)]);
    let vsource = View { shape: m.module("Vsource").prototype() };
    assert_eq!(vsource.function("stim:0").1, [Cells::input(0, part::RE), Cells::input(0, part::IM)]);
    assert_eq!(vsource.function("terms").1, [Cells::term(0), Cells::term(1)]);
}

#[test]
fn reads_and_partials_are_the_dependency_sets() {
    let m = Models::all();
    // Diode pins: is=0 n=1 cj0=2 a=3 k=4 temperature=5; input vd=0.
    let diode = View { shape: m.module("Diode").prototype() };
    let (reads, _, partials) = diode.function("limit:vd");
    assert_eq!(
        reads,
        [
            Cells::pin(3),
            Cells::pin(4),
            Cells::pin(0),
            Cells::pin(1),
            Cells::pin(5),
            Cells::input(0, part::LAST),
            Cells::input(0, part::AGE),
            Cells::ambient(ambient::STATIC),
        ]
    );
    assert_eq!(partials, [Cells::partial(0, 0), Cells::partial(0, 1)], "PROPOSED against a and k");
    let (reads, _, partials) = diode.function("terms");
    assert_eq!(
        reads,
        [
            Cells::input(0, part::VALUE),
            Cells::pin(0),
            Cells::pin(1),
            Cells::pin(2),
            Cells::pin(5),
            Cells::ambient(ambient::GMIN),
        ]
    );
    assert_eq!(partials, [Cells::partial(0, 0), Cells::partial(1, 0)], "flow and charge against vd");

    // Resistor pins: r=0 p=1 n=2. No partial against the param.
    let (reads, _, partials) = View { shape: m.module("Resistor").prototype() }.function("terms");
    assert_eq!(reads, [Cells::pin(1), Cells::pin(2), Cells::pin(0)]);
    assert_eq!(partials, [Cells::partial(0, 0), Cells::partial(0, 1)]);

    // Capacitor pins: c=0 p=1 n=2.
    let (reads, _, partials) = View { shape: m.module("Capacitor").prototype() }.function("terms");
    assert_eq!(reads, [Cells::pin(1), Cells::pin(2), Cells::pin(0)]);
    assert_eq!(partials, [Cells::partial(0, 0), Cells::partial(0, 1)]);

    // Vsource pins: dc=0 ac=1 p=2 n=3 b0.flow=4; input ac=0. The stimulus reads its magnitude.
    let vsource = View { shape: m.module("Vsource").prototype() };
    let (reads, _, partials) = vsource.function("stim:0");
    assert_eq!(reads, [Cells::pin(1)]);
    assert!(partials.is_empty());
    let (reads, _, partials) = vsource.function("terms");
    assert_eq!(
        reads,
        [
            Cells::pin(2),
            Cells::pin(3),
            Cells::pin(4),
            Cells::input(0, part::VALUE),
            Cells::pin(0),
            Cells::pin(1),
            Cells::ambient(ambient::SOURCE_SCALE),
        ]
    );
    // b0.flow against the private pin; b0.eq against p, n and the stimulus
    assert_eq!(partials, [Cells::partial(0, 2), Cells::partial(1, 0), Cells::partial(1, 1), Cells::partial(1, 3)]);

    // Isource pins: dc=0 ac=1 p=2 n=3: its flow term reads no pin.
    let (reads, _, partials) = View { shape: m.module("Isource").prototype() }.function("terms");
    assert_eq!(reads, [Cells::input(0, part::VALUE), Cells::pin(0), Cells::pin(1), Cells::ambient(ambient::SOURCE_SCALE)]);
    assert_eq!(partials, [Cells::partial(0, 0)]);
}

#[test]
fn every_entry_carries_its_origin() {
    let m = Models::all();
    let diode = m.module("Diode").prototype();
    let pins: Vec<&str> = diode.pins.iter().map(|p| p.origin.as_str()).collect();
    assert_eq!(pins, ["devices.msl:32:5", "devices.msl:33:5", "devices.msl:34:5", "devices.msl:31:18", "devices.msl:31:33", "devices.msl:31:1", "devices.msl:32:5", "devices.msl:33:5", "devices.msl:34:5"]);
    let terms: Vec<&str> = diode.terms.iter().map(|t| t.origin.as_str()).collect();
    assert_eq!(terms, ["devices.msl:41:17", "devices.msl:41:17"], "the contribution that writes the branch");
    assert_eq!(diode.inputs.get(0).unwrap().origin.as_str(), "devices.msl:39:17", "the site");
    let functions: Vec<&str> = diode.functions.iter().map(|f| f.origin.as_str()).collect();
    assert_eq!(
        functions,
        ["devices.msl:39:17", "devices.msl:31:1", "devices.msl:32:5", "devices.msl:33:5", "devices.msl:34:5"],
        "the site, the body, then each param with a where"
    );

    let vsource = m.module("Vsource").prototype();
    assert_eq!(vsource.pins.get(4).unwrap().origin.as_str(), "devices.msl:21:30", "the private pin: its contribution");
    assert_eq!(vsource.inputs.get(0).unwrap().origin.as_str(), "devices.msl:21:62");
    assert_eq!(vsource.functions.get(0).unwrap().origin.as_str(), "devices.msl:21:62");

    // nothing is left without an origin
    let (quiet, held) = (Odd::quiet(), Interpreter::new(Arc::new(Circuits::rc_held())).expect("checked"));
    for (name, shape) in Cells::every_shape(&m, &quiet, &held) {
        let origins = shape
            .pins
            .iter()
            .map(|p| p.origin)
            .chain(shape.terms.iter().map(|t| t.origin))
            .chain(shape.inputs.iter().map(|x| x.origin))
            .chain(shape.functions.iter().map(|f| f.origin));
        for o in origins {
            assert!(!o.as_str().is_empty(), "{name}: an entry without an origin");
        }
    }
}

#[test]
fn every_m1_shape_validates() {
    let (m, quiet) = (Models::all(), Odd::quiet());
    let held = Interpreter::new(Arc::new(Circuits::rc_held())).expect("checked");
    let shapes = Cells::every_shape(&m, &quiet, &held);
    assert_eq!(shapes.len(), 7);
    for (name, shape) in shapes {
        assert_eq!(validate(shape), Ok(()), "{name}");
    }
}

#[test]
fn no_function_writes_a_limit_and_a_term() {
    let (m, quiet) = (Models::all(), Odd::quiet());
    let held = Interpreter::new(Arc::new(Circuits::rc_held())).expect("checked");
    let mut limit_writers = 0;
    for (name, shape) in Cells::every_shape(&m, &quiet, &held) {
        for f in shape.functions.iter() {
            let limit = f.writes.iter().any(|c| c.kind == cell::INPUT && matches!(c.part, part::PROPOSED | part::LIMITED));
            let term = f.writes.iter().any(|c| c.kind == cell::TERM);
            assert!(!(limit && term), "{name}: function {} writes a limit and a term", f.name.as_str());
            limit_writers += usize::from(limit);
        }
    }
    assert_eq!(limit_writers, 1, "the diode's limit function is among those checked");
}

#[test]
fn a_where_declares_a_check_pin_and_function() {
    let m = Models::all();
    let resistor = m.module("Resistor").prototype();
    let view = View { shape: resistor };
    assert_eq!(view.pin_names(), ["r", "p", "n", "check:r"]);
    let r = resistor.pins.get(0).copied().unwrap();
    assert_eq!(r.flags & pin_flag::HAS_CHECK, pin_flag::HAS_CHECK);
    assert_eq!(r.check, 3, "the param's check is the check pin");
    let check = resistor.pins.get(3).copied().unwrap();
    assert_eq!((check.kind, check.role, check.layout.size), (kind::DISCRETE, role::PRIVATE, 1), "a private BIT");
    // `check:r` reads the param alone and writes the check pin
    let (reads, writes, partials) = view.function("check:r");
    assert_eq!(reads, [Cells::pin(0)]);
    assert_eq!(writes, [Cells::pin(3)]);
    assert!(partials.is_empty());
    // a param with no `where` has neither
    let vsource = m.module("Vsource").prototype();
    assert!(vsource.pins.iter().all(|p| p.flags & pin_flag::HAS_CHECK == 0));
    assert!(View { shape: vsource }.function_names().iter().all(|n| !n.starts_with("check:")));
    // the diode's three
    let diode = View { shape: m.module("Diode").prototype() };
    for p in ["is", "n", "cj0"] {
        let (reads, writes, _) = diode.function(&format!("check:{p}"));
        assert_eq!(reads, [Cells::pin(diode.pin_index(p))]);
        assert_eq!(writes, [Cells::pin(diode.pin_index(&format!("check:{p}")))]);
    }
    // the function writes the bit: r > 0 is true, r <= 0 false
    let h = m.instance("Resistor");
    let k = Check::index(&m);
    for (r, want) in [(1000.0, [1u8]), (-1.0, [0]), (0.0, [0])] {
        let bytes = Check::bytes(r);
        let mut out = [0u8; 1];
        h.eval(k, &[&bytes[..]], &mut [&mut out[..]], &[], &mut []).expect("check");
        assert_eq!(out, want, "r = {r}");
    }
}

/// Small readers for the check function's call.
struct Check;

impl Check {
    fn bytes(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// The index of `check:r` among the resistor's functions.
    fn index(m: &Models) -> u32 {
        let names: Vec<String> = View { shape: m.module("Resistor").prototype() }.function_names().iter().map(|s| s.to_string()).collect();
        names.iter().position(|n| n == "check:r").expect("check:r") as u32
    }
}
