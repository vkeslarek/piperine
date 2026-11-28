//! Scope: handle functions at elaboration (generics GN-11, GN-12, GN-29, part I §3.1.5): a call of a function with a
//! handle parameter is inlined where it stands, so no IR function exists for it and its body is written out over the
//! carrier or node the caller passed.


use piperine_ir::Ir;
use piperine_lang_fixtures::elaboration::SourceElaboration;

/// The IR of a design whose `analog` block of `Top` is `statement`, after `items`.
struct HandleDesigns;

impl HandleDesigns {
    /// The elaborated `Top`, which must pass `Ir::check`.
    fn analog(items: &str, statement: &str) -> Ir {
        SourceElaboration::ir(&format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; signal t: Thermal; }}\nanalog for Top {{ {statement} }}\n"), "Top")
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
    let found = SourceElaboration::run(&src, "Top").err().expect("refused");
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
        let printed = SourceElaboration::ir(&src, "Top").print_structural();
        assert!(printed.contains(probe), "{src}: {printed}");
    }
}

#[test]
fn a_contribution_through_a_let_bound_to_a_node_is_the_nodes() {
    // §6 rule 4, §11: `let y = x; v(y) +> 1.0;` lowers to the contribution `v(x) +> 1.0;` lowers to, and through a template the
    // potential of the reached node `c0.sn`
    let alias = SourceElaboration::ir("module Top() { signal x: Electrical; }\nanalog for Top { let y = x; v(y) +> 1.0; }", "Top").print_structural();
    let direct = SourceElaboration::ir("module Top() { signal x: Electrical; }\nanalog for Top { v(x) +> 1.0; }", "Top").print_structural();
    assert_eq!(alias, direct);
    let child = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); }\n";
    let template = SourceElaboration::ir(&format!("{child}module Bank<C: Fn(Electrical)>(a: Electrical) {{ cell: C; let c0 = cell(a); }}\nanalog<C> for Bank<C> {{ let y = c0.sn; v(y) +> 1.0; }}\nmodule Top() {{ signal x: Electrical; let bank = Bank::<_>(x) {{ cell: Dram {{ }} }}; }}"), "Top").print_structural();
    assert!(template.contains("branch b0 (c0.sn, gnd) potential") && template.contains("b0.potential +> direct 1.0"), "{template}");
}

#[test]
fn an_access_a_let_bound_nodes_nature_lacks_is_e0206() {
    // §6 rule 4, §6 rule 10: `let y = t; v(y)` with `t: Thermal` is refused as `v(t)` is, `E0206` at the access
    let src = "module Top() { signal t: Thermal; signal p: Electrical; }\nanalog for Top { let y = t; i(p, gnd) +> v(y); }";
    let diagnostics = SourceElaboration::run(src, "Top").expect_err("`Thermal` declares no `v`");
    assert_eq!(SourceElaboration::codes(&diagnostics), ["E0206"]);
    assert!(diagnostics[0].diagnostic.message.contains("declares no access `v`"), "{diagnostics:?}");
}

#[test]
fn parentheses_around_a_node_argument_lower_to_the_node() {
    // §1.5, §6 rule 4: `v((x))`, `v((y))` with `let y = x;`, `v((c0.sn))` and `v((q)) +> 1.0` lower to the IR the forms without the
    // parentheses lower to
    let child = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); }\n";
    let design = |statements: &str| format!("{child}module Top() {{ signal x: Electrical; let c0 = Dram(x); signal p: Electrical; signal q: Electrical; }}\nanalog for Top {{ {statements} }}");
    let wrapped = SourceElaboration::ir(&design("let y = x; i(p, gnd) +> v((x)) + v((y)) + v((c0.sn)); v((q)) +> 1.0;"), "Top").print_structural();
    let bare = SourceElaboration::ir(&design("i(p, gnd) +> v(x) + v(x) + v(c0.sn); v(q) +> 1.0;"), "Top").print_structural();
    assert_eq!(wrapped, bare);
}

#[test]
fn a_node_chosen_at_run_time_is_e0133_not_an_elaboration_refusal() {
    // §1.4.7: `let y = if v(x) > 0.5 { x } else { p };` is refused by the checker with `E0133` at the `if`, never with the
    // elaborator's `P0001` for a node argument that is not a name
    let src = "module Top() { signal x: Electrical; signal p: Electrical; }\nanalog for Top { let y = if v(x) > 0.5 { x } else { p }; i(p, gnd) +> v(y); }";
    let diagnostics = SourceElaboration::run(src, "Top").expect_err("a node chosen at run time");
    assert_eq!(SourceElaboration::codes(&diagnostics), ["E0133"], "{:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
    assert_eq!(&src[diagnostics[0].diagnostic.span().range()], "if v(x) > 0.5 { x } else { p }");
}

#[test]
fn a_node_chosen_at_a_probe_or_a_target_is_e0133_alone() {
    // §1.4.7: `v(if ..)` read, `v(if ..) +> ..` and `i(if ..) +> ..` are refused with `E0133` at the `if`, and no `P0001` follows
    for statement in ["i(p, gnd) +> v(if v(p) > 0.0 { x } else { p });", "v(if v(p) > 0.0 { x } else { p }) +> 1.0;", "i(if v(p) > 0.0 { x } else { p }) +> 1.0;"] {
        let src = format!("module Top() {{ signal x: Electrical; signal p: Electrical; }}\nanalog for Top {{ {statement} }}");
        let diagnostics = SourceElaboration::run(&src, "Top").expect_err("a node chosen at run time");
        assert_eq!(SourceElaboration::codes(&diagnostics), ["E0133"], "{statement}: {:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
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
    let ir = SourceElaboration::ir(src, "Top");
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
    let ir = SourceElaboration::ir(src, "Top");
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
    let diagnostics = SourceElaboration::run(src, "Top").expect_err("the choice is the run's");
    assert_eq!(SourceElaboration::codes(&diagnostics), ["E0133"], "{:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
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
        let printed = SourceElaboration::ir(&src, "Top").print_structural();
        assert!(printed.contains(&format!("b2.flow +> direct probe({node}, gnd)")), "{analog}: {printed}");
    }
}

#[test]
fn a_let_assigned_only_values_of_elaboration_folds_to_the_node_it_holds_there() {
    // §1.4.7 (coordinator ruling, finding 26): the statements before the choice run at elaboration, so with `sel` false and `k` 1
    // each `s` below holds the value written last where `if s` reads it, and the choice names `x` for true and `p` for false
    for (analog, node) in [
        ("let s = sel; s = !sel;", "x"),
        ("let s = sel; if k > 0.5 { s = !s; }", "x"),
        ("let s = sel; if k < 0.5 { s = !s; }", "p"),
        ("let s = sel; for j in 0..3 { s = !s; }", "x"),
        ("let s = sel; for j in 0..2 { s = !s; }", "p"),
        ("let s = sel; let w = v(x); s = !s;", "x"),
        ("let s = true; s = sel;", "p"),
        ("let t = !sel; let s = sel; s = t;", "x"),
    ] {
        let src = format!("module Top() {{ signal x: Electrical; signal p: Electrical; signal q: Electrical; sel: bool = false; k: f64 = 1.0; }}\nanalog for Top {{ i(x, gnd) +> v(x); i(p, gnd) +> v(p); {analog} let y = if s {{ x }} else {{ p }}; i(q, gnd) +> v(y); }}");
        let printed = SourceElaboration::ir(&src, "Top").print_structural();
        assert!(printed.contains(&format!("b2.flow +> direct probe({node}, gnd)")), "{analog}: {printed}");
    }
    // assigned from a probe: the checker's `E0133` naming the read, and nothing from the elaborator
    let src = "module Top() { signal x: Electrical; signal p: Electrical; sel: bool = false; }\nanalog for Top { let s = sel; s = v(x) > 0.5; let y = if s { x } else { p }; i(p, gnd) +> v(y); }";
    let diagnostics = SourceElaboration::run(src, "Top").expect_err("a node chosen at run time");
    assert_eq!(SourceElaboration::codes(&diagnostics), ["E0133"], "{:?}", diagnostics.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
    assert!(diagnostics[0].diagnostic.message.contains("the variable `s`, whose value depends on the run: it reads a probe"), "{}", diagnostics[0].diagnostic.message);
}

#[test]
fn a_choice_whose_condition_assigns_folds_and_ends() {
    // §1.4.7 (finding 28): an assignment inside a choice's condition folds at elaboration like one before it, and elaboration
    // ends: with `sel` false, `{ s = !s; s }` is true and names `x`; a condition block that loops on its own variable names `p`
    // once it ends; each form elaborates within a time bound, on a thread of its own
    for (analog, node) in [
        ("let s = sel; let y = if { s = !s; s } { x } else { p };", "x"),
        ("let s = sel; let y = if { s = !s; s = !s; s } { x } else { p };", "p"),
        ("let s = sel; let y = if { let t = !s; t } { x } else { p };", "x"),
        ("let s = sel; let y = match { s = !s; s } { true => x, false => p };", "x"),
        ("let n = 0; let y = if { while n < 3 { n = n + 1; } n == 3 } { x } else { p };", "x"),
        ("let s = sel; s = !s; let y = if { s = !s; s } { x } else { p };", "p"),
    ] {
        let src = format!("module Top() {{ signal x: Electrical; signal p: Electrical; signal q: Electrical; sel: bool = false; }}\nanalog for Top {{ i(x, gnd) +> v(x); i(p, gnd) +> v(p); {analog} i(q, gnd) +> v(y); }}");
        let printed = Bounded::elaborate(src.clone()).unwrap_or_else(|| panic!("{analog}: elaboration did not end within the bound"));
        assert!(printed.contains(&format!("b2.flow +> direct probe({node}, gnd)")), "{analog}: {printed}");
    }
    // an assignment of the run inside the condition does not fold: `E0133` alone, naming the read, within the bound too
    let src = "module Top() { signal x: Electrical; signal p: Electrical; sel: bool = false; }\nanalog for Top { let s = sel; let y = if { s = v(x) > 0.5; s } { x } else { p }; i(p, gnd) +> v(y); }".to_string();
    let refused = Bounded::refusal(src).expect("elaboration ends within the bound");
    assert_eq!(refused.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0133"], "{refused:?}");
    assert!(refused[0].1.contains("a probe"), "{refused:?}");
}

/// Elaboration on a thread of its own, given a time bound: a fold that never ends fails the test instead of hanging it.
struct Bounded;

impl Bounded {
    /// The structural print of `Top`, or `None` when elaboration has not ended after 20 s.
    fn elaborate(src: String) -> Option<String> {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(SourceElaboration::ir(&src, "Top").print_structural());
        });
        receiver.recv_timeout(std::time::Duration::from_secs(20)).ok()
    }

    /// The codes and messages with which elaboration refuses `Top`, or `None` when it has not ended after 20 s.
    fn refusal(src: String) -> Option<Vec<(String, String)>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let refused = SourceElaboration::run(&src, "Top").err().unwrap_or_default();
            let _ = sender.send(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect());
        });
        receiver.recv_timeout(std::time::Duration::from_secs(20)).ok()
    }
}

#[test]
fn a_node_of_a_child_connects_to_another_placement() {
    // §8.2.8, §8.1.10: `Th(c0.sn)` binds `Th`'s port to the storage node of `c0`, placed by name or through a template, and so
    // does `Th(if sel { c0.sn } else { t })` with `sel` true
    let modules = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd); }\nmodule Th(a: Electrical) { }\nanalog for Th { i(a, gnd) +> v(a, gnd); }\n";
    for (top, path) in [
        ("module Top() { signal x: Electrical; let c0 = Dram(x); let th = Th(c0.sn); }", "Top"),
        ("module Bank<C: Fn(Electrical)>(a: Electrical) { cell: C; let c0 = cell(a); let th = Th(c0.sn); }\nmodule Top() { signal x: Electrical; let b = Bank::<_>(x) { cell: Dram { } }; }", "Top.b"),
        ("module Bank<C: Fn(Electrical)>(a: Electrical) { cell: C; sel: bool = true; signal t: Electrical; let c0 = cell(a); let th = Th(if sel { c0.sn } else { t }); }\nmodule Top() { signal x: Electrical; let b = Bank::<_>(x) { cell: Dram { } }; }", "Top.b"),
    ] {
        let ir = SourceElaboration::ir(&format!("{modules}{top}"), "Top");
        let binding = |at: &str| ir.instance_at(at).map(|instance| format!("{:?}", instance.bindings())).unwrap_or_else(|| panic!("no instance {at}"));
        let storage = format!("{:?}", ir.instance_at(&format!("{path}.c0")).expect("c0").bindings()[1]);
        assert!(binding(&format!("{path}.th")).contains(&storage), "{top}: th is on c0.sn ({storage}): {}", binding(&format!("{path}.th")));
    }
}

#[test]
fn a_placement_binds_the_node_of_a_child_written_after_it() {
    // §8.2, §8.2.14 (finding 24): `let q = D2(p.sn); let p = D2(x);` binds `q`'s port to `p`'s storage node as the forward order does
    let modules = "module D2(a: Electrical) { pub signal sn: Electrical; }\nanalog for D2 { i(a, sn) +> v(a, sn); i(sn, gnd) +> v(sn, gnd); }\n";
    let forward = SourceElaboration::ir(&format!("{modules}module Top() {{ signal x: Electrical; let p = D2(x); let q = D2(p.sn); }}"), "Top");
    let reverse = SourceElaboration::ir(&format!("{modules}module Top() {{ signal x: Electrical; let q = D2(p.sn); let p = D2(x); }}"), "Top");
    for ir in [&forward, &reverse] {
        assert_eq!(SourceElaboration::node_paths(ir, "Top.q"), ["Top.p.sn", "Top.q.sn"]);
        assert_eq!(SourceElaboration::node_paths(ir, "Top.p"), ["Top.x", "Top.p.sn"]);
    }
}

#[test]
fn a_cycle_of_node_connections_binds_each_port_to_the_node_it_names() {
    // §8.2.14, §8.3.5 (finding 25): a connection is hardware, so a cross-coupled pair, an instance wired to its own node and a
    // ring of three each bind every port to the storage node it names
    let modules = "module D2(a: Electrical) { pub signal sn: Electrical; }\nanalog for D2 { i(a, sn) +> v(a, sn); i(sn, gnd) +> v(sn, gnd); }\n";
    let pair = SourceElaboration::ir(&format!("{modules}module Top() {{ let p = D2(q.sn); let q = D2(p.sn); }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&pair, "Top.p"), ["Top.q.sn", "Top.p.sn"]);
    assert_eq!(SourceElaboration::node_paths(&pair, "Top.q"), ["Top.p.sn", "Top.q.sn"]);
    let own = SourceElaboration::ir(&format!("{modules}module Top() {{ let p = D2(p.sn); }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&own, "Top.p"), ["Top.p.sn", "Top.p.sn"]);
    let ring = SourceElaboration::ir(&format!("{modules}module Top() {{ let p = D2(r.sn); let q = D2(p.sn); let r = D2(q.sn); }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&ring, "Top.p"), ["Top.r.sn", "Top.p.sn"]);
    assert_eq!(SourceElaboration::node_paths(&ring, "Top.q"), ["Top.p.sn", "Top.q.sn"]);
    assert_eq!(SourceElaboration::node_paths(&ring, "Top.r"), ["Top.q.sn", "Top.r.sn"]);
    // through a child of a child (`b.c0.sn`), written before the bank and fed back into the bank itself
    let banks = format!("{modules}module Bank(a: Electrical) {{ pub let c0 = D2(a); }}\n");
    let later = SourceElaboration::ir(&format!("{banks}module Top() {{ signal x: Electrical; let q = D2(b.c0.sn); let b = Bank(x); }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&later, "Top.q"), ["Top.b.c0.sn", "Top.q.sn"]);
    let fed_back = SourceElaboration::ir(&format!("{banks}module Top() {{ let b = Bank(b.c0.sn); }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&fed_back, "Top.b.c0"), ["Top.b.c0.sn", "Top.b.c0.sn"]);
}

#[test]
fn a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port() {
    // §8.2.14, §8.3.5 (finding 27): through a template two levels deep and a generation, the cross pair binds
    // each bank's port to the other's cell node and the self-connection binds the bank's port to its own cell's node
    let modules = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, sn) +> v(a, sn); i(sn, gnd) +> v(sn, gnd); }\n\
                   module Bank<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let c0 = cell(a); }\n\
                   module Row<C: Fn(Electrical)>(a: Electrical) { cell: C; for j in 0..2 { pub let cells = cell(a); } }\n\
                   module Plain(a: Electrical) { pub let c0 = Dram(a); }\nmodule Deep(a: Electrical) { pub let mid = Plain(a); }\n";
    let pair = SourceElaboration::ir(&format!("{modules}module Top() {{ let p = Bank::<_>(q.c0.sn) {{ cell: Dram {{ }} }}; let q = Bank::<_>(p.c0.sn) {{ cell: Dram {{ }} }}; }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&pair, "Top.p.c0"), ["Top.q.c0.sn", "Top.p.c0.sn"]);
    assert_eq!(SourceElaboration::node_paths(&pair, "Top.q.c0"), ["Top.p.c0.sn", "Top.q.c0.sn"]);
    let own = SourceElaboration::ir(&format!("{modules}module Top() {{ let p = Bank::<_>(p.c0.sn) {{ cell: Dram {{ }} }}; }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&own, "Top.p.c0"), ["Top.p.c0.sn", "Top.p.c0.sn"]);
    let row = SourceElaboration::ir(&format!("{modules}module Top() {{ let p = Row::<_>(q.cells[1].sn) {{ cell: Dram {{ }} }}; let q = Row::<_>(p.cells[0].sn) {{ cell: Dram {{ }} }}; }}"), "Top");
    assert_eq!(SourceElaboration::node_paths(&row, "Top.p.cells[0]"), ["Top.q.cells[1].sn", "Top.p.cells[0].sn"]);
    assert_eq!(SourceElaboration::node_paths(&row, "Top.q.cells[1]"), ["Top.p.cells[0].sn", "Top.q.cells[1].sn"]);
}

#[test]
fn a_body_written_in_reverse_order_elaborates_as_written_forward() {
    // §8.2: the items of a body appear in any order; a body whose placements and `let`s are written in reverse gives every
    // instance the same body, params and nodes as the forward one
    let modules = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd); }\nmodule Th(a: Electrical) { g: f64 = 1.0; }\nanalog for Th { i(a, gnd) +> v(a, gnd) * g; }\n";
    let forward = SourceElaboration::ir(&format!("{modules}module Top() {{ k: f64 = 2.0; signal x: Electrical; let c0 = Dram(x); let g = k * 2.0; let th = Th(c0.sn) {{ g: g }}; }}"), "Top");
    let reverse = SourceElaboration::ir(&format!("{modules}module Top() {{ let th = Th(c0.sn) {{ g: g }}; let g = k * 2.0; let c0 = Dram(x); signal x: Electrical; k: f64 = 2.0; }}"), "Top");
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
