//! Scope: handle functions at elaboration (generics GN-11, GN-12, GN-29, part I §3.1.5): a call of a function with a
//! handle parameter is inlined where it stands, so no IR function exists for it and its body is written out over the
//! carrier or node the caller passed.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::Ir;
use support::Elab;

/// The IR of a design whose `analog` block of `Top` is `statement`, after `items`.
struct HandleDesigns;

impl HandleDesigns {
    /// The elaborated `Top`, which must pass `Ir::check`.
    fn analog(items: &str, statement: &str) -> Ir {
        Elab::ir(&format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; signal t: Thermal; }}\nanalog for Top {{ {statement} }}\n"), "Top")
    }
}

#[test]
fn a_handle_call_lowers_to_its_written_out_body() {
    // GN-11 AC 1, GN-29 / D11: a handle function is no IR function; the call is its body over the node passed: the
    // probe is a probe of `a`, and `diff(a, gnd)` of `through` reads the same through two levels of handles
    let items = "fn level(x: Electrical) -> f64 { v(x) }\nfn through(x: Electrical) -> f64 { level(x) * 2.0 }\nimpl Thermal { fn celsius(self) -> f64 { temp(self) - 273.15 } }\nfn generic<N: Nature>(n: N) -> f64 { potential(n) }";
    let ir = HandleDesigns::analog(items, "v(o) +> through(a) + t.celsius() + generic(a) + generic(t);");
    assert_eq!(ir.functions().count(), 0, "no IR function for a handle function: {:?}", ir.functions().map(|f| f.name().to_string()).collect::<Vec<_>>());
    let text = ir.to_string();
    assert!(text.contains("probe"), "the body is written out: {text}");
}

#[test]
fn a_recursion_through_a_handle_function_is_e0126() {
    // GN-11 / D6: a handle function is inlined at its call, so a recursion through one cannot be expanded: `E0126` naming
    // the function, never an IR recursion
    let items = "fn again(x: Electrical, n: u8) -> f64 { if n == 0 { v(x) } else { again(x, n - 1) } }";
    let src = format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; }}\nanalog for Top {{ v(o) +> again(a, 3); }}\n");
    let found = Elab::run(&src, "Top").err().expect("refused");
    assert!(found.iter().any(|d| d.diagnostic.code == "E0126" && d.diagnostic.message.contains("`again`")), "{:?}", found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
}

#[test]
fn a_let_bound_to_a_node_probes_the_node() {
    // §6 rule 4, §8.2.8, §8.4.8: a `let` bound to a node holds no value, its name stands for the node: `v(y)` lowers to the probe of
    // the node it was bound to, in a plain module, through a child placed by name and through a template
    let child = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); }\n";
    for (src, probe) in [
        ("module Top() { signal x: Electrical; signal p: Electrical; }\nanalog for Top { i(x, gnd) +> v(x, gnd); let y = x; i(p, gnd) +> v(y) - 1.0; }".to_string(), "(probe(x, gnd) - 1.0)"),
        (format!("{child}module Top() {{ signal x: Electrical; let c0 = Dram(x); signal p: Electrical; }}\nanalog for Top {{ let y = c0.sn; i(p, gnd) +> v(y); }}"), "direct probe(c0.sn, gnd)"),
        (format!("{child}module Bank<C: Fn(Electrical)>(a: Electrical) {{ cell: C; let c0 = cell(a); signal p: Electrical; }}\nanalog<C> for Bank<C> {{ let y = c0.sn; i(p, gnd) +> v(y); }}\nmodule Top() {{ signal x: Electrical; let bank = Bank::<_>(x) {{ cell: Dram {{ }} }}; }}"), "direct probe(c0.sn, gnd)"),
    ] {
        let printed = Elab::ir(&src, "Top").print_structural();
        assert!(printed.contains(probe), "{src}: {printed}");
    }
}

#[test]
fn a_contribution_through_a_let_bound_to_a_node_is_the_nodes() {
    // §6 rule 4, §11: `let y = x; v(y) +> 1.0;` lowers to the contribution `v(x) +> 1.0;` lowers to, and through a template the
    // potential of the reached node `c0.sn`
    let alias = Elab::ir("module Top() { signal x: Electrical; }\nanalog for Top { let y = x; v(y) +> 1.0; }", "Top").print_structural();
    let direct = Elab::ir("module Top() { signal x: Electrical; }\nanalog for Top { v(x) +> 1.0; }", "Top").print_structural();
    assert_eq!(alias, direct);
    let child = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); }\n";
    let template = Elab::ir(&format!("{child}module Bank<C: Fn(Electrical)>(a: Electrical) {{ cell: C; let c0 = cell(a); }}\nanalog<C> for Bank<C> {{ let y = c0.sn; v(y) +> 1.0; }}\nmodule Top() {{ signal x: Electrical; let bank = Bank::<_>(x) {{ cell: Dram {{ }} }}; }}"), "Top").print_structural();
    assert!(template.contains("branch b0 (c0.sn, gnd) potential") && template.contains("b0.potential +> direct 1.0"), "{template}");
}

#[test]
fn an_access_a_let_bound_nodes_nature_lacks_is_e0206() {
    // §6 rule 4, §6 rule 10: `let y = t; v(y)` with `t: Thermal` is refused as `v(t)` is, `E0206` at the access
    let src = "module Top() { signal t: Thermal; signal p: Electrical; }\nanalog for Top { let y = t; i(p, gnd) +> v(y); }";
    let diagnostics = Elab::run(src, "Top").expect_err("`Thermal` declares no `v`");
    assert_eq!(Elab::codes(&diagnostics), ["E0206"]);
    assert!(diagnostics[0].diagnostic.message.contains("declares no access `v`"), "{diagnostics:?}");
}

#[test]
fn parentheses_around_a_node_argument_lower_to_the_node() {
    // §1.5, §6 rule 4: `v((x))`, `v((y))` with `let y = x;`, `v((c0.sn))` and `v((q)) +> 1.0` lower to the IR the forms without the
    // parentheses lower to
    let child = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); }\n";
    let design = |statements: &str| format!("{child}module Top() {{ signal x: Electrical; let c0 = Dram(x); signal p: Electrical; signal q: Electrical; }}\nanalog for Top {{ {statements} }}");
    let wrapped = Elab::ir(&design("let y = x; i(p, gnd) +> v((x)) + v((y)) + v((c0.sn)); v((q)) +> 1.0;"), "Top").print_structural();
    let bare = Elab::ir(&design("i(p, gnd) +> v(x) + v(x) + v(c0.sn); v(q) +> 1.0;"), "Top").print_structural();
    assert_eq!(wrapped, bare);
}

#[test]
fn a_node_chosen_at_run_time_is_e0133_not_an_elaboration_refusal() {
    // §1.4.7: `let y = if v(x) > 0.5 { x } else { p };` is refused by the checker with `E0133` at the `if`, never with the
    // elaborator's `P0001` for a node argument that is not a name
    let src = "module Top() { signal x: Electrical; signal p: Electrical; }\nanalog for Top { let y = if v(x) > 0.5 { x } else { p }; i(p, gnd) +> v(y); }";
    let diagnostics = Elab::run(src, "Top").expect_err("a node chosen at run time");
    assert_eq!(Elab::codes(&diagnostics), ["E0133"], "{:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
    assert_eq!(&src[diagnostics[0].diagnostic.span().range()], "if v(x) > 0.5 { x } else { p }");
}

#[test]
fn a_node_chosen_at_a_probe_or_a_target_is_e0133_alone() {
    // §1.4.7: `v(if ..)` read, `v(if ..) +> ..` and `i(if ..) +> ..` are refused with `E0133` at the `if`, and no `P0001` follows
    for statement in ["i(p, gnd) +> v(if v(p) > 0.0 { x } else { p });", "v(if v(p) > 0.0 { x } else { p }) +> 1.0;", "i(if v(p) > 0.0 { x } else { p }) +> 1.0;"] {
        let src = format!("module Top() {{ signal x: Electrical; signal p: Electrical; }}\nanalog for Top {{ {statement} }}");
        let diagnostics = Elab::run(&src, "Top").expect_err("a node chosen at run time");
        assert_eq!(Elab::codes(&diagnostics), ["E0133"], "{statement}: {:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
    }
}

#[test]
fn a_node_chosen_at_elaboration_folds_to_the_node() {
    // §1.4.7, §8.1.2 (user decision of 2026-11-24): `if k > 0.5 { a } else { b }` names `a` where `k` is 1 and `b` where it is 0, at a
    // probe, a contribution target and a placement's connection; `k` is structural, one body per value
    let src = "module R(a: Electrical, b: Electrical) { }\nanalog for R { i(a, b) +> v(a, b); }\n\
               module Pick(a: Electrical, b: Electrical, level: Electrical, t: Electrical) { k: f64 = 1.0; let r = R(level, if k > 0.5 { a } else { b }); }\n\
               analog for Pick { potential(level) +> v(if k > 0.5 { a } else { b }); v(if k > 0.5 { t } else { level }) +> 0.3; }\n\
               module Top() { signal a: Electrical; signal b: Electrical; signal l1: Electrical; signal l2: Electrical; signal t: Electrical; let one = Pick(a, b, l1, t) { k: 1.0 }; let zero = Pick(a, b, l2, t) { k: 0.0 }; }";
    let ir = Elab::ir(src, "Top");
    let printed = ir.print_structural();
    let body = |key: &str| printed.split("\nbody ").find(|body| body.starts_with(&format!("\"Pick~k={key}\""))).unwrap_or_else(|| panic!("no body Pick~k={key}: {printed}")).to_string();
    let (one, zero) = (body("1.0"), body("0.0"));
    assert!(one.contains("probe(a, gnd)") && one.contains("child r: R  nodes (level, a)") && one.contains("branch b1 (t, gnd) potential"), "{one}");
    assert!(zero.contains("probe(b, gnd)") && zero.contains("child r: R  nodes (level, b)") && zero.contains("(level, gnd) potential"), "{zero}");
}

#[test]
fn a_node_chosen_at_elaboration_connects_through_a_template() {
    // §1.4.7, §8.4.5: `cell(a, if k > 0.5 { b } else { gnd })` through `C: Fn(Electrical, Electrical)` places the template's module
    // on `b` where `k` is 1 and on ground where it is 0
    let src = "module R(a: Electrical, b: Electrical) { }\nanalog for R { i(a, b) +> v(a, b); }\n\
               module H<C: Fn(Electrical, Electrical)>(a: Electrical) { cell: C; signal b: Electrical; k: f64 = 1.0; let c = cell(a, if k > 0.5 { b } else { gnd }); }\n\
               module Top() { signal x: Electrical; let one = H::<_>(x) { cell: R { }, k: 1.0 }; let zero = H::<_>(x) { cell: R { }, k: 0.0 }; }";
    let ir = Elab::ir(src, "Top");
    let bindings = |path: &str| ir.instance_at(path).map(|instance| format!("{:?}", instance.bindings())).unwrap_or_else(|| panic!("no instance {path}"));
    let own_b = format!("{:?}", ir.instance_at("Top.one").expect("one").bindings()[1]);
    assert!(bindings("Top.one.c").contains(&own_b), "one places R on its own b: {} {own_b}", bindings("Top.one.c"));
    assert!(bindings("Top.zero.c").contains("Ground"), "zero places R on ground: {}", bindings("Top.zero.c"));
}

#[test]
fn a_node_chosen_through_a_function_that_reads_the_run_is_e0133() {
    // §1.4.7: `if late() { x } else { p }` with `late()` reading `now()` does not fold at elaboration: the elaborator refuses it
    // with `E0133` at the `if`, the code the checker gives a choice that reads the run where it is written
    let src = "fn late() -> bool { now() > 1.0 }\nmodule Top() { signal x: Electrical; signal p: Electrical; }\nanalog for Top { i(p, gnd) +> v(if late() { x } else { p }); }";
    let diagnostics = Elab::run(src, "Top").expect_err("the choice is the run's");
    assert_eq!(Elab::codes(&diagnostics), ["E0133"], "{:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
    assert_eq!(&src[diagnostics[0].diagnostic.span().range()], "if late() { x } else { p }");
}

#[test]
fn a_node_chosen_through_lets_and_arm_statements_folds_to_the_node() {
    // §1.4.7: statements in either arm and the `let`s a choice reads fold at elaboration: with `sel` false each form names `p`,
    // and with `let c = k * 2.0` (`k` 1) the `if c > 0.5` names `x`
    for (analog, node) in [
        ("let y = if sel { let z = 1.0; x } else { p }; i(q, gnd) +> v(y);", "p"),
        ("let y = if sel { x } else { let z = 1.0; p }; i(q, gnd) +> v(y);", "p"),
        ("let y = match sel { true => { let z = 1.0; x }, false => p }; i(q, gnd) +> v(y);", "p"),
        ("let y = { let s = sel; if s { x } else { p } }; i(q, gnd) +> v(y);", "p"),
        ("let c = k * 2.0; let y = if c > 0.5 { x } else { p }; i(q, gnd) +> v(y);", "x"),
    ] {
        let src = format!("module Top() {{ signal x: Electrical; signal p: Electrical; signal q: Electrical; sel: bool = false; k: f64 = 1.0; }}\nanalog for Top {{ i(x, gnd) +> v(x); i(p, gnd) +> v(p); {analog} }}");
        let printed = Elab::ir(&src, "Top").print_structural();
        assert!(printed.contains(&format!("b2.flow +> direct probe({node}, gnd)")), "{analog}: {printed}");
    }
}

#[test]
fn a_node_of_a_child_connects_to_another_placement() {
    // §8.2.8, §8.1.10: `Th(c0.sn)` binds `Th`'s port to the storage node of `c0`, placed by name or through a template, and so
    // does `Th(if sel { c0.sn } else { t })` with `sel` true; the child that owns the node is placed first
    let modules = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd); }\nmodule Th(a: Electrical) { }\nanalog for Th { i(a, gnd) +> v(a, gnd); }\n";
    for (top, path) in [
        ("module Top() { signal x: Electrical; let c0 = Dram(x); let th = Th(c0.sn); }", "Top"),
        ("module Bank<C: Fn(Electrical)>(a: Electrical) { cell: C; let c0 = cell(a); let th = Th(c0.sn); }\nmodule Top() { signal x: Electrical; let b = Bank::<_>(x) { cell: Dram { } }; }", "Top.b"),
        ("module Bank<C: Fn(Electrical)>(a: Electrical) { cell: C; sel: bool = true; signal t: Electrical; let c0 = cell(a); let th = Th(if sel { c0.sn } else { t }); }\nmodule Top() { signal x: Electrical; let b = Bank::<_>(x) { cell: Dram { } }; }", "Top.b"),
    ] {
        let ir = Elab::ir(&format!("{modules}{top}"), "Top");
        let binding = |at: &str| ir.instance_at(at).map(|instance| format!("{:?}", instance.bindings())).unwrap_or_else(|| panic!("no instance {at}"));
        let storage = format!("{:?}", ir.instance_at(&format!("{path}.c0")).expect("c0").bindings()[1]);
        assert!(binding(&format!("{path}.th")).contains(&storage), "{top}: th is on c0.sn ({storage}): {}", binding(&format!("{path}.th")));
    }
}

#[test]
fn a_body_written_in_reverse_order_elaborates_as_written_forward() {
    // §8.2: the items of a body appear in any order; a body whose placements and `let`s are written in reverse gives every
    // instance the same body, params and nodes as the forward one
    let modules = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd); }\nmodule Th(a: Electrical) { g: f64 = 1.0; }\nanalog for Th { i(a, gnd) +> v(a, gnd) * g; }\n";
    let forward = Elab::ir(&format!("{modules}module Top() {{ k: f64 = 2.0; signal x: Electrical; let c0 = Dram(x); let g = k * 2.0; let th = Th(c0.sn) {{ g: g }}; }}"), "Top");
    let reverse = Elab::ir(&format!("{modules}module Top() {{ let th = Th(c0.sn) {{ g: g }}; let g = k * 2.0; let c0 = Dram(x); signal x: Electrical; k: f64 = 2.0; }}"), "Top");
    for path in ["Top.c0", "Top.th"] {
        let describe = |ir: &piperine_ir::Ir| {
            let instance = ir.instance_at(path).unwrap_or_else(|| panic!("no instance {path}"));
            let nodes: Vec<String> = instance.bindings().iter().map(|bound| match bound {
                piperine_ir::NodeRef::Node(id) => ir.node(*id).path().to_string(),
                other => format!("{other:?}"),
            }).collect();
            (instance.body().name().to_string(), instance.params().to_vec(), nodes)
        };
        assert_eq!(describe(&reverse), describe(&forward), "{path}");
    }
}
