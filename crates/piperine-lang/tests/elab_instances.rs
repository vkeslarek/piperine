//! Scope: modules, nodes and placements of `piperine_lang::elab` (ELAB-03): the body of a
//! module (its params and nodes, ports first, natures lowered once with their tolerances), the
//! node table and the instance tree. Placement of children joins in T4.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

#[test]
fn an_empty_module_is_an_empty_body() {
    let ir = Elab::ir("module Top() { }", "Top");
    let body = ir.bodies().next().expect("one body");
    assert_eq!(body.name(), "Top");
    assert!(body.params().is_empty() && body.nodes().is_empty() && body.branches().is_empty());
    assert!(body.limits().is_empty() && body.stims().is_empty() && body.locals().is_empty());
    assert!(body.analog().is_none(), "no analog block, none lowered");
    assert!(body.children().is_empty());
    let top = ir.top();
    assert!(top.children().next().is_none() && top.bindings().is_empty() && top.params().is_empty());
    assert_eq!(ir.nodes().count(), 0);
}

#[test]
fn a_node_init_that_reads_a_param_is_p0001() {
    let src = "module Top() { p: f64 = 1.0; signal n: Electrical = p; }";
    let err = Elab::run(src, "Top").expect_err("a node's initial value cannot read a param");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "p", "the diagnostic sits at the value");
}

#[test]
fn electrical_is_lowered_once_with_its_tolerances() {
    let ir = Elab::ir("module Top() { signal a: Electrical; signal b: Electrical; }", "Top");
    assert_eq!(ir.natures().count(), 1, "one nature however many signals use it");
    let text = ir.print_structural();
    assert!(text.contains("nature Electrical\n  potential Voltage v \"V\" abstol 1e-6\n  flow Current i \"A\" abstol 1e-12\n"), "{text}");
}

#[test]
fn a_raw_signal_name_is_a_node_without_the_prefix() {
    let ir = Elab::ir("module Top() { signal r#out: Electrical = 0.5; }", "Top");
    let node = ir.node_at("Top.out").expect("the node `out`");
    assert_eq!(node.init().map(|v| v.get()), Some(0.5));
    let body = ir.bodies().next().expect("one body");
    assert_eq!(body.nodes()[0].name, "out");
}

const LEAF: &str = "module Leaf(p: Electrical, q: Electrical) { signal inner: Electrical; }\n";

/// The paths of the instances, in table order.
fn instance_paths(ir: &piperine_ir::Ir) -> Vec<String> {
    ir.instances().map(|i| i.path().to_string()).collect()
}

#[test]
fn one_body_per_module_one_instance_per_placement() {
    let src = format!("{LEAF}module Mid(p: Electrical) {{ let x = Leaf {{ }}(p, gnd); }}\nmodule Top() {{ signal s: Electrical; let a = Leaf {{ }}(s, gnd); let m = Mid {{ }}(s); }}");
    let ir = Elab::ir(&src, "Top");
    assert_eq!(ir.bodies().count(), 3, "Top, Leaf and Mid: one body per module");
    assert_eq!(instance_paths(&ir).len(), 4, "Top, Top.a, Top.m and Top.m.x: one instance per placement");
    assert_eq!(ir.instance_at("Top.a").unwrap().body().id(), ir.instance_at("Top.m.x").unwrap().body().id(), "a module under two parents is one body");
}

#[test]
fn a_module_under_two_parents_is_one_body() {
    let src = format!("{LEAF}module Left() {{ signal s: Electrical; let x = Leaf {{ }}(s, gnd); }}\nmodule Right() {{ signal s: Electrical; let x = Leaf {{ }}(s, gnd); }}\nmodule Top() {{ let l = Left {{ }}(); let r = Right {{ }}(); }}");
    let ir = Elab::ir(&src, "Top");
    assert_eq!(ir.bodies().filter(|b| b.name() == "Leaf").count(), 1);
    assert_eq!(ir.instances().filter(|i| i.body().name() == "Leaf").count(), 2);
}

#[test]
fn instances_come_in_pre_order() {
    let src = format!("{LEAF}module Mid() {{ signal s: Electrical; let x = Leaf {{ }}(s, gnd); let y = Leaf {{ }}(s, gnd); }}\nmodule Top() {{ let a = Mid {{ }}(); let b = Mid {{ }}(); }}");
    let ir = Elab::ir(&src, "Top");
    assert_eq!(instance_paths(&ir), ["Top", "Top.a", "Top.a.x", "Top.a.y", "Top.b", "Top.b.x", "Top.b.y"]);
    // Children, too, in source order.
    assert_eq!(ir.top().children().map(|c| c.name().to_string()).collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn paths_start_at_the_top_module() {
    let src = format!("{LEAF}module Top() {{ signal r#out: Electrical; let l = Leaf {{ }}(r#out, gnd); }}");
    let ir = Elab::ir(&src, "Top");
    let mut nodes: Vec<&str> = ir.nodes().map(|n| n.path()).collect();
    nodes.sort_unstable();
    assert_eq!(nodes, ["Top.l.inner", "Top.out"], "a node is its instance's path and its name without `r#`");
    assert_eq!(ir.node_at("Top.l.inner").unwrap().declared_by().path(), "Top.l");
}

#[test]
fn signals_are_nodes_and_ports_bind_the_parents() {
    let src = format!("{LEAF}module Top() {{ signal s: Electrical; let l = Leaf {{ }}(s, gnd); }}");
    let ir = Elab::ir(&src, "Top");
    let leaf = ir.instance_at("Top.l").unwrap();
    assert_eq!(leaf.node(0).map(|n| n.path().to_string()), Some("Top.s".to_string()), "port `p` is the parent's `s`, not a node of its own");
    assert_eq!(leaf.binding(1), Some(piperine_ir::NodeRef::Ground), "port `q` is ground");
    assert_eq!(leaf.node(2).map(|n| n.path().to_string()), Some("Top.l.inner".to_string()), "the signal after the ports is the instance's own node");
    assert_eq!(ir.nodes().count(), 2, "a port is never a node of the table");
}

#[test]
fn a_node_init_is_recorded_on_body_and_design() {
    let ir = Elab::ir("module Top() { signal n: Electrical = 0.75; }", "Top");
    assert_eq!(ir.bodies().next().unwrap().nodes()[0].init.map(|v| v.get()), Some(0.75));
    assert_eq!(ir.node_at("Top.n").unwrap().init().map(|v| v.get()), Some(0.75));
}

#[test]
fn an_unnamed_placement_is_module_hash_k() {
    let src = format!("{LEAF}module Top() {{ signal s: Electrical; let a = {{ Leaf {{ }}(s, gnd) }}; let named = Leaf {{ }}(s, gnd); let b = {{ Leaf {{ }}(s, gnd) }}; }}");
    let ir = Elab::ir(&src, "Top");
    assert_eq!(instance_paths(&ir), ["Top", "Top.Leaf#0", "Top.named", "Top.Leaf#2"].map(String::from), "k counts the placements of the module in source order, named ones included");
}

#[test]
fn a_module_that_places_itself_is_e0341() {
    let src = "module Loop() { let again = Loop { }(); }\nmodule Top() { let l = Loop { }(); }";
    let err = Elab::run(src, "Top").expect_err("recursion never ends");
    assert_eq!(Elab::codes(&err), ["E0341"]);
    assert!(err[0].diagnostic.message.contains("Loop"), "{}", err[0].diagnostic.message);
}

#[test]
fn a_module_that_places_itself_through_another_is_e0341() {
    let src = "module A() { let b = B { }(); }\nmodule B() { let a = A { }(); }\nmodule Top() { let a = A { }(); }";
    let err = Elab::run(src, "Top").expect_err("A -> B -> A never ends");
    assert_eq!(Elab::codes(&err), ["E0341"]);
    assert!(err[0].diagnostic.message.contains("Top.a.b.a"), "{}", err[0].diagnostic.message);
    assert!(err[0].diagnostic.message.contains('A'), "{}", err[0].diagnostic.message);
}

#[test]
fn an_unconnected_node_stays_in_the_table() {
    let ir = Elab::ir("module Top() { signal floating: Electrical; }", "Top");
    assert!(ir.node_at("Top.floating").is_some(), "a floating node is the solver's to report (E0503)");
}
