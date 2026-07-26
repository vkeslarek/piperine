//! FLAT-04 — `urc` lumped RC line: a pure-structural mid-level module
//! elaborates and the flatten pass inlines the 3-level hierarchy
//! (Top → urcN → urc_seg → res/cap) into the expected leaf-instance count.
//!
//! The fixed-N authoring (`urc2`/`urc5`/`urc10` in `headers/spice/urc.phdl`)
//! stays inside the MVP flatten boundary: no array wires (gap-3 deferred), no
//! const-arg-into-behavior (gap-2 deferred). It exercises the same 3-level
//! inlining a parametric `urc[N]` would, with composed labels like
//! `u1.s0.r1`.

use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    SourceMap::dummy()
}

/// A `Top` module that instantiates one `urcN` ladder driven by a DC source
/// with a load resistor on the output — the FLAT-04 ngspice cross-check
/// shape (T8). For T7 only the structure matters: any driver/load would do.
fn top_using(urc_mod: &str) -> String {
    format!(
        "use piperine::disciplines;
         use spice::passives;
         use spice::sources;
         use spice::urc;

         mod Top () {{
             wire gnd : Electrical;
             wire in_ : Electrical;
             wire out : Electrical;
             v1 : vsrc (.p=in_, .n=gnd) {{ .dc = 5.0 }};
             u1 : {urc_mod} (.p=in_, .n=out, .g=gnd) {{ .r = 100.0, .c = 1.0e-9 }};
             rl : res (.p=out, .n=gnd) {{ .r = 1.0e3 }};
         }}"
    )
}

/// Each `urcN` ladder flattens to 2N leaf instances (N `res` + N `cap`),
/// composed through three label levels: `u1.sK.r1` and `u1.sK.c1`.
#[test]
fn urc5_flattens_to_ten_leaves_with_composed_labels() {
    let design = piperine_lang::parse_and_elaborate(&top_using("urc5"), &headers_source_map())
        .expect("urc5 Top elaborates");

    // Authored hierarchy intact: Top has one `u1 : urc5` instance.
    let top = design.module("Top").expect("Top authored");
    assert_eq!(top.instances().len(), 3, "Top authored: v1, u1, rl");
    assert_eq!(top.instances()[1].module_name(), "urc5", "u1 is urc5");

    // Flat form: the `urc5` instance is gone, its 5 `urc_seg` segments each
    // inlined to 2 leaves → 10 urc leaves + the v1/rl driver/load leaves.
    let flat = design.flat_module("Top").expect("Top flat form recorded");
    assert_eq!(
        flat.instances().len(),
        12,
        "5 urc_seg × 2 leaves + v1 + rl = 12 leaf instances (got {:?})",
        flat.instances()
            .iter()
            .map(|i| i.label().unwrap_or("?"))
            .collect::<Vec<_>>()
    );
    // The inlined urc5 leaves are all res/cap (v1=vsrc and rl=res are the
    // driver/load leaves kept verbatim).
    let urc_leaves: Vec<_> = flat.instances().iter().filter(|i| i.label().unwrap().starts_with("u1.")).collect();
    assert_eq!(urc_leaves.len(), 10, "exactly 10 inlined urc leaves");
    assert!(
        urc_leaves.iter().all(|i| i.module_name() == "res" || i.module_name() == "cap"),
        "inlined urc leaves are only res/cap"
    );

    // Labels compose through three levels: u1.s0.r1, u1.s0.c1, …, u1.s4.c1.
    let labels: Vec<&str> = flat.instances().iter().map(|i| i.label().unwrap()).collect();
    assert!(labels.contains(&"u1.s0.r1"), "first series R composed label present: {labels:?}");
    assert!(labels.contains(&"u1.s4.c1"), "last shunt C composed label present: {labels:?}");
    assert!(
        labels.iter().filter(|l| l.ends_with(".r1")).count() == 5,
        "exactly 5 series R leaves: {labels:?}"
    );
    assert!(
        labels.iter().filter(|l| l.ends_with(".c1")).count() == 5,
        "exactly 5 shunt C leaves: {labels:?}"
    );

    // The non-destructive invariant: authored `modules` is never mutated by
    // flattening. The authored `urc5` still has its 5 `urc_seg` instances
    // (they were inlined only into `flat_modules`).
    let urc5 = design.module("urc5").expect("urc5 authored");
    assert_eq!(urc5.instances().len(), 5, "authored urc5 keeps its 5 segments");
    assert!(
        urc5.instances().iter().all(|i| i.module_name() == "urc_seg"),
        "authored urc5's instances are urc_seg (not the inlined leaves)"
    );
}

/// `urc2` and `urc10` flatten to 4 and 20 leaves respectively — the same
/// 2N shape, scaling with the segment count. Confirms flattening is uniform
/// across the ladder sizes ngspice cross-checks (T8).
#[test]
fn urc2_and_urc10_flatten_to_expected_leaf_counts() {
    for (urc_mod, expected_leaves) in [("urc2", 6), ("urc10", 22)] {
        let design = piperine_lang::parse_and_elaborate(&top_using(urc_mod), &headers_source_map())
            .unwrap_or_else(|e| panic!("{urc_mod} Top elaborates: {e:?}"));
        let flat = design.flat_module("Top").unwrap_or_else(|| panic!("{urc_mod} flat Top"));
        assert_eq!(
            flat.instances().len(),
            expected_leaves,
            "{urc_mod}: expected {expected_leaves} leaves (2N+2 driver/load), got {}",
            flat.instances().len()
        );
        // The inlined urc leaves are all res/cap (v1=vsrc and rl=res are the
        // driver/load leaves kept verbatim).
        let urc_leaves: Vec<_> = flat
            .instances()
            .iter()
            .filter(|i| i.label().unwrap().starts_with("u1."))
            .collect();
        assert_eq!(urc_leaves.len(), expected_leaves - 2, "{urc_mod}: {} inlined urc leaves", expected_leaves - 2);
        assert!(
            urc_leaves.iter().all(|i| i.module_name() == "res" || i.module_name() == "cap"),
            "{urc_mod}: inlined urc leaves are only res/cap"
        );
    }
}

/// The mid-level `urcN` modules themselves flatten (their flat form is the
/// 2N-leaf netlist codegen consumes from `flat_module(root)`). The flat form
/// of `urc5` has 10 leaves — this is what codegen sees when the host picks
/// `urc5` as the root rather than `Top`.
#[test]
fn urc5_module_alone_flattens_to_ten_leaves() {
    let design = piperine_lang::parse_and_elaborate(&top_using("urc5"), &headers_source_map())
        .expect("urc5 Top elaborates");
    let flat_urc5 = design.flat_module("urc5").expect("urc5 has a flat form");
    assert_eq!(
        flat_urc5.instances().len(),
        10,
        "flat urc5 has 10 leaf instances (5 seg × 2 leaves)"
    );
    assert!(
        flat_urc5.instances().iter().all(|i| i.module_name() == "res" || i.module_name() == "cap"),
        "flat urc5 contains only leaves"
    );
}
