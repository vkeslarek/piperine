//! Bind (SAI-01, SAI-09; design `bind`): how a netlist of device instances
//! becomes unknowns, nets and the two plans. One fixture module carries a pin
//! of every role (`PARAM`, `PORT` continuous and discrete, `GLOBAL`, `NAMED`,
//! `PRIVATE`) and the tests read what bind made of it by name and by the
//! plans' own nets.

#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BYTE, F64};
use piperine_abi::raw::{Cell, DEFAULT, Module, Shape, cell, global, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port, Site};
use piperine_solver::prelude::{AnalogReference, CircuitBuilder, Tolerances};

const R_DEFAULT: [u8; 8] = 1.0e3_f64.to_le_bytes();

/// pins: r=0 (PARAM F64, default 1e3); a=1 (PORT, abstol 1e-9); b=2 (PORT, abstol 4e-10);
/// code=3, other=4 (PORT BYTE); temp=5 (GLOBAL TEMPERATURE); k=6 (GLOBAL NAMED);
/// i=7 (PRIVATE continuous)
/// terms: T0 FLOW{a, b}
/// fn 0 "i": reads [a, b, r, code, temp, k, i] writes [T0]   (`other` is read by no function)
struct Probe;
static PROBE: OnceLock<ShapeRef> = OnceLock::new();
static MODULE: OnceLock<Module> = OnceLock::new();

fn pin(index: u32) -> Cell {
    Cell { kind: cell::PIN, index, part: part::VALUE }
}

fn probe_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &R_DEFAULT, ..PinSpec::discrete("r", role::PARAM, &F64) });
    let a = s.pin(PinSpec { abstol: [1.0e-9, 0.0], ..PinSpec::continuous("a", role::PORT) });
    let b = s.pin(PinSpec { abstol: [4.0e-10, 0.0], ..PinSpec::continuous("b", role::PORT) });
    s.pin(PinSpec::discrete("code", role::PORT, &BYTE));
    s.pin(PinSpec::discrete("other", role::PORT, &BYTE));
    s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("temp", role::GLOBAL, &F64) });
    s.pin(PinSpec { global: global::NAMED, ..PinSpec::discrete("k", role::GLOBAL, &F64) });
    s.pin(PinSpec::continuous("i", role::PRIVATE));
    let t0 = s.term(TermSpec { name: "i", origin: "bind.rs", op: op::FLOW, plus: a, minus: b, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        reads: vec![pin(a), pin(b), pin(0), pin(3), pin(5), pin(6), pin(7)],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for Probe {
    fn prototype() -> &'static Shape<'static> {
        PROBE.get_or_init(probe_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Probe, Arc::clone(PROBE.get_or_init(probe_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, 0.0_f64);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn probe() -> &'static Module {
    MODULE.get_or_init(|| module::<Probe>("probe"))
}

fn real(bytes: &[u8]) -> f64 {
    f64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"))
}

#[test]
fn pins_bind_by_role() {
    let mut b = CircuitBuilder::new("roles");
    let gnd = b.ground();
    let (n1, n2) = (b.node("n1"), b.node("n2"));
    b.session_param("k", ParamValue::Real(7.0));
    b.device(
        probe(),
        "x1",
        &[("r", ParamValue::Real(2.0e3))],
        &[Port::Node(n1.clone()), Port::Node(n2.clone()), Port::Net("code".into()), Port::Net("o1".into())],
    );
    b.device(
        probe(),
        "x2",
        &[],
        &[Port::Node(n2.clone()), Port::Node(gnd), Port::Net("code".into()), Port::Net("o2".into())],
    );
    let circuit = b.build().expect("bind");
    let bound = circuit.bound().expect("a device circuit");
    let (names, plan) = (&bound.names, bound.digital.plan());

    // PORT pins bind by position: the continuous ones to the nodes, the discrete ones to named nets.
    assert_eq!(names.pins[0][1], Site::Unknown(n1));
    assert_eq!(names.pins[0][2], Site::Unknown(n2.clone()));
    assert_eq!(names.pins[1][1], Site::Unknown(n2));
    assert_eq!(names.pins[1][2], Site::Ground);
    assert_eq!(names.pins[0][3], names.pins[1][3], "ports of one name share a net");
    assert_ne!(names.pins[0][4], names.pins[1][4]);

    // The circuit keeps the values each instance was created with, by name, and no others.
    assert_eq!(circuit.given("x1"), Some(&[("r".to_string(), ParamValue::Real(2.0e3))][..]));
    assert_eq!(circuit.given("x2"), Some(&[][..]));
    assert_eq!(circuit.given("x3"), None);

    // A PARAM binds by name to a net seeded with the value given, else the prototype's default.
    let seed = |path: &str| real(&plan.nets[names.params[&(path.to_string(), "r".to_string())].0 as usize].init);
    assert_eq!(seed("x1"), 2.0e3);
    assert_eq!(seed("x2"), 1.0e3);

    // A GLOBAL binds by constant to one design-wide net, a NAMED one by name.
    assert_eq!(names.pins[0][5], names.pins[1][5]);
    let Site::Net(temp) = names.pins[0][5] else { panic!("temp is a net") };
    assert_eq!(real(&plan.nets[temp.0 as usize].init), Tolerances::default().temperature);
    let Site::Net(k) = names.pins[0][6] else { panic!("k is a net") };
    assert_eq!(names.pins[1][6], Site::Net(k));
    assert_eq!(real(&plan.nets[k.0 as usize].init), 7.0);

    // A PRIVATE pin is allocated: a branch unknown of its own per instance.
    let (Site::Unknown(i1), Site::Unknown(i2)) = (&names.pins[0][7], &names.pins[1][7]) else {
        panic!("i is an unknown");
    };
    assert!(i1.is_branch() && i2.is_branch() && i1 != i2);
}

#[test]
fn a_built_circuit_names_each_node_as_the_builder_did() {
    let mut b = CircuitBuilder::new("names");
    let gnd = b.ground();
    let (top, mid) = (b.node("Top.top"), b.node("Top.mid"));
    b.session_param("k", ParamValue::Real(7.0));
    b.device(probe(), "x1", &[], &[Port::Node(top.clone()), Port::Node(mid.clone()), Port::Net("c".into()), Port::Net("o".into())]);
    b.device(probe(), "x2", &[], &[Port::Node(mid.clone()), Port::Node(gnd), Port::Net("c".into()), Port::Net("o2".into())]);
    let circuit = b.build().expect("bind");
    assert_eq!(circuit.node("Top.top"), Some(&top));
    assert_eq!(circuit.node("Top.mid"), Some(&mid));
    assert_eq!(circuit.node("Top.out"), None);
    let label = |r: &AnalogReference| {
        circuit.nets().into_iter().find(|n| n.analog_variable() == Some(r.variable())).expect("a solved node").label().to_string()
    };
    assert_eq!(label(&top), "v(Top.top)");
    assert_eq!(label(&mid), "v(Top.mid)");
}

#[test]
fn a_named_global_nobody_set_fails_the_bind_naming_it() {
    let mut b = CircuitBuilder::new("named");
    let n1 = b.node("n1");
    b.device(probe(), "x1", &[], &[Port::Node(n1.clone()), Port::Node(n1)]);
    let err = b.build().err().expect("a NAMED global with no session value").to_string();
    assert!(err.contains("`k`") && err.contains("x1"), "{err}");
}

#[test]
fn ports_on_one_net_share_one_unknown() {
    let mut b = CircuitBuilder::new("share");
    let (n1, n2) = (b.node("n1"), b.node("n2"));
    b.session_param("k", ParamValue::Real(1.0));
    for path in ["x1", "x2"] {
        b.device(probe(), path, &[], &[Port::Node(n1.clone()), Port::Node(n2.clone())]);
    }
    let circuit = b.build().expect("bind");
    let names = &circuit.bound().expect("bound").names;

    let unknown = |inst: usize, pin: usize| -> AnalogReference {
        match &names.pins[inst][pin] {
            Site::Unknown(r) => r.clone(),
            other => panic!("pin {pin} of instance {inst} is {other:?}"),
        }
    };
    assert_eq!(unknown(0, 1), unknown(1, 1));
    assert_eq!(unknown(0, 2), unknown(1, 2));
    // Each private continuous pin is an unknown of its own, allocated in instance order.
    let (p1, p2) = (unknown(0, 7), unknown(1, 7));
    assert!(p1.is_branch() && p2.is_branch());
    assert_ne!(p1, p2);
    assert!(p1.idx() < p2.idx());
    assert!(p1.idx() > unknown(0, 2).idx(), "private pins come after the nodes");
}

#[test]
fn a_pin_abstol_tightens_its_node() {
    let mut b = CircuitBuilder::new("abstol");
    let (n, m) = (b.node("n"), b.node("m"));
    b.session_param("k", ParamValue::Real(1.0));
    // x1: `a` (1e-9) on n, `b` (4e-10) on m.
    b.device(probe(), "x1", &[], &[Port::Node(n.clone()), Port::Node(m.clone())]);
    // x3: both pins on o.
    let o = b.node("o");
    b.device(probe(), "x3", &[], &[Port::Node(o.clone()), Port::Node(o.clone())]);
    let circuit = b.build().expect("bind");
    let bound = circuit.bound().expect("bound");
    let id = |r: &AnalogReference| match r.variable().as_ref() {
        piperine_solver::abi::AnalogVariable::Node(id) => id.clone(),
        other => panic!("{other:?}"),
    };

    // The smallest abstol of the pins on the node.
    assert_eq!(bound.vntol[&id(&n)], 1.0e-9);
    assert_eq!(bound.vntol[&id(&m)], 4.0e-10);
    assert_eq!(bound.vntol[&id(&o)], 4.0e-10, "a and b on one node: the smaller");

    // It reaches the convergence tests through the per-node override, never loosening one.
    let mut tol = Tolerances::default();
    let mut overrides = std::collections::HashMap::new();
    overrides.insert(id(&n), 1.0e-3);
    overrides.insert(id(&m), 1.0e-12);
    tol.node_vntol_overrides = std::rc::Rc::new(overrides);
    bound.tighten(&mut tol);
    assert_eq!(tol.node_vntol_overrides[&id(&n)], 1.0e-9);
    assert_eq!(tol.node_vntol_overrides[&id(&m)], 1.0e-12);
    assert_eq!(tol.node_vntol_overrides[&id(&o)], 4.0e-10);
}

#[test]
fn a_discrete_net_an_analog_function_reads_is_flagged() {
    let mut b = CircuitBuilder::new("flags");
    let n = b.node("n");
    b.session_param("k", ParamValue::Real(1.0));
    b.device(
        probe(),
        "x1",
        &[],
        &[Port::Node(n.clone()), Port::Node(n), Port::Net("read".into()), Port::Net("unread".into())],
    );
    let circuit = b.build().expect("bind");
    let bound = circuit.bound().expect("bound");
    let nets = &bound.digital.plan().nets;
    let net = |key: &str| &nets[bound.names.nets[key].0 as usize];

    assert!(net("net:read").analog_reads, "an analog function reads `code`");
    assert!(!net("net:unread").analog_reads, "no analog function reads `other`");
    // The parameter and the globals the function reads are the analog side's too.
    assert!(net("global:temperature").analog_reads);
    assert!(net("named:k").analog_reads);
    // Each flagged net has a slot in the analog copy.
    assert_eq!(bound.names.slot_nets.len(), 4, "code, r, temp and k");
}

#[test]
fn a_shared_param_net_feeds_every_instance_bound_to_it() {
    let mut b = CircuitBuilder::new("card");
    let n = b.node("n");
    b.session_param("k", ParamValue::Real(1.0));
    b.param_net("card", ParamValue::Real(5.0e3));
    for path in ["x1", "x2"] {
        b.device(
            probe(),
            path,
            &[("r", ParamValue::Shared("card".into()))],
            &[Port::Node(n.clone()), Port::Node(n.clone())],
        );
    }
    b.device(probe(), "x3", &[("r", ParamValue::Real(9.0e3))], &[Port::Node(n.clone()), Port::Node(n)]);
    let circuit = b.build().expect("bind");
    let bound = circuit.bound().expect("bound");
    let net = |path: &str| bound.names.params[&(path.to_string(), "r".to_string())];

    assert_eq!(net("x1"), net("x2"), "one net for the model card");
    assert_ne!(net("x1"), net("x3"));
    let nets = &bound.digital.plan().nets;
    assert_eq!(real(&nets[net("x1").0 as usize].init), 5.0e3);
    assert_eq!(real(&nets[net("x3").0 as usize].init), 9.0e3);
    assert_eq!(bound.names.slots[&net("x1")], bound.names.slots[&net("x2")], "one analog slot for both");
}

// ---------------------------------------------------------------------------
// One create per placement
// ---------------------------------------------------------------------------

static CREATES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// pins: a=0 (PORT continuous), nothing else; counts its `create` calls.
struct Counting;
static COUNTING_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static COUNTING_MODULE: OnceLock<Module> = OnceLock::new();

fn counting_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec::continuous("a", role::PORT));
    s.freeze()
}

impl Device for Counting {
    fn prototype() -> &'static Shape<'static> {
        COUNTING_SHAPE.get_or_init(counting_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        CREATES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok((Counting, Arc::clone(COUNTING_SHAPE.get_or_init(counting_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn bind_creates_each_placement_once_for_both_sides() {
    let module = COUNTING_MODULE.get_or_init(|| module::<Counting>("counting"));
    let mut b = CircuitBuilder::new("count");
    let n = b.node("n");
    for path in ["x1", "x2", "x3"] {
        b.device(module, path, &[], &[Port::Node(n.clone())]);
    }
    let before = CREATES.load(std::sync::atomic::Ordering::SeqCst);
    let circuit = b.build().expect("bind");
    assert_eq!(CREATES.load(std::sync::atomic::Ordering::SeqCst) - before, 3, "creates = placements");
    assert_eq!(circuit.bound().expect("bound").names.instances.len(), 3);
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Probe", MODULE.get_or_init(|| module::<Probe>("probe")));
    partials::verify_every_function("Counting", COUNTING_MODULE.get_or_init(|| module::<Counting>("counting")));
}
