//! FLAT-04 — the `urc` lumped RC line: a purely structural mid-level module
//! elaborates and the flatten pass inlines the 3-level hierarchy
//! (Top → urcN → urc_seg → res/cap) into the expected leaf-instance count,
//! while the authored hierarchy stays untouched (MD-25).
//!
//! **Retargeted from PHDL.** The original fixture instantiated `urc[5]` —
//! PHDL's generic-module form, monomorphized to a `urc__5` module. VAMS has
//! no type-level generic instantiation, so `headers/spice/urc.va` ships
//! three concrete ladders (`urc2`/`urc5`/`urc10`), each a `parameter
//! integer segs` plus a `generate for` over the middle segments. The leaf
//! counts this file asserts are unchanged by that rewrite — 2N leaves per
//! N-segment ladder — which is the property FLAT-04 is actually about.

use piperine_lang::SourceMap;

/// A `Top` that drives one `urcN` ladder from a DC source into a load
/// resistor — the FLAT-04 ngspice cross-check shape. Only the structure
/// matters here; any driver/load would do.
fn top_using(urc_mod: &str) -> String {
    format!(
        "`include \"disciplines.vams\"\n\
         `include \"spice/passives.va\"\n\
         `include \"spice/sources.va\"\n\
         `include \"spice/urc.va\"\n\
         module Top();\n\
         \x20   electrical gnd, in_, out;\n\
         \x20   vsrc #(.dc(5.0)) v1 (.p(in_), .n(gnd));\n\
         \x20   {urc_mod} #(.r(100.0), .c(1.0e-9)) u1 (.p(in_), .n(out), .g(gnd));\n\
         \x20   res #(.r(1.0e3)) rl (.p(out), .n(gnd));\n\
         endmodule\n"
    )
}

fn elaborate(urc_mod: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(&top_using(urc_mod), &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("{urc_mod} Top must elaborate: {e:?}"))
}

/// Every leaf inlined out of the ladder, by label prefix — `u1.` is the
/// instance the ladder was instantiated under, so every leaf composed
/// through it carries that prefix regardless of how the intermediate
/// segment labels are spelled.
fn ladder_leaves(flat: &piperine_lang::Module) -> Vec<&piperine_lang::Instance> {
    flat.instances().iter().filter(|i| i.label().unwrap_or("").starts_with("u1.")).collect()
}

/// The 5-segment ladder flattens to 10 leaves (5 series R + 5 shunt C),
/// plus the driver and load kept verbatim.
#[test]
fn urc5_flattens_to_ten_leaves() {
    let design = elaborate("urc5");

    // Authored hierarchy: Top holds exactly what was written — v1, u1, rl.
    let top = design.module("Top").expect("Top authored");
    assert_eq!(top.instances().len(), 3, "Top authored: v1, u1, rl");
    assert_eq!(top.instances()[1].module_name(), "urc5", "u1 instantiates the urc5 ladder");

    let flat = design.flat_module("Top").expect("Top flat form recorded");
    assert_eq!(
        flat.instances().len(),
        12,
        "5 urc_seg x 2 leaves + v1 + rl = 12 (got {:?})",
        flat.instances().iter().map(|i| i.label().unwrap_or("?")).collect::<Vec<_>>()
    );

    let leaves = ladder_leaves(flat);
    assert_eq!(leaves.len(), 10, "exactly 10 inlined ladder leaves");
    assert!(
        leaves.iter().all(|i| i.module_name() == "res" || i.module_name() == "cap"),
        "the ladder inlines to res/cap only, got {:?}",
        leaves.iter().map(|i| i.module_name()).collect::<Vec<_>>()
    );
    assert_eq!(
        leaves.iter().filter(|i| i.module_name() == "res").count(),
        5,
        "5 series resistors"
    );
    assert_eq!(leaves.iter().filter(|i| i.module_name() == "cap").count(), 5, "5 shunt capacitors");

    // Labels compose through all three levels — each leaf is reachable by a
    // distinct path rooted at the `u1` instance.
    let labels: Vec<&str> = leaves.iter().map(|i| i.label().unwrap()).collect();
    assert_eq!(
        labels.iter().collect::<std::collections::HashSet<_>>().len(),
        labels.len(),
        "every composed leaf label is unique: {labels:?}"
    );
}

/// MD-25's non-destructive invariant: flattening never mutates the authored
/// modules. `urc5` keeps exactly the segments its *source* declares — the
/// two explicit ones (`s0`, `slast`); the `generate for` body's instances
/// exist only in the derived flat form, never in the authored module.
#[test]
fn flattening_does_not_mutate_the_authored_ladder() {
    let design = elaborate("urc5");
    let urc5 = design.module("urc5").expect("urc5 authored");

    assert_eq!(
        urc5.instances().len(),
        2,
        "authored urc5 holds only its two literally-written urc_seg instances \
         (the generate-loop segments are a derived artifact, MD-25), got {:?}",
        urc5.instances().iter().map(|i| i.label().unwrap_or("?")).collect::<Vec<_>>()
    );
    assert!(
        urc5.instances().iter().all(|i| i.module_name() == "urc_seg"),
        "authored urc5 instantiates urc_seg, not the inlined res/cap leaves"
    );
}

/// The 2N shape scales across the ladder sizes ngspice cross-checks.
#[test]
fn urc2_and_urc10_flatten_to_expected_leaf_counts() {
    for (urc_mod, segs) in [("urc2", 2usize), ("urc10", 10)] {
        let design = elaborate(urc_mod);
        let flat = design.flat_module("Top").unwrap_or_else(|| panic!("{urc_mod} flat Top"));

        let expected_leaves = 2 * segs + 2; // 2 per segment, plus v1 and rl
        assert_eq!(
            flat.instances().len(),
            expected_leaves,
            "{urc_mod}: expected {expected_leaves} leaves (2N + driver + load), got {}",
            flat.instances().len()
        );
        assert_eq!(
            ladder_leaves(flat).len(),
            2 * segs,
            "{urc_mod}: {} inlined ladder leaves",
            2 * segs
        );
    }
}
