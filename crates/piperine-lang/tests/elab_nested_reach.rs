//! Scope: `inst.name` through children of children at any depth, at elaboration (feature `nested-reach`, part III §8.2.8, §8.4.8):
//! the node, the carrier and the `let` over the solve of a cell three and four levels below the reader are reached by name, through
//! templates and in generation, in any text order, and a cycle of node connections through such a reach binds each port.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;
#[path = "support/nested_reach.rs"]
mod nested;

use nested::{MODELS, READS};
use piperine_ir::{Constant, Ir, NodeRef, Real};
use support::Elab;

/// The top that reads `reach` below the placement `placement`, with `reads` as its `analog` block.
fn top(placement: &str, reads: &str) -> String {
    format!("module Top() {{ signal a: Electrical; signal o: Electrical; {placement} }}\nanalog for Top {{ potential(a) +> 1.0; {reads} }}\n")
}

/// The path of the node each port and signal of an instance is bound to, in the instance's node order.
fn node_paths(ir: &Ir, path: &str) -> Vec<String> {
    let instance = ir.instance_at(path).unwrap_or_else(|| panic!("no instance {path}"));
    instance
        .bindings()
        .iter()
        .map(|bound| match bound {
            NodeRef::Node(id) => ir.node(*id).path().to_string(),
            other => format!("{other:?}"),
        })
        .collect()
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
    // `let` over it; the instance the read names holds that value
    for (placement, reach, form) in READS {
        let ir = Elab::ir(&format!("{MODELS}{}", top(placement, &format!("potential(o) +> {reach}.k + {reach}.twice;"))), "Top");
        let cell = ir.instance_at(&format!("Top.{reach}")).unwrap_or_else(|| panic!("{form}: no cell"));
        let wanted = if reach.contains("rows") { 2.0 } else { 1.0 };
        assert_eq!(cell.param(0), Some(&Constant::Real(Real::new(wanted))), "{form}");
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
        ("let p = Grid { cell: Cell { } }(q.row.cells[1].sn); let q = Grid { cell: Cell { } }(p.row.cells[1].sn);", "let p = Grid { cell: Cell { } }(p.row.cells[1].sn);", "row.cells[1]", "row.cells[1].sn"),
        ("let p = Plane { cell: Cell { } }(q.grid.row.cells[1].sn); let q = Plane { cell: Cell { } }(p.grid.row.cells[1].sn);", "let p = Plane { cell: Cell { } }(p.grid.row.cells[1].sn);", "grid.row.cells[1]", "grid.row.cells[1].sn"),
    ] {
        for text in [format!("{MODELS}module Top() {{ {pair} }}"), format!("module Top() {{ {pair} }}{MODELS}")] {
            let ir = Elab::ir(&text, "Top");
            assert_eq!(node_paths(&ir, &format!("Top.p.{path}")), [format!("Top.q.{reach}"), format!("Top.p.{reach}")], "{pair}");
            assert_eq!(node_paths(&ir, &format!("Top.q.{path}")), [format!("Top.p.{reach}"), format!("Top.q.{reach}")], "{pair}");
        }
        let ir = Elab::ir(&format!("{MODELS}module Top() {{ {own} }}"), "Top");
        assert_eq!(node_paths(&ir, &format!("Top.p.{path}")), [format!("Top.p.{reach}"), format!("Top.p.{reach}")], "{own}");
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
