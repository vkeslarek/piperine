//! Scope: modules, nodes and placements of `piperine_lang::elab` (ELAB-03): the body of a
//! module (its params and nodes, ports first, natures lowered once with their tolerances), the
//! node table and the instance tree. Placement of children joins in T4.



use piperine_lang_fixtures::elaboration::SourceElaboration;
#[test]
fn an_empty_module_is_an_empty_body() {
    let ir = SourceElaboration::ir("module Top() { }", "Top");
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
fn a_node_init_that_reads_a_param_is_folded_per_variant() {
    // AS-15 AC 2 (it was `P0001` before `analog-structure` T34): the param is structural, so the body
    // variant `Top~p=1.0` holds the value of `p` as the node's initial value.
    let ir = SourceElaboration::ir("module Top() { p: f64 = 1.0; signal n: Electrical = p; }", "Top");
    let top = ir.bodies().find(|b| b.name() == "Top~p=1.0").expect("the variant of p = 1.0");
    assert_eq!(top.nodes().iter().map(|n| n.init).collect::<Vec<_>>(), [Some(piperine_ir::Real::new(1.0))], "n starts at the value of p");
}

#[test]
fn electrical_is_lowered_once_with_its_tolerances() {
    let ir = SourceElaboration::ir("module Top() { signal a: Electrical; signal b: Electrical; }", "Top");
    assert_eq!(ir.natures().count(), 1, "one nature however many signals use it");
    let text = ir.print_structural();
    assert!(text.contains("nature Electrical\n  potential Voltage v \"V\" abstol 1e-6\n  flow Current i \"A\" abstol 1e-12\n"), "{text}");
}

#[test]
fn a_raw_signal_name_is_a_node_without_the_prefix() {
    let ir = SourceElaboration::ir("module Top() { signal r#out: Electrical = 0.5; }", "Top");
    let node = ir.node_at("Top.out").expect("the node `out`");
    assert_eq!(node.init().map(|v| v.get()), Some(0.5));
    let body = ir.bodies().next().expect("one body");
    assert_eq!(body.nodes()[0].name, "out");
}

const LEAF: &str = "module Leaf(p: Electrical, q: Electrical) { signal inner: Electrical; }\n";

/// The paths of the instances of an IR.
struct InstancePaths;

impl InstancePaths {
    /// The paths of the instances, in table order.
    fn instance_paths(ir: &piperine_ir::Ir) -> Vec<String> {
        ir.instances().map(|i| i.path().to_string()).collect()
    }
}

#[test]
fn one_body_per_module_one_instance_per_placement() {
    let src = format!("{LEAF}module Mid(p: Electrical) {{ let x = Leaf {{ }}(p, gnd); }}\nmodule Top() {{ signal s: Electrical; let a = Leaf {{ }}(s, gnd); let m = Mid {{ }}(s); }}");
    let ir = SourceElaboration::ir(&src, "Top");
    assert_eq!(ir.bodies().count(), 3, "Top, Leaf and Mid: one body per module");
    assert_eq!(InstancePaths::instance_paths(&ir).len(), 4, "Top, Top.a, Top.m and Top.m.x: one instance per placement");
    assert_eq!(ir.instance_at("Top.a").unwrap().body().id(), ir.instance_at("Top.m.x").unwrap().body().id(), "a module under two parents is one body");
}

#[test]
fn a_module_under_two_parents_is_one_body() {
    let src = format!("{LEAF}module Left() {{ signal s: Electrical; let x = Leaf {{ }}(s, gnd); }}\nmodule Right() {{ signal s: Electrical; let x = Leaf {{ }}(s, gnd); }}\nmodule Top() {{ let l = Left {{ }}(); let r = Right {{ }}(); }}");
    let ir = SourceElaboration::ir(&src, "Top");
    assert_eq!(ir.bodies().filter(|b| b.name() == "Leaf").count(), 1);
    assert_eq!(ir.instances().filter(|i| i.body().name() == "Leaf").count(), 2);
}

#[test]
fn instances_come_in_pre_order() {
    let src = format!("{LEAF}module Mid() {{ signal s: Electrical; let x = Leaf {{ }}(s, gnd); let y = Leaf {{ }}(s, gnd); }}\nmodule Top() {{ let a = Mid {{ }}(); let b = Mid {{ }}(); }}");
    let ir = SourceElaboration::ir(&src, "Top");
    assert_eq!(InstancePaths::instance_paths(&ir), ["Top", "Top.a", "Top.a.x", "Top.a.y", "Top.b", "Top.b.x", "Top.b.y"]);
    // Children, too, in source order.
    assert_eq!(ir.top().children().map(|c| c.name().to_string()).collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn paths_start_at_the_top_module() {
    let src = format!("{LEAF}module Top() {{ signal r#out: Electrical; let l = Leaf {{ }}(r#out, gnd); }}");
    let ir = SourceElaboration::ir(&src, "Top");
    let mut nodes: Vec<&str> = ir.nodes().map(|n| n.path()).collect();
    nodes.sort_unstable();
    assert_eq!(nodes, ["Top.l.inner", "Top.out"], "a node is its instance's path and its name without `r#`");
    assert_eq!(ir.node_at("Top.l.inner").unwrap().declared_by().path(), "Top.l");
}

#[test]
fn signals_are_nodes_and_ports_bind_the_parents() {
    let src = format!("{LEAF}module Top() {{ signal s: Electrical; let l = Leaf {{ }}(s, gnd); }}");
    let ir = SourceElaboration::ir(&src, "Top");
    let leaf = ir.instance_at("Top.l").unwrap();
    assert_eq!(leaf.node(0).map(|n| n.path().to_string()), Some("Top.s".to_string()), "port `p` is the parent's `s`, not a node of its own");
    assert_eq!(leaf.binding(1), Some(piperine_ir::NodeRef::Ground), "port `q` is ground");
    assert_eq!(leaf.node(2).map(|n| n.path().to_string()), Some("Top.l.inner".to_string()), "the signal after the ports is the instance's own node");
    assert_eq!(ir.nodes().count(), 2, "a port is never a node of the table");
}

#[test]
fn a_childs_own_signal_is_own_in_the_authored_child_and_real_in_the_instance_table() {
    // S4 closed by `analog-structure` T35 (AS-16; it was `gnd` padding, ELAB-03's M1 amendment): the
    // authored view names the child's own node `own2`; the instance table carries the real node.
    let src = format!("{LEAF}module Top() {{ signal s: Electrical; let l = Leaf {{ }}(s, gnd); }}");
    let ir = SourceElaboration::ir(&src, "Top");
    let child = &ir.top().body().children()[0];
    assert_eq!(child.nodes.len(), 3, "one slot per node of the child's body");
    assert_eq!(child.nodes[2], piperine_ir::Terminal::Own(2), "the own signal is the child's own node");
    assert_eq!(ir.instance_at("Top.l").unwrap().node(2).map(|n| n.path().to_string()), Some("Top.l.inner".to_string()));
}

#[test]
fn a_node_init_is_recorded_on_body_and_design() {
    let ir = SourceElaboration::ir("module Top() { signal n: Electrical = 0.75; }", "Top");
    assert_eq!(ir.bodies().next().unwrap().nodes()[0].init.map(|v| v.get()), Some(0.75));
    assert_eq!(ir.node_at("Top.n").unwrap().init().map(|v| v.get()), Some(0.75));
}

#[test]
fn an_unnamed_placement_is_module_hash_k() {
    let src = format!("{LEAF}module Top() {{ signal s: Electrical; let a = {{ Leaf {{ }}(s, gnd) }}; let named = Leaf {{ }}(s, gnd); let b = {{ Leaf {{ }}(s, gnd) }}; }}");
    let ir = SourceElaboration::ir(&src, "Top");
    assert_eq!(InstancePaths::instance_paths(&ir), ["Top", "Top.Leaf#0", "Top.named", "Top.Leaf#2"].map(String::from), "k counts the placements of the module in source order, named ones included");
}

#[test]
fn a_module_that_places_itself_is_e0341() {
    let src = "module Loop() { let again = Loop { }(); }\nmodule Top() { let l = Loop { }(); }";
    let err = SourceElaboration::run(src, "Top").expect_err("recursion never ends");
    assert_eq!(SourceElaboration::codes(&err), ["E0341"]);
    assert!(err[0].diagnostic.message.contains("Loop"), "{}", err[0].diagnostic.message);
}

#[test]
fn a_module_that_places_itself_through_another_is_e0341() {
    let src = "module A() { let b = B { }(); }\nmodule B() { let a = A { }(); }\nmodule Top() { let a = A { }(); }";
    let err = SourceElaboration::run(src, "Top").expect_err("A -> B -> A never ends");
    assert_eq!(SourceElaboration::codes(&err), ["E0341"]);
    assert!(err[0].diagnostic.message.contains("Top.a.b.a"), "{}", err[0].diagnostic.message);
    assert!(err[0].diagnostic.message.contains('A'), "{}", err[0].diagnostic.message);
}

#[test]
fn an_unconnected_node_stays_in_the_table() {
    let ir = SourceElaboration::ir("module Top() { signal floating: Electrical; }", "Top");
    assert!(ir.node_at("Top.floating").is_some(), "a floating node is the solver's to report (E0503)");
}

/// A cell with a required-free param `cs`, a placement of it through the param `cell` of `Top`,
/// and a parent that may bind it (PI-04).
const CELL: &str = "module Cell(a: Electrical) { cs: f64 = 1.0; }\nanalog for Cell { i(a, gnd) +> v(a, gnd) * cs; }\n";

/// The instances of an elaborated design, read by path.
struct Placed;

impl Placed {
    /// The value of the first param of the instance at `path`.
    fn first_param(ir: &piperine_ir::Ir, path: &str) -> piperine_ir::Constant {
        *ir.instance_at(path).unwrap_or_else(|| panic!("no instance {path}")).param(0).expect("the instance has a param")
    }
}

#[test]
fn a_placement_through_a_module_param_places_its_template() {
    // PI-04 AC 1, §8.4.9: `cell(x)` with `cell: Cell = Cell { cs: 30.0 }` places a `Cell` with `cs`
    // 30, the instance `Cell(x) { cs: 30.0 }` places
    let through = SourceElaboration::ir(&format!("{CELL}module Top() {{ cell: Cell = Cell {{ cs: 30.0 }}; signal x: Electrical; let c1 = cell(x); }}"), "Top");
    let direct = SourceElaboration::ir(&format!("{CELL}module Top() {{ signal x: Electrical; let c1 = Cell(x) {{ cs: 30.0 }}; }}"), "Top");
    assert_eq!(Placed::first_param(&through, "Top.c1"), piperine_ir::Constant::Real(piperine_ir::Real::new(30.0)));
    assert_eq!(Placed::first_param(&through, "Top.c1"), Placed::first_param(&direct, "Top.c1"));
    assert_eq!(through.instance_at("Top.c1").map(|i| i.body().name().to_string()), direct.instance_at("Top.c1").map(|i| i.body().name().to_string()));
}

#[test]
fn a_parent_binding_replaces_the_template() {
    // PI-04 AC 2: `Top { cell: Cell { cs: 10.0 } }` replaces the default template
    let src = format!("{CELL}module Top() {{ cell: Cell = Cell {{ cs: 30.0 }}; signal x: Electrical; let c1 = cell(x); }}\nmodule Parent() {{ let t = Top {{ cell: Cell {{ cs: 10.0 }} }}(); }}");
    let ir = SourceElaboration::ir(&src, "Parent");
    assert_eq!(Placed::first_param(&ir, "Parent.t.c1"), piperine_ir::Constant::Real(piperine_ir::Real::new(10.0)));
    // the same `Top` with its default places the other template: two templates, two bodies
    let both = format!("{CELL}module Top() {{ cell: Cell = Cell {{ cs: 30.0 }}; signal x: Electrical; let c1 = cell(x); }}\nmodule Parent() {{ let t = Top {{ cell: Cell {{ cs: 10.0 }} }}(); let u = Top(); }}");
    let ir = SourceElaboration::ir(&both, "Parent");
    assert_eq!(Placed::first_param(&ir, "Parent.t.c1"), piperine_ir::Constant::Real(piperine_ir::Real::new(10.0)));
    assert_eq!(Placed::first_param(&ir, "Parent.u.c1"), piperine_ir::Constant::Real(piperine_ir::Real::new(30.0)));
}

#[test]
fn a_template_leaving_a_required_param_unbound_is_e0319() {
    // PI-04 AC 4, §8.4.2: a template of a module with a required param it does not bind
    let src = "module Cell(a: Electrical) { cs: f64; }\nanalog for Cell { i(a, gnd) +> v(a, gnd) * cs; }\nmodule Top() { cell: Cell = Cell { }; signal x: Electrical; let c1 = cell(x); }";
    let err = SourceElaboration::run(src, "Top").expect_err("the template leaves `cs` unbound");
    assert_eq!(SourceElaboration::codes(&err), ["E0319"]);
    assert!(err[0].diagnostic.message.contains("`cs`"), "{}", err[0].diagnostic.message);
}

#[test]
fn the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module() {
    // PI-04 AC 6, MD-25: the POM shows `cell(x)` as written (the param `cell` and the placement
    // `c1`, no `Cell(..)` placement); the IR holds the concrete instance of `Cell`
    let src = format!("{CELL}module Top() {{ cell: Cell = Cell {{ cs: 30.0 }}; signal x: Electrical; let c1 = cell(x); }}");
    let design = SourceElaboration::design(&src);
    let before = design.dump();
    let ir = piperine_lang::elab::Elaborator::new(&design).run("Top").unwrap_or_else(|d| panic!("{d:?}"));
    assert_eq!(design.dump(), before, "elaboration leaves the POM as it was");
    assert!(before.contains("Param cell") && before.contains("Binding c1"), "{before}");
    let instance = ir.instance_at("Top.c1").expect("the concrete instance");
    assert!(instance.body().name().starts_with("Cell"), "{}", instance.body().name());
    assert_eq!(ir.top().children().map(|c| c.path().to_string()).collect::<Vec<_>>(), ["Top.c1"]);
}
