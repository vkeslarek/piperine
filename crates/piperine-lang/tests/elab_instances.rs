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
