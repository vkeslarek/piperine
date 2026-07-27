//! dv-core T9 (DVC-05, D4) — `Module::constraints` exposes the authored
//! block; monomorphized variants (`Dac[8]` → `Dac__8`) carry it exactly as
//! they carry their behavior bodies; the POM holds exactly one authored
//! block per authored module; a selector walk (`constraint::*`) reaches
//! the block on both the authored module and a variant. No accessor reads
//! `flat_modules` (MD-25, UNBREAKABLE).

use piperine_lang::pom::node::Node;
use piperine_lang::pom::Named;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> piperine_lang::pom::Design {
    parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &SourceMap::dummy())
        .expect("fixture must elaborate")
}

/// DVC-05: `Dac[8]` → `Dac__8` carries the block in authored form, and the
/// design holds no other copy — exactly one authored block per authored
/// module (D4).
#[test]
fn monomorphized_variant_carries_the_authored_block() {
    let design = elaborate(
        "mod Dac[N] ( inout out : Electrical, inout gnd : Electrical ) {\n\
             param vref : Real = 1.8;\n\
         }\n\
         constraint Dac {\n\
             require full_scale : V(out, gnd) <= vref;\n\
             var headroom : Real = vref - V(out, gnd);\n\
         }\n\
         mod Top ( inout out : Electrical, inout gnd : Electrical ) {\n\
             dac : Dac[8] ( out, gnd );\n\
         }",
    );

    assert!(
        design.module("Dac").is_none(),
        "a generic module exists only through its variants"
    );
    let variant = design.module("Dac__8").expect("Dac[8] monomorphizes to Dac__8");
    let blocks = variant.constraints();
    assert_eq!(blocks.len(), 1, "exactly one authored block per authored module (D4)");
    let block = &blocks[0];
    assert_eq!(block.name(), "Dac", "the block keeps its authored name");
    assert_eq!(block.stmts().len(), 2, "the block keeps its authored statements");
    assert!(
        block.var_type("headroom").is_some(),
        "the elaborated side table rides along"
    );
}

/// DVC-05: a non-generic module holds exactly one block per authored
/// `constraint` declaration — no duplication through elaboration.
#[test]
fn one_authored_block_per_authored_module() {
    let design = elaborate(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; }\n\
         constraint M { require ok : x >= 0.0; }",
    );
    assert_eq!(design.module("M").expect("M elaborated").constraints().len(), 1);
}

/// DVC-05: a selector walk (`constraint::*`) reaches the block on both an
/// authored module and a monomorphized variant — and, through an instance
/// step, on the child module the instance instantiates.
#[test]
fn selector_walk_reaches_the_block_on_authored_module_and_variant() {
    let design = elaborate(
        "mod Dac[N] ( inout out : Electrical, inout gnd : Electrical ) {\n\
             param vref : Real = 1.8;\n\
         }\n\
         constraint Dac { require full_scale : V(out, gnd) <= vref; }\n\
         mod Plain ( inout p : Electrical ) { param x : Real = 1.0; }\n\
         constraint Plain { require ok : x >= 0.0; }\n\
         mod Top ( inout out : Electrical, inout gnd : Electrical ) {\n\
             dac : Dac[8] ( out, gnd );\n\
         }",
    );

    // The authored (non-generic) module.
    let plain = design.module("Plain").expect("Plain elaborated");
    let sel = Node::Module(plain).select(&design, "constraint::*").expect("selector parses");
    assert_eq!(sel.len(), 1, "the authored module's block is reachable");
    assert_eq!(sel.get(0).expect("one node").name(), "Plain");

    // The monomorphized variant.
    let variant = design.module("Dac__8").expect("variant elaborated");
    let sel = Node::Module(variant).select(&design, "constraint::*").expect("selector parses");
    assert_eq!(sel.len(), 1, "the variant's block is reachable");
    assert_eq!(sel.get(0).expect("one node").name(), "Dac");

    // Through the instance axis, from the parent — the authored tree.
    let top = design.module("Top").expect("Top elaborated");
    let sel = Node::Module(top).select(&design, "dac/constraint::*").expect("selector parses");
    assert_eq!(sel.len(), 1, "the block is reachable through the instance");
    assert_eq!(sel.get(0).expect("one node").name(), "Dac");

    // A module with no constraint block selects nothing.
    let sel = Node::Module(top).select(&design, "constraint::*").expect("selector parses");
    assert_eq!(sel.len(), 0);
}
