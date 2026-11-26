//! Scope: `inst.name` through children of children at any depth, at elaboration (feature `nested-reach`, part III §8.2.8, §8.4.8):
//! the node, the carrier and the `let` over the solve of a cell three and four levels below the reader are reached by name, through
//! templates and in generation, in any text order, and a cycle of node connections through such a reach binds each port.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;
#[path = "support/nested_reach.rs"]
mod nested;

use nested::{MIXED_LETS, MODELS, READS};
use piperine_ir::Ir;
use piperine_lang::elab::{Elaborator, Overrides};
use support::Elab;

/// The top that reads `reach` below the placement `placement`, with `reads` as its `analog` block.
fn top(placement: &str, reads: &str) -> String {
    format!("module Top() {{ signal a: Electrical; signal o: Electrical; {placement} }}\nanalog for Top {{ potential(a) +> 1.0; {reads} }}\n")
}

#[test]
fn a_node_is_reached_three_and_four_levels_below_the_reader() {
    // §8.2.8: `R.sn` names the storage node of the cell `R` places, whatever the depth of the chain: the reader's reached node
    // takes the name of the path and is the cell's own node, one node with every reached node on the way
    for (placement, reach, form) in READS {
        let ir = Elab::ir(&format!("{MODELS}{}", top(placement, &format!("potential(o) +> v({reach}.sn);"))), "Top");
        let reader = ir.instance_at("Top").expect("the top");
        let at = reader.body().nodes().iter().position(|node| node.name == format!("{reach}.sn")).unwrap_or_else(|| panic!("{form}: `Top` reaches `{reach}.sn`"));
        assert_eq!(reader.node(at).expect("a bound node").path(), format!("Top.{reach}.sn"), "{form}");
        assert!(ir.instance_at(&format!("Top.{reach}")).is_some(), "{form}: the cell is placed at the path the read names");
    }
}

#[test]
fn a_param_and_a_let_over_the_params_read_the_value_the_placement_gives_three_and_four_levels_below() {
    // §8.2.8: `R.k` is the param of the cell the placement writes (`1.0 + j` in generation: 2.0 for `cells[1]`), and `R.twice` the
    // `let` over it: the reader lowers the value of the placement, not the module's default 1.0, for each (§8.2.25)
    for (placement, reach, k, form) in [
        ("let p = Level3(a);", "p.mid.c0", 1.0, "3 by name, the default"),
        ("let p = Block { cell: Cell { k: 3.0 } }(a);", "p.bank.c0", 3.0, "3 through templates"),
        ("let p = Wing { cell: Cell { k: 4.0 } }(a);", "p.block.bank.c0", 4.0, "4 through templates"),
        ("let p = GWing::<_>(a) { cell: Cell { k: 6.0 } };", "p.block.bank.c0", 6.0, "4 through generic templates"),
        ("let p = Grid { cell: Cell { k: 5.0 } }(a);", "p.row.cells[1]", 5.0, "3 through a template in generation"),
        ("let p = GPlane::<_>(a) { cell: Cell { k: 7.0 } };", "p.grid.row.cells[1]", 7.0, "4 through generic templates in generation"),
        ("let p = Tower(a);", "p.rows.cells[1]", 2.0, "3 by name in generation, `1.0 + j`"),
        ("let p = Tall(a);", "p.tower.rows.cells[1]", 2.0, "4 by name in generation, `1.0 + j`"),
    ] {
        let printed = |item: &str| Elab::ir(&format!("{MODELS}{}", top(placement, &format!("potential(o) +> {reach}.{item};"))), "Top").print_structural();
        // the top's own drive `potential(a) +> 1.0` prints `direct 1.0` once: the read adds one more only at `k = 1.0`
        let own = usize::from(k == 1.0);
        assert_eq!(printed("k").matches(&format!("direct {k:?}")).count(), own + 1, "{form}: `{reach}.k` lowers to {k:?}: {}", printed("k"));
        assert!(printed("twice").contains(&format!("direct ({k:?} * 2.0)")), "{form}: `{reach}.twice` lowers to `{k:?} * 2.0`: {}", printed("twice"));
    }
}

#[test]
fn a_let_over_the_solve_reaches_the_node_it_reads_three_and_four_levels_below_the_reader() {
    // §8.2.8, §8.2.25 (GAPS L81): `R.vs` is `v(sn)` evaluated in the reader's context, so the reader reaches the cell's `sn`
    // though it names only `vs`
    for (placement, reach, form) in READS {
        let ir = Elab::ir(&format!("{MODELS}{}", top(placement, &format!("potential(o) +> {reach}.vs;"))), "Top");
        let reader = ir.instance_at("Top").expect("the top");
        let at = reader.body().nodes().iter().position(|node| node.name == format!("{reach}.sn")).unwrap_or_else(|| panic!("{form}: `Top` reaches `{reach}.sn` for `vs`"));
        assert_eq!(reader.node(at).expect("a bound node").path(), format!("Top.{reach}.sn"), "{form}");
    }
}

#[test]
fn a_carrier_is_reached_three_and_four_levels_below_the_reader() {
    // §8.2.8 (GAPS L81): `R.q` names the carrier of the cell `R` places: the reader's reached carrier is bound to the cell's net
    for (placement, reach, form) in READS {
        let ir = Elab::ir(&format!("{MODELS}{}", top(placement, &format!("potential(o) +> {reach}.q;"))), "Top");
        let reader = ir.instance_at("Top").expect("the top");
        let at = reader.body().carriers().iter().position(|carrier| carrier.name == format!("{reach}.q")).unwrap_or_else(|| panic!("{form}: `Top` reaches `{reach}.q`"));
        assert_eq!(reader.net(at).expect("a bound net").path(), format!("Top.{reach}.q"), "{form}");
    }
}

#[test]
fn a_reach_is_the_same_wherever_the_models_and_the_top_are_written() {
    // §8.2: the items of a design appear in any order: the top written before the models it places binds the same nodes and nets
    for (placement, reach, form) in READS {
        let reads = format!("potential(o) +> v({reach}.sn) + {reach}.vs + {reach}.q + {reach}.twice;");
        let forward = Elab::ir(&format!("{MODELS}{}", top(placement, &reads)), "Top");
        let reverse = Elab::ir(&format!("{}{MODELS}", top(placement, &reads)), "Top");
        let names = |ir: &Ir| {
            let reader = ir.instance_at("Top").expect("the top");
            let nodes: Vec<String> = reader.body().nodes().iter().enumerate().map(|(at, node)| format!("{}={}", node.name, reader.node(at).map_or_else(String::new, |n| n.path().to_string()))).collect();
            let nets: Vec<String> = reader.body().carriers().iter().enumerate().map(|(at, carrier)| format!("{}={}", carrier.name, reader.net(at).map_or_else(String::new, |n| n.path().to_string()))).collect();
            (nodes, nets)
        };
        assert_eq!(names(&forward), names(&reverse), "{form}");
    }
}

#[test]
fn a_cycle_of_node_connections_three_and_four_levels_deep_binds_each_port() {
    // §8.2.14, §8.3.5 (GAPS L82): a cross pair and a self-connection through `p.mid.c0.sn`, `p.top.mid.c0.sn` and the same through
    // templates and a generation bind each wrapper's port to the cell node it names; the reverse text order binds the same
    for (pair, own, path, reach) in [
        ("let p = Level3(q.mid.c0.sn); let q = Level3(p.mid.c0.sn);", "let p = Level3(p.mid.c0.sn);", "mid.c0", "mid.c0.sn"),
        ("let p = Level4(q.top.mid.c0.sn); let q = Level4(p.top.mid.c0.sn);", "let p = Level4(p.top.mid.c0.sn);", "top.mid.c0", "top.mid.c0.sn"),
        ("let p = Block { cell: Cell { } }(q.bank.c0.sn); let q = Block { cell: Cell { } }(p.bank.c0.sn);", "let p = Block { cell: Cell { } }(p.bank.c0.sn);", "bank.c0", "bank.c0.sn"),
        ("let p = Wing { cell: Cell { } }(q.block.bank.c0.sn); let q = Wing { cell: Cell { } }(p.block.bank.c0.sn);", "let p = Wing { cell: Cell { } }(p.block.bank.c0.sn);", "block.bank.c0", "block.bank.c0.sn"),
        ("let p = GBlock::<_>(q.bank.c0.sn) { cell: Cell { } }; let q = GBlock::<_>(p.bank.c0.sn) { cell: Cell { } };", "let p = GBlock::<_>(p.bank.c0.sn) { cell: Cell { } };", "bank.c0", "bank.c0.sn"),
        ("let p = GWing::<_>(q.block.bank.c0.sn) { cell: Cell { } }; let q = GWing::<_>(p.block.bank.c0.sn) { cell: Cell { } };", "let p = GWing::<_>(p.block.bank.c0.sn) { cell: Cell { } };", "block.bank.c0", "block.bank.c0.sn"),
        ("let p = GPlane::<_>(q.grid.row.cells[1].sn) { cell: Cell { } }; let q = GPlane::<_>(p.grid.row.cells[1].sn) { cell: Cell { } };", "let p = GPlane::<_>(p.grid.row.cells[1].sn) { cell: Cell { } };", "grid.row.cells[1]", "grid.row.cells[1].sn"),
        ("let p = Grid { cell: Cell { } }(q.row.cells[1].sn); let q = Grid { cell: Cell { } }(p.row.cells[1].sn);", "let p = Grid { cell: Cell { } }(p.row.cells[1].sn);", "row.cells[1]", "row.cells[1].sn"),
        ("let p = Plane { cell: Cell { } }(q.grid.row.cells[1].sn); let q = Plane { cell: Cell { } }(p.grid.row.cells[1].sn);", "let p = Plane { cell: Cell { } }(p.grid.row.cells[1].sn);", "grid.row.cells[1]", "grid.row.cells[1].sn"),
    ] {
        for text in [format!("{MODELS}module Top() {{ {pair} }}"), format!("module Top() {{ {pair} }}{MODELS}")] {
            let ir = Elab::ir(&text, "Top");
            assert_eq!(Elab::node_paths(&ir, &format!("Top.p.{path}")), [format!("Top.q.{reach}"), format!("Top.p.{reach}")], "{pair}");
            assert_eq!(Elab::node_paths(&ir, &format!("Top.q.{path}")), [format!("Top.p.{reach}"), format!("Top.q.{reach}")], "{pair}");
        }
        let ir = Elab::ir(&format!("{MODELS}module Top() {{ {own} }}"), "Top");
        assert_eq!(Elab::node_paths(&ir, &format!("Top.p.{path}")), [format!("Top.p.{reach}"), format!("Top.p.{reach}")], "{own}");
    }
}

#[test]
fn a_missing_or_private_item_is_e0343_three_and_four_levels_below_the_reader() {
    // §8.2.8: an item the cell lacks or keeps private is `E0343` at any depth, from the checker where the path names modules and
    // at elaboration where a template does, never a silent reach
    for (placement, reach, form) in READS {
        for item in ["missing", "hidden"] {
            let src = format!("{MODELS}{}", top(placement, &format!("potential(o) +> {reach}.{item};")));
            let design = Elab::design(&src);
            let mut codes: Vec<String> = design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect();
            if codes.is_empty() {
                codes = Elab::codes(&Elab::run(&src, "Top").expect_err("out of reach"));
            }
            assert!(codes.iter().all(|code| code == "E0343") && !codes.is_empty(), "{form}: {item}: {codes:?}");
        }
    }
}

#[test]
fn a_drive_of_a_resolved_carrier_of_a_cell_three_levels_down_is_held_beside_the_cells_own() {
    // §5.2.2, §8.2.8: `p.mid.c0.line = false` holds a drive of the cell's resolved net: the net's resolver takes two drives, the
    // top's (`Top#0`) and the cell's own (`Top.p.mid.c0#0`)
    let src = "nature AndWire { value: bool }\nimpl Resolve<bool> for AndWire { fn resolve<const N: usize>(d: [bool; N]) -> bool { d.iter().fold(true, |acc, x| acc && x) } }\n\
               module Pin() { pub signal line: AndWire; line = true; }\nmodule Level2() { pub let c0 = Pin(); }\nmodule Level3() { pub let mid = Level2(); }\n\
               module Top() { let p = Level3(); p.mid.c0.line = false; }";
    let printed = Elab::ir(src, "Top").print_structural();
    let net = printed.lines().find(|line| line.starts_with("net Top.p.mid.c0.line:")).expect("the cell's net");
    assert!(net.contains("resolved by Top.p.mid.c0.line$resolve of bool drives (Top.p.mid.c0.line$drive0, Top.p.mid.c0.line$drive1)"), "{net}");
    assert!(net.contains("held by (Top#0, Top.p.mid.c0#0)"), "{net}");
}

#[test]
fn a_reach_refused_for_its_cause_reports_that_cause_once() {
    // GAPS L94, L95: a read through a module that declares a node in generation, and through a hoisted module that holds a placement,
    // is one `P0001` naming the cause, with no second diagnostic after it
    for (src, cause) in [
        (
            format!("{MODELS}module GenNode(a: Electrical) {{ for j in 0..2 {{ pub signal s: Electrical; }} pub let mid = Level2(a); }}\n{}", top("let p = GenNode(a);", "potential(o) +> v(p.mid.c0.sn);")),
            "a node declared in generation of a module a parent reads through",
        ),
        (
            format!("{MODELS}module GenNode(a: Electrical) {{ for j in 0..2 {{ pub signal s: Electrical; }} pub let c0 = Cell(a); }}\n{}", top("let p = GenNode(a);", "potential(o) +> v(p.c0.sn);")),
            "a node declared in generation of a module a parent reads through",
        ),
        (
            format!("{MODELS}module HW(x: f64) -> f64 {{ signal n: Electrical; pub let w = Level2(n); x }}\n{}", top("let h = HW(1.0);", "potential(o) +> v(h.w.c0.sn);")),
            "a hoisted module that holds a placement of a module that is not hoisted",
        ),
    ] {
        let found = Elab::run(&src, "Top").expect_err("the read is refused");
        let messages: Vec<String> = found.iter().map(|d| format!("{} {}", d.diagnostic.code, d.diagnostic.message)).collect();
        assert_eq!(messages.len(), 1, "{messages:?}");
        assert!(messages[0].starts_with("P0001") && messages[0].contains(cause), "{messages:?}");
    }
}

#[test]
fn a_let_over_the_solve_three_and_four_levels_below_the_reader_is_elaborated_in_one_pass() {
    // §8.2.25 (verification finding 7): `R.vs` is `v(sn)` read in the reader's context, so the node `sn` it names is a demand of
    // each hop, known from the text before the first pass; so are the names a `let` reads through another `let` of its module
    // (`vs2 = vs + v(a)`: the node `sn` through `vs`, and the port `a`). A form whose hops the text names takes one pass, never one
    // for each level
    let models = MODELS.replace("pub let vs = v(sn);", "pub let vs = v(sn); pub let vs2 = vs + v(a);");
    for (placement, reach, form) in READS.iter().filter(|(placement, _, _)| !placement.contains("::<_>")) {
        for item in ["vs", "vs2"] {
            let design = support::Elab::design(&format!("{models}{}", top(placement, &format!("potential(o) +> {reach}.{item};"))));
            let (result, statistics) = Elaborator::new(&design).run_with_statistics("Top", &Overrides::none());
            let ir = result.unwrap_or_else(|d| panic!("{form}: {item}: {d:?}"));
            let reader = ir.instance_at("Top").expect("the top");
            assert!(reader.body().nodes().iter().any(|node| node.name == format!("{reach}.sn")), "{form}: {item}: `Top` reaches `{reach}.sn`");
            assert_eq!(statistics.passes, 1, "{form}: `{reach}.{item}` is elaborated in one pass");
        }
    }
}

#[test]
fn every_read_whose_hops_the_text_names_is_elaborated_in_one_pass() {
    // verification finding 8: the demands of a read through children of children are known from the text before the first pass
    // (`reach_seed.rs`), so every item of the cell (a node, a `let` over the solve, a carrier, a param, a `let` over the params) is
    // one pass in every form, by name, through templates, a template placed through a template and in generation, and so is a
    // cycle of node connections. The one rerun left is the documented fallback: a `let` of a cell placed through a type parameter
    // (`GBank<C>`'s `cell(a)`), whose module the text does not name, so the names that `let` reads are found by running, one pass
    // for each level below the reader
    let elaborate = |src: &str| {
        let design = support::Elab::design(src);
        let (result, statistics) = Elaborator::new(&design).run_with_statistics("Top", &Overrides::none());
        assert!(result.is_ok(), "{src}: {:?}", result.err());
        statistics.passes
    };
    for (placement, reach, form) in READS {
        let through_type_parameter = placement.contains("::<_>");
        let depth = if form.starts_with('4') { 4 } else { 3 };
        for (item, read) in [("sn", format!("v({reach}.sn)")), ("vs", format!("{reach}.vs")), ("q", format!("{reach}.q")), ("k", format!("{reach}.k")), ("twice", format!("{reach}.twice"))] {
            let expected = if through_type_parameter && item == "vs" { depth } else { 1 };
            let passes = elaborate(&format!("{MODELS}{}", top(placement, &format!("potential(o) +> {read};"))));
            assert_eq!(passes, expected, "{form}: `{read}`");
        }
    }
    for cycle in [
        "let p = Level3(q.mid.c0.sn); let q = Level3(p.mid.c0.sn);",
        "let p = Level4(p.top.mid.c0.sn);",
        "let p = Holder { make: Bank { cell: Cell { } } }(q.b.c0.sn); let q = Holder { make: Bank { cell: Cell { } } }(p.b.c0.sn);",
        "let p = Plane { cell: Cell { } }(q.grid.row.cells[1].sn); let q = Plane { cell: Cell { } }(p.grid.row.cells[1].sn);",
        "let p = Tall(q.tower.rows.cells[1].sn); let q = Tall(p.tower.rows.cells[1].sn);",
        "let p = GPlane::<_>(q.grid.row.cells[1].sn) { cell: Cell { } }; let q = GPlane::<_>(p.grid.row.cells[1].sn) { cell: Cell { } };",
    ] {
        assert_eq!(elaborate(&format!("{MODELS}module Top() {{ {cycle} }}")), 1, "{cycle}");
    }
}


#[test]
fn a_refusal_of_another_cause_hides_no_refusal_of_a_read() {
    // verification finding 9: a read refused for its own cause (`v(r.h.n)`, a node of a hoisted module read through a child) says so
    // though the run refused something else before it, a sibling read through a module that declares a node in generation (GAPS L94)
    // or a hoisted module that holds a placement (L95): only a read through the refused module itself is silent after its cause
    let models = format!(
        "{MODELS}module GenNode(a: Electrical) {{ for j in 0..2 {{ pub signal s: Electrical; }} pub let mid = Level2(a); }}\n\
         module HW(x: f64) -> f64 {{ signal n: Electrical; pub let w = Level2(n); x }}\n\
         module HN(x: f64) -> f64 {{ pub signal n: Electrical; x }}\nmodule MidH(a: Electrical) {{ pub let h = HN(1.0); }}\nmodule TopH(a: Electrical) {{ pub let m = MidH(a); }}\n"
    );
    let own = "a name of a child that is not one of its nodes, as a node";
    for (placement, reads, earlier) in [
        ("let g = GenNode(a); let r = MidH(a);", "v(g.mid.c0.sn) + v(r.h.n)", "a node declared in generation of a module a parent reads through"),
        ("let g = GenNode(a); let r = TopH(a);", "v(g.mid.c0.sn) + v(r.m.h.n)", "a node declared in generation of a module a parent reads through"),
        ("let h = HW(1.0); let r = MidH(a);", "v(h.w.c0.sn) + v(r.h.n)", "a hoisted module that holds a placement of a module that is not hoisted"),
    ] {
        let found = Elab::run(&format!("{models}{}", top(placement, &format!("potential(o) +> {reads};"))), "Top").expect_err("both reads are refused");
        let messages: Vec<String> = found.iter().map(|d| format!("{} {}", d.diagnostic.code, d.diagnostic.message)).collect();
        assert_eq!(messages.len(), 2, "{reads}: {messages:?}");
        assert!(messages[0].starts_with("P0001") && messages[0].contains(earlier), "{reads}: the earlier cause first: {messages:?}");
        assert!(messages[1].starts_with("P0001") && messages[1].contains(own), "{reads}: the read's own refusal: {messages:?}");
    }
}

#[test]
fn a_let_over_the_solve_and_a_param_reads_the_value_the_placement_gives_one_three_and_four_levels_below() {
    // §8.2.25 (verification finding 10): a `let` of the cell that reads a param and the solve (`deep = k * vs`, `gated = if k > 0.5
    // { vs } else { 0.0 }`, `gi = g * v(sn)`) is lowered in the reader's context with each param at the value the placement gives,
    // else the default (`g = 0.5`), never refused: one level below (`p`), three and four by name, through templates and in
    // generation, and through a type parameter. A form whose hops the text names takes one pass (C25: the type parameter, one pass
    // for each level below the reader)
    let models = MODELS.replace("pub let vs = v(sn);", MIXED_LETS);
    for (placement, reach, k, g, passes) in [
        ("let p = Cell(a) { k: 2.0, g: 3.0 };", "p", 2.0, 3.0, 1),
        ("let p = Level3(a);", "p.mid.c0", 1.0, 0.5, 1),
        ("let p = Level4(a);", "p.top.mid.c0", 1.0, 0.5, 1),
        ("let p = Block { cell: Cell { k: 3.0, g: 2.0 } }(a);", "p.bank.c0", 3.0, 2.0, 1),
        ("let p = Wing { cell: Cell { k: 4.0, g: 3.0 } }(a);", "p.block.bank.c0", 4.0, 3.0, 1),
        ("let p = Grid { cell: Cell { k: 0.25 } }(a);", "p.row.cells[1]", 0.25, 0.5, 1),
        ("let p = Tower(a);", "p.rows.cells[1]", 2.0, 0.5, 1),
        ("let p = Tall(a);", "p.tower.rows.cells[1]", 2.0, 0.5, 1),
        ("let p = GPlane::<_>(a) { cell: Cell { k: 7.0, g: 1.5 } };", "p.grid.row.cells[1]", 7.0, 1.5, 4),
    ] {
        for (item, lowered) in [("deep", format!("({k:?} * probe(")), ("gated", format!("if ({k:?} > 0.5) {{ probe(")), ("gi", format!("({g:?} * probe("))] {
            let design = support::Elab::design(&format!("{models}{}", top(placement, &format!("potential(o) +> {reach}.{item};"))));
            let (result, statistics) = Elaborator::new(&design).run_with_statistics("Top", &Overrides::none());
            let printed = result.unwrap_or_else(|d| panic!("{reach}.{item}: {d:?}")).print_structural();
            let read = printed.lines().find(|line| line.contains("b1.potential +> direct")).unwrap_or_else(|| panic!("{reach}.{item}: the read of `o`: {printed}"));
            assert!(read.contains(&lowered) && read.contains(&format!("{reach}.sn")), "{reach}.{item} lowers `{lowered}` over the cell's node: {read}");
            assert_eq!(statistics.passes, passes, "{reach}.{item}");
        }
    }
}

#[test]
fn a_given_value_over_a_param_of_the_reader_is_read_in_place_one_and_three_levels_below() {
    // §8.2.25 (verification finding 11): a placement that gives a param a value over a param of the reader that is not a key
    // (`Cell(a) { k: m * 2.0 }`, the reader's `m` a param the host may set) keeps its reach: `v(R.sn)` and `R.vs` read the cell's
    // node, and `R.deep` (`k * vs`) reads `k` as the reader's own expression, lowered where the value is written: one level below,
    // three by name with `m` handed down through each wrapper's param, and three in generation. One pass each
    let models = format!(
        "{}module RL2(a: Electrical) {{ g2: f64 = 1.0; pub let c0 = Cell(a) {{ k: g2 * 2.0 }}; }}\nmodule RL3(a: Electrical) {{ m3: f64 = 1.0; pub let mid = RL2(a) {{ g2: m3 }}; }}\n\
         module RRows(a: Electrical) {{ m3: f64 = 1.0; for j in 0..2 {{ pub let cells = Cell(a) {{ k: m3 * 2.0 }}; }} }}\nmodule RTower(a: Electrical) {{ m4: f64 = 1.0; pub let rows = RRows(a) {{ m3: m4 * 0.5 }}; }}\n",
        MODELS.replace("pub let vs = v(sn);", MIXED_LETS)
    );
    for (placement, reach, k) in [
        ("let c = Cell(a) { k: m * 2.0 };", "c", "(m * 2.0)"),
        ("let p = RL3(a) { m3: m };", "p.mid.c0", "(m * 2.0)"),
        ("let p = RTower(a) { m4: m };", "p.rows.cells[1]", "((m * 0.5) * 2.0)"),
    ] {
        for (read, lowered) in [(format!("v({reach}.sn)"), "probe(".to_string()), (format!("{reach}.vs"), "probe(".to_string()), (format!("{reach}.deep"), format!("({k} * probe("))] {
            let src = format!("{models}module Top() {{ m: f64 = 1.5; signal a: Electrical; signal o: Electrical; {placement} }}\nanalog for Top {{ potential(a) +> 1.0; potential(o) +> {read}; }}\n");
            let design = support::Elab::design(&src);
            let (result, statistics) = Elaborator::new(&design).run_with_statistics("Top", &Overrides::none());
            let printed = result.unwrap_or_else(|d| panic!("`{read}` under `{placement}`: {d:?}")).print_structural();
            let line = printed.lines().find(|line| line.contains("b1.potential +> direct")).unwrap_or_else(|| panic!("{read}: the read of `o`: {printed}"));
            assert!(line.contains(&lowered) && line.contains(&format!("{reach}.sn")), "`{read}` lowers `{lowered}` over the cell's node: {line}");
            assert_eq!(statistics.passes, 1, "{read}");
        }
    }
}

#[test]
fn a_placement_with_a_false_where_or_a_missing_param_is_reported_once_through_a_reach() {
    // §8.1.16 (verification finding 12): a false `where` (`E0317`) or a required param left out (`E0319`) is the placement's own
    // diagnostic, reported once: the reach frame that reads the child (one level and three levels below) reports nothing again
    let models = "module W(a: Electrical) { r: f64 = 1.0 where r > 0.0; pub signal sn: Electrical; }\nanalog for W { i(a, sn) +> v(a, sn) * r; }\n\
                  module WR(a: Electrical) { r: f64; pub signal sn: Electrical; }\nanalog for WR { i(a, sn) +> v(a, sn) * r; }\n\
                  module WL2(a: Electrical) { pub let c0 = W(a) { r: -1.0 }; }\nmodule WL3(a: Electrical) { pub let mid = WL2(a); }\n\
                  module WRL2(a: Electrical) { pub let c0 = WR(a); }\nmodule WRL3(a: Electrical) { pub let mid = WRL2(a); }\n";
    for (placement, read, code) in [
        ("let p = W(a) { r: -1.0 };", "v(p.sn)", "E0317"),
        ("let p = WL3(a);", "v(p.mid.c0.sn)", "E0317"),
        ("let p = WR(a);", "v(p.sn)", "E0319"),
        ("let p = WRL3(a);", "v(p.mid.c0.sn)", "E0319"),
    ] {
        let found = Elab::run(&format!("{models}{}", top(placement, &format!("potential(o) +> {read};"))), "Top").expect_err("the placement is refused");
        assert_eq!(Elab::codes(&found), [code], "`{read}` under `{placement}`: {:?}", found.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>());
    }
}

#[test]
fn a_placement_in_generation_reads_a_param_of_its_module_and_the_loop_variable() {
    // §8.2.23, §8.1.16: a param value written in generation over a param of the module that holds it (not a key) and the loop
    // variable (`k: m3 + (j as f64)`) is each instance's own constant: the module's param at the value its placement gives
    // (`m3 = m4 * 0.5`, `m4 = m = 1.5`) and the iteration's `j`. A read three levels below the reader of the cell's node, `vs` and
    // `deep = k * vs` reads that same `k` in place, the loop variable of its iteration included
    let models = format!(
        "{}module GRows(a: Electrical) {{ m3: f64 = 1.0; for j in 0..2 {{ pub let cells = Cell(a) {{ k: m3 + (j as f64) }}; }} }}\n\
         module GTower(a: Electrical) {{ m4: f64 = 1.0; pub let rows = GRows(a) {{ m3: m4 * 0.5 }}; }}\n",
        MODELS.replace("pub let vs = v(sn);", MIXED_LETS)
    );
    let top = |read: &str| format!("{models}module Top() {{ m: f64 = 1.5; signal a: Electrical; signal o: Electrical; let p = GTower(a) {{ m4: m }}; }}\nanalog for Top {{ potential(a) +> 1.0; potential(o) +> {read}; }}\n");
    let ir = Elab::ir(&top("1.0"), "Top");
    for (iteration, k) in [(0, 1.5 * 0.5), (1, 1.5 * 0.5 + 1.0)] {
        let cell = ir.instance_at(&format!("Top.p.rows.cells[{iteration}]")).unwrap_or_else(|| panic!("cells[{iteration}] is placed"));
        assert_eq!(cell.param(0), Some(&piperine_ir::Constant::Real(piperine_ir::Real::new(k))), "cells[{iteration}].k is m3 + j");
    }
    for (read, lowered) in [("v(p.rows.cells[1].sn)", "probe("), ("p.rows.cells[1].vs", "probe("), ("p.rows.cells[1].deep", "(((m * 0.5) + (1i32 as f64)) * probe(")] {
        let printed = Elab::ir(&top(read), "Top").print_structural();
        let line = printed.lines().find(|line| line.contains("b1.potential +> direct")).unwrap_or_else(|| panic!("{read}: the read of `o`: {printed}"));
        assert!(line.contains(lowered) && line.contains("p.rows.cells[1].sn"), "`{read}` lowers `{lowered}` over the cell's node: {line}");
    }
}

#[test]
fn a_param_in_generation_over_a_param_and_the_loop_variable_reads_through_three_levels() {
    // §8.2.8, §8.2.23: `R.k` and `R.twice` of a cell whose `k` the placement gives over a param of its module and the loop
    // variable (`k: m3 + (j as f64)`): the loop variable is a constant of the iteration, so the params the value reads are the
    // module's own (`m3`, then `m4`, then the reader's `m = 1.5`), each fixed by its placement, and the reader reads each
    // iteration's `k` (`cells[1]`: 0.75 + 1, `cells[0]`: 0.75)
    let models = format!(
        "{MODELS}module GRows(a: Electrical) {{ m3: f64 = 1.0; for j in 0..2 {{ pub let cells = Cell(a) {{ k: m3 + (j as f64) }}; }} }}\n\
         module GTower(a: Electrical) {{ m4: f64 = 1.0; pub let rows = GRows(a) {{ m3: m4 * 0.5 }}; }}\n"
    );
    for (read, lowered) in [("p.rows.cells[1].k", "direct 1.75"), ("p.rows.cells[1].twice", "direct (1.75 * 2.0)"), ("p.rows.cells[0].k", "direct 0.75"), ("p.rows.cells[0].twice", "direct (0.75 * 2.0)")] {
        let src = format!("{models}module Top() {{ m: f64 = 1.5; signal a: Electrical; signal o: Electrical; let p = GTower(a) {{ m4: m }}; }}\nanalog for Top {{ potential(a) +> 1.0; potential(o) +> {read}; }}\n");
        let printed = Elab::ir(&src, "Top").print_structural();
        let line = printed.lines().find(|line| line.contains("b1.potential +> direct")).unwrap_or_else(|| panic!("{read}: the read of `o`: {printed}"));
        assert!(line.ends_with(lowered), "`{read}` lowers `{lowered}`: {line}");
    }
}
