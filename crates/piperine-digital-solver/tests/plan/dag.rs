//! The levelized order of the discrete functions (feature
//! `solver-abi-digital`, SAD-05 AC 10, SAD-06, SAD-07; abi.md §3.1 P7, §9 D1):
//! every function after the writers of what it reads, cycles refused unless
//! they pass through a `RELAX` net, a stable order.

use piperine_digital_solver::dag::{Dag, Step};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::plan::{FnNode, NetPart};
use piperine_digital_solver::error::DigitalError;

#[test]
fn a_function_runs_after_every_writer_of_its_reads() {
    // C11, first half. A diamond: a -> {b, c} -> d, listed with the sink first.
    let fns = vec![
        DagDesigns::func("d", 0, 3, &[1, 2], &[3]),
        DagDesigns::func("c", 0, 2, &[0], &[2]),
        DagDesigns::func("b", 0, 1, &[0], &[1]),
        DagDesigns::func("a", 0, 0, &[9], &[0]),
    ];
    let dag = Dag::build(&fns, &DagDesigns::nets(10, &[])).expect("a diamond is acyclic");
    let order = DagOrder::run_order(&dag);
    assert_eq!(order.len(), 4);
    assert!(DagOrder::position(&order, "a") < DagOrder::position(&order, "b"));
    assert!(DagOrder::position(&order, "a") < DagOrder::position(&order, "c"));
    assert!(DagOrder::position(&order, "b") < DagOrder::position(&order, "d"));
    assert!(DagOrder::position(&order, "c") < DagOrder::position(&order, "d"));

    // a chain of ten, handed over in an interleaved order: f_i reads n_i and writes n_(i+1)
    let mut chain: Vec<FnNode> = (0..10).map(|i| DagDesigns::func(&format!("f{i}"), i, 0, &[i], &[i + 1])).collect();
    chain.reverse();
    chain.rotate_left(3);
    let dag = Dag::build(&chain, &DagDesigns::nets(11, &[])).expect("a chain is acyclic");
    let order = DagOrder::run_order(&dag);
    let expected: Vec<String> = (0..10).map(|i| format!("f{i}")).collect();
    assert_eq!(order, expected);
}

#[test]
fn a_cycle_through_no_relax_net_is_refused_naming_its_functions() {
    // C10: two cross-coupled gates
    let gates = vec![DagDesigns::func("g1", 0, 0, &[0, 2], &[1]), DagDesigns::func("g2", 1, 0, &[1, 3], &[2])];
    match Dag::build(&gates, &DagDesigns::nets(4, &[])) {
        Err(DigitalError::Bind { rule, what }) => {
            assert_eq!(rule, "cycle");
            let text = what.join(" ");
            assert!(text.contains("g1") && text.contains("g2"), "both functions named: {text}");
        }
        other => panic!("expected a cycle error, got {other:?}"),
    }
    // a self-loop
    let looped = vec![DagDesigns::func("osc", 0, 0, &[0], &[0])];
    match Dag::build(&looped, &DagDesigns::nets(1, &[])) {
        Err(DigitalError::Bind { rule, what }) => {
            assert_eq!(rule, "cycle");
            assert!(what.join(" ").contains("osc"));
        }
        other => panic!("expected a cycle error, got {other:?}"),
    }
    // a function outside the cycle is not named
    let mixed = vec![DagDesigns::func("g1", 0, 0, &[2], &[1]), DagDesigns::func("g2", 1, 0, &[1], &[2]), DagDesigns::func("bystander", 2, 0, &[1], &[5])];
    match Dag::build(&mixed, &DagDesigns::nets(6, &[])) {
        Err(DigitalError::Bind { what, .. }) => assert!(!what.join(" ").contains("bystander")),
        other => panic!("expected a cycle error, got {other:?}"),
    }
}

#[test]
fn a_cycle_through_relax_nets_is_one_relax_step() {
    // a resolver loop: drive -> value -> drive, value flagged RELAX
    let fns = vec![
        DagDesigns::func("driver", 0, 0, &[0, 1], &[2]),
        DagDesigns::func("resolve", 1, 0, &[2], &[1]),
        DagDesigns::func("after", 2, 0, &[1], &[3]),
        DagDesigns::func("before", 3, 0, &[4], &[0]),
    ];
    let dag = Dag::build(&fns, &DagDesigns::nets(5, &[1])).expect("the cycle passes through a RELAX net");
    let steps = dag.order();
    assert_eq!(steps.len(), 3, "before, one relax group, after");
    let Step::Relax { fns: group, nets: relaxed } = &steps[1] else { panic!("the middle step relaxes: {steps:?}") };
    let names: Vec<&str> = group.iter().map(|f| dag.function(*f).name.as_str()).collect();
    assert_eq!(names, ["driver", "resolve"]);
    assert_eq!(relaxed, &[NetId(1)], "the RELAX nets of the cycle");
    assert!(matches!(steps[0], Step::Run(f) if dag.function(f).name == "before"));
    assert!(matches!(steps[2], Step::Run(f) if dag.function(f).name == "after"));
}

#[test]
fn the_order_is_stable_across_runs() {
    // independent functions: (instance, function) order whatever the input order
    let a: Vec<FnNode> = vec![
        DagDesigns::func("u2.f1", 2, 1, &[0], &[5]),
        DagDesigns::func("u0.f0", 0, 0, &[0], &[3]),
        DagDesigns::func("u2.f0", 2, 0, &[0], &[4]),
        DagDesigns::func("u1.f0", 1, 0, &[0], &[6]),
    ];
    let mut b = a.clone();
    b.reverse();
    let (da, db) = (Dag::build(&a, &DagDesigns::nets(7, &[])).unwrap(), Dag::build(&b, &DagDesigns::nets(7, &[])).unwrap());
    let want = ["u0.f0", "u1.f0", "u2.f0", "u2.f1"];
    assert_eq!(DagOrder::run_order(&da), want);
    assert_eq!(DagOrder::run_order(&db), want);
    assert_eq!(DagOrder::run_order(&Dag::build(&a, &DagDesigns::nets(7, &[])).unwrap()), DagOrder::run_order(&da), "the same input, the same order");
}

#[test]
fn readers_lists_the_functions_that_read_a_net() {
    let fns = vec![DagDesigns::func("a", 0, 0, &[0], &[1]), DagDesigns::func("b", 1, 0, &[1], &[2]), DagDesigns::func("c", 2, 0, &[1], &[3])];
    let dag = Dag::build(&fns, &DagDesigns::nets(4, &[])).unwrap();
    let names: Vec<&str> = dag.readers(NetId(1)).iter().map(|f| dag.function(*f).name.as_str()).collect();
    assert_eq!(names, ["b", "c"]);
    assert!(dag.readers(NetId(3)).is_empty());
}

#[test]
fn an_inner_cycle_through_no_relax_net_inside_a_relax_component_is_refused() {
    // round-1 finding 3, abi.md §9 D1: every directed cycle needs a RELAX net. g1 <-> g2 passes
    // through n1 and n2 only; the loop through r passes through n3, which is RELAX.
    let fns = vec![DagDesigns::func("g1", 0, 0, &[2], &[1]), DagDesigns::func("g2", 1, 0, &[1, 3], &[2]), DagDesigns::func("r", 2, 0, &[2], &[3])];
    match Dag::build(&fns, &DagDesigns::nets(4, &[3])) {
        Err(DigitalError::Bind { rule, what }) => {
            assert_eq!(rule, "cycle");
            let text = what.join(" ");
            assert!(text.contains("g1") && text.contains("g2"), "the offending cycle is named: {text}");
            assert!(!what.iter().any(|w| w == "r"), "the resolver is not part of it: {what:?}");
        }
        other => panic!("expected a cycle error, got {other:?}"),
    }
    // the resolver loop alone, through the RELAX net, is accepted
    let ok = vec![DagDesigns::func("g2", 1, 0, &[3], &[2]), DagDesigns::func("r", 2, 0, &[2], &[3])];
    assert!(Dag::build(&ok, &DagDesigns::nets(4, &[3])).is_ok());
}

#[test]
fn definitions_of_different_elements_of_one_array_net_chain_without_a_cycle() {
    // abi.md §9 D1 at element granularity (part III §8.2.1): `z[1] = z[0] + 1` reads element 0 and writes element 1 of one
    // net, so it runs after `z[0] = y[1]` and is no cycle; `z[0]` reads `y[1]`, written by `y1`
    let fns = vec![DagDesigns::element_func("z1", 2, &[(1, 0)], &[(1, 1)]), DagDesigns::element_func("z0", 1, &[(0, 1)], &[(1, 0)]), DagDesigns::element_func("y1", 0, &[(2, 0)], &[(0, 1)])];
    let dag = Dag::build(&fns, &DagDesigns::nets(3, &[])).expect("different elements make no cycle");
    let order = DagOrder::run_order(&dag);
    assert!(DagOrder::position(&order, "y1") < DagOrder::position(&order, "z0") && DagOrder::position(&order, "z0") < DagOrder::position(&order, "z1"), "{order:?}");
    // a function that reads the element it writes is a cycle still
    let looped = vec![DagDesigns::element_func("z1", 0, &[(1, 1)], &[(1, 1)])];
    assert!(matches!(Dag::build(&looped, &DagDesigns::nets(2, &[])), Err(DigitalError::Bind { rule: "cycle", .. })));
    // a read of the whole net depends on every element's writer
    let mut whole = DagDesigns::element_func("sum", 1, &[], &[(2, 0)]);
    whole.reads = vec![NetId(1)];
    whole.read_parts = vec![NetPart::whole(NetId(1))];
    let fns = vec![whole, DagDesigns::element_func("z1", 0, &[(0, 0)], &[(1, 1)])];
    let order = DagOrder::run_order(&Dag::build(&fns, &DagDesigns::nets(3, &[])).expect("acyclic"));
    assert!(DagOrder::position(&order, "z1") < DagOrder::position(&order, "sum"), "{order:?}");
}

/// The functions and nets the order tests build the dag from.
struct DagDesigns;

impl DagDesigns {
    /// The func helper `func`.
    fn func(name: &str, instance: u32, k: u32, reads: &[u32], writes: &[u32]) -> FnNode {
        FnNode::over_whole_nets(name, instance, k, reads.iter().map(|n| NetId(*n)).collect(), writes.iter().map(|n| NetId(*n)).collect())
    }

    /// The nets helper `nets`.
    fn nets(n: usize, relax: &[usize]) -> Vec<NetSpec> {
        (0..n)
            .map(|i| NetSpec { relax: relax.contains(&i), ..NetSpec::discrete(&format!("n{i}"), 1, 1) })
            .collect()
    }

    /// A function over elements of array nets: its reads and writes each name one element.
    fn element_func(name: &str, k: u32, reads: &[(u32, u64)], writes: &[(u32, u64)]) -> FnNode {
        let part = |(net, element): &(u32, u64)| NetPart { net: NetId(*net), elements: Some((*element, 1)) };
        FnNode {
            name: name.to_string(),
            instance: 0,
            k,
            reads: reads.iter().map(|(net, _)| NetId(*net)).collect(),
            writes: writes.iter().map(|(net, _)| NetId(*net)).collect(),
            read_parts: reads.iter().map(part).collect(),
            write_parts: writes.iter().map(part).collect(),
        }
    }
}

/// The order the dag runs the functions in.
struct DagOrder;

impl DagOrder {
    /// The names of the functions in the order the dag runs them (a relax step flattened).
    fn run_order(dag: &Dag) -> Vec<String> {
        let mut out = Vec::new();
        for step in dag.order() {
            match step {
                Step::Run(f) => out.push(dag.function(*f).name.clone()),
                Step::Relax { fns, .. } => out.extend(fns.iter().map(|f| dag.function(*f).name.clone())),
            }
        }
        out
    }

    /// The position helper `position`.
    fn position(order: &[String], name: &str) -> usize {
        order.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} is not in {order:?}"))
    }
}
