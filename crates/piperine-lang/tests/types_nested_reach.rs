//! Scope: `inst.name` through children of children at any depth in the checker (feature `nested-reach`, part III §8.2.8, §8.4.8):
//! a node, a param, a `let` over the params, a `let` over the solve and a carrier of a cell three and four levels below the reader
//! type, by name, through templates and in generation; an item that is not `pub` is refused at any depth; and a cycle of node
//! connections through such a reach types as hardware.

#[path = "support/nested_reach.rs"]
mod nested;

use nested::{MODELS, READS};
use piperine_lang::pom::Design;
use piperine_lang::types::Types;

/// The `(code, message)` pairs of the design and of the check of `Top` of `src`.
fn found(src: &str) -> Vec<(String, String)> {
    let design = Design::builder().source("top.msl", src).build();
    let mut found: Vec<(String, String)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
    if let Err(refused) = Types::check(&design, "Top") {
        found.extend(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())));
    }
    found
}

#[test]
fn every_kind_of_item_types_three_and_four_levels_below_the_reader() {
    // §8.2.8, §8.4.8: `R.sn` (a node), `R.k` (a param), `R.twice` (a `let` over the params), `R.vs` (a `let` over the solve) and `R.q`
    // (a carrier) of the cell `R` names, placed by name, through templates and in generation, type with no diagnostic
    for (placement, reach, form) in READS {
        let top = format!("module Top() {{ signal a: Electrical; signal o: Electrical; {placement} }}\nanalog for Top {{ v(o) +> v({reach}.sn) + {reach}.k + {reach}.twice + {reach}.vs + {reach}.q; }}");
        assert_eq!(found(&format!("{MODELS}{top}")), Vec::<(String, String)>::new(), "{form}: {reach}");
    }
}

#[test]
fn an_item_that_is_not_pub_is_refused_three_and_four_levels_below_the_reader() {
    // §8.2.8: `hidden` is no `pub` item of the cell, so no read of it types, whatever the depth
    for (placement, reach, form) in READS {
        let top = format!("module Top() {{ signal a: Electrical; signal o: Electrical; {placement} }}\nanalog for Top {{ v(o) +> {reach}.hidden; }}");
        let codes: Vec<String> = found(&format!("{MODELS}{top}")).into_iter().map(|(code, _)| code).collect();
        assert!(!codes.is_empty() && codes.iter().all(|code| code == "E0343"), "{form}: a read of a private item is E0343: {codes:?}");
    }
}

#[test]
fn a_cycle_of_node_connections_three_and_four_levels_deep_types_as_hardware() {
    // §8.2.14, §8.3.5: a cross pair and a self-connection through `p.mid.c0.sn` and `p.top.mid.c0.sn`, by name, through templates
    // and in a generation, are connections, so no `E0332`
    for (placement, form) in [
        ("let p = Level3(q.mid.c0.sn); let q = Level3(p.mid.c0.sn);", "3 pair"),
        ("let p = Level3(p.mid.c0.sn);", "3 own"),
        ("let p = Level4(q.top.mid.c0.sn); let q = Level4(p.top.mid.c0.sn);", "4 pair"),
        ("let p = Level4(p.top.mid.c0.sn);", "4 own"),
        ("let p = Block { cell: Cell { } }(q.bank.c0.sn); let q = Block { cell: Cell { } }(p.bank.c0.sn);", "3 template pair"),
        ("let p = Wing { cell: Cell { } }(p.block.bank.c0.sn);", "4 template own"),
        ("let p = Grid { cell: Cell { } }(q.row.cells[1].sn); let q = Grid { cell: Cell { } }(p.row.cells[0].sn);", "3 generation pair"),
        ("let p = Plane { cell: Cell { } }(p.grid.row.cells[1].sn);", "4 generation own"),
    ] {
        assert_eq!(found(&format!("{MODELS}module Top() {{ {placement} }}")), Vec::<(String, String)>::new(), "{form}");
    }
}
