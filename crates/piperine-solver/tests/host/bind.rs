//! Bind (SAI-01, SAI-09; design `bind`): how a netlist of device instances
//! becomes unknowns, nets and the two plans. One fixture module carries a pin
//! of every role (`PARAM`, `PORT` continuous and discrete, `GLOBAL`, `NAMED`,
//! `PRIVATE`) and the tests read what bind made of it by name and by the
//! plans' own nets.

use piperine_device_fixtures::solver_devices::bind::{BoundValuesProbe, CREATES, CreateCounter};

use piperine_abi::raw::Module;
use piperine_solver::abi::{ParamValue, Port, Site};
use piperine_solver::prelude::{AnalogReference, CircuitBuilder, Tolerances};

#[test]
fn pins_bind_by_role() {
    let mut b = CircuitBuilder::new("roles");
    let gnd = b.ground();
    let (n1, n2) = (b.node("n1"), b.node("n2"));
    b.session_param("k", ParamValue::Real(7.0));
    b.device(
        BindCases::probe(),
        "x1",
        &[("r", ParamValue::Real(2.0e3))],
        &[Port::Node(n1.clone()), Port::Node(n2.clone()), Port::Net("code".into()), Port::Net("o1".into())],
    );
    b.device(
        BindCases::probe(),
        "x2",
        &[],
        &[Port::Node(n2.clone()), Port::Node(gnd), Port::Net("code".into()), Port::Net("o2".into())],
    );
    let circuit = b.build().expect("bind");
    let bound = circuit.bound();
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
    let seed = |path: &str| BindCases::real(&plan.nets[names.params[&(path.to_string(), "r".to_string())].0 as usize].init);
    assert_eq!(seed("x1"), 2.0e3);
    assert_eq!(seed("x2"), 1.0e3);

    // A GLOBAL binds by constant to one design-wide net, a NAMED one by name.
    assert_eq!(names.pins[0][5], names.pins[1][5]);
    let Site::Net(temp) = names.pins[0][5] else { panic!("temp is a net") };
    assert_eq!(BindCases::real(&plan.nets[temp.0 as usize].init), Tolerances::default().temperature);
    let Site::Net(k) = names.pins[0][6] else { panic!("k is a net") };
    assert_eq!(names.pins[1][6], Site::Net(k));
    assert_eq!(BindCases::real(&plan.nets[k.0 as usize].init), 7.0);

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
    b.device(BindCases::probe(), "x1", &[], &[Port::Node(top.clone()), Port::Node(mid.clone()), Port::Net("c".into()), Port::Net("o".into())]);
    b.device(BindCases::probe(), "x2", &[], &[Port::Node(mid.clone()), Port::Node(gnd), Port::Net("c".into()), Port::Net("o2".into())]);
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
    b.device(BindCases::probe(), "x1", &[], &[Port::Node(n1.clone()), Port::Node(n1)]);
    let err = b.build().err().expect("a NAMED global with no session value").to_string();
    assert!(err.contains("`k`") && err.contains("x1"), "{err}");
}

#[test]
fn ports_on_one_net_share_one_unknown() {
    let mut b = CircuitBuilder::new("share");
    let (n1, n2) = (b.node("n1"), b.node("n2"));
    b.session_param("k", ParamValue::Real(1.0));
    for path in ["x1", "x2"] {
        b.device(BindCases::probe(), path, &[], &[Port::Node(n1.clone()), Port::Node(n2.clone())]);
    }
    let circuit = b.build().expect("bind");
    let names = &circuit.bound().names;

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
    b.device(BindCases::probe(), "x1", &[], &[Port::Node(n.clone()), Port::Node(m.clone())]);
    // x3: both pins on o.
    let o = b.node("o");
    b.device(BindCases::probe(), "x3", &[], &[Port::Node(o.clone()), Port::Node(o.clone())]);
    let circuit = b.build().expect("bind");
    let bound = circuit.bound();
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
        BindCases::probe(),
        "x1",
        &[],
        &[Port::Node(n.clone()), Port::Node(n), Port::Net("read".into()), Port::Net("unread".into())],
    );
    let circuit = b.build().expect("bind");
    let bound = circuit.bound();
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
            BindCases::probe(),
            path,
            &[("r", ParamValue::Shared("card".into()))],
            &[Port::Node(n.clone()), Port::Node(n.clone())],
        );
    }
    b.device(BindCases::probe(), "x3", &[("r", ParamValue::Real(9.0e3))], &[Port::Node(n.clone()), Port::Node(n)]);
    let circuit = b.build().expect("bind");
    let bound = circuit.bound();
    let net = |path: &str| bound.names.params[&(path.to_string(), "r".to_string())];

    assert_eq!(net("x1"), net("x2"), "one net for the model card");
    assert_ne!(net("x1"), net("x3"));
    let nets = &bound.digital.plan().nets;
    assert_eq!(BindCases::real(&nets[net("x1").0 as usize].init), 5.0e3);
    assert_eq!(BindCases::real(&nets[net("x3").0 as usize].init), 9.0e3);
    assert_eq!(bound.names.slots[&net("x1")], bound.names.slots[&net("x2")], "one analog slot for both");
}

#[test]
fn bind_creates_each_placement_once_for_both_sides() {
    let module = CreateCounter::module();
    let mut b = CircuitBuilder::new("count");
    let n = b.node("n");
    for path in ["x1", "x2", "x3"] {
        b.device(module, path, &[], &[Port::Node(n.clone())]);
    }
    let before = CREATES.load(std::sync::atomic::Ordering::SeqCst);
    let circuit = b.build().expect("bind");
    assert_eq!(CREATES.load(std::sync::atomic::Ordering::SeqCst) - before, 3, "creates = placements");
    assert_eq!(circuit.bound().names.instances.len(), 3);
}

#[test]
fn a_window_outside_its_nets_bytes_is_an_error_naming_the_net_and_the_window() {
    // a seed of 2 elements of 2 bytes holds elements 0 and 1: the window of elements 1..3 is outside it, and an empty copy would hide it
    let seed = [1u8, 2, 3, 4];
    let inside = piperine_solver::abi::NameMap::window_bytes("mem", Some((1, 1)), &seed, 2).expect("element 1 is inside");
    assert_eq!(inside, [3, 4]);
    let error = piperine_solver::abi::NameMap::window_bytes("mem", Some((1, 2)), &seed, 2).expect_err("element 2 is outside");
    let text = error.to_string();
    assert!(text.contains("`mem`") && text.contains("1..3") && text.contains("4 bytes"), "{text}");
    // a net with no seed has none to cut
    assert_eq!(piperine_solver::abi::NameMap::window_bytes("mem", Some((1, 2)), &[], 2).expect("no seed, no bytes"), Vec::<u8>::new());
}

/// The helpers of the bind tests.
pub struct BindCases;

impl BindCases {

    /// The probe helper `probe`.
    fn probe() -> &'static Module {
        BoundValuesProbe::module()
    }

    /// The real helper `real`.
    fn real(bytes: &[u8]) -> f64 {
        f64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"))
    }

}
