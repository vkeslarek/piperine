//! LRM-COVERS: 3.9, 3.10
//!
//! lrm-conformance-wave3 T26: an out-of-module (out-of-context) discipline
//! declaration — `electrical top.u1.n;`, LRM §3.9/§3.10's own worked
//! example (`electrical example2.net;`) — assigns a discipline to a net
//! declared in a *different* module, by dotted hierarchical path. §3.10
//! ranks three ways a net's discipline can be decided, highest first: (1)
//! an out-of-module declaration, (2) the net's own local declaration, (3)
//! Annex F discipline resolution — "It is not legal to have two different
//! disciplines at the same level of precedence for the same net."
//!
//! Rank 3 (discipline resolution, including the `` `default_discipline ``
//! fallback) is already covered independently in
//! `vams_annex_f_domain.rs`; this file proves the new rank-1 mechanism and
//! that it wins over both of the other two ranks.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// AC1 (T26 Done-when #1), the LRM's own two-segment shape
/// (`example2.net`): an out-of-module declaration resolves the target
/// net's discipline, with `path[0]` naming the target module directly.
#[test]
fn two_segment_path_resolves_the_target_ports_discipline() {
    let src = r#"
        module leaf (inout p);
        endmodule
        module elsewhere ();
            electrical leaf.p;
        endmodule
        "#;
    let d = elab(src);
    let leaf = d.module("leaf").expect("leaf elaborated");
    let p = leaf.port("p").expect("port p");
    assert_eq!(p.net_type().discipline_name(), "electrical");
}

/// AC1, the task's own worked example shape (`top.u1.n`): a deeper path
/// walks two levels of instances before reaching the target net.
#[test]
fn deep_instance_path_resolves_the_target_ports_discipline() {
    let src = r#"
        module leaf2 (inout p);
        endmodule
        module mid2 ();
            leaf2 u1();
        endmodule
        module top2 ();
            mid2 u2();
            electrical top2.u2.u1.p;
        endmodule
        "#;
    let d = elab(src);
    let leaf2 = d.module("leaf2").expect("leaf2 elaborated");
    let p = leaf2.port("p").expect("port p");
    assert_eq!(p.net_type().discipline_name(), "electrical");
}

/// AC2 (T26 Done-when #2, rank 1 vs rank 2): the target net already
/// carries a local discipline declaration (`inout electrical p;`); the
/// out-of-module declaration still wins, per §3.10's precedence order.
#[test]
fn out_of_module_declaration_beats_the_local_declaration() {
    let src = r#"
        module leaf_b (inout electrical p);
        endmodule
        module top_b ();
            leaf_b lb();
            thermal top_b.lb.p;
        endmodule
        "#;
    let d = elab(src);
    let leaf_b = d.module("leaf_b").expect("leaf_b elaborated");
    let p = leaf_b.port("p").expect("port p");
    assert_eq!(
        p.net_type().discipline_name(),
        "thermal",
        "out-of-module declaration must override the net's own local discipline"
    );
}

/// AC2 (rank 1 vs rank 3): absent the override, the target port
/// (unresolved, no connections) would fall back to the in-force
/// `` `default_discipline `` (`thermal`, Annex F step 4.b — the same
/// mechanism `vams_annex_f_domain.rs` proves independently). The
/// out-of-module declaration still wins over that resolution outcome.
#[test]
fn out_of_module_declaration_beats_discipline_resolution() {
    let src = r#"
        `default_discipline thermal
        module leaf_d (inout p);
        endmodule
        module top_d ();
            leaf_d ld();
            electrical top_d.ld.p;
        endmodule
        "#;
    let d = elab(src);
    let leaf_d = d.module("leaf_d").expect("leaf_d elaborated");
    let p = leaf_d.port("p").expect("port p");
    assert_eq!(
        p.net_type().discipline_name(),
        "electrical",
        "out-of-module declaration must override the default_discipline fallback"
    );
}

/// AC3 (T26 Done-when #3): two out-of-module declarations targeting the
/// same net with different disciplines fail loud, naming both spans (in
/// practice: both disciplines and the target net) — §3.10's "not legal to
/// have two different disciplines at the same level of precedence".
#[test]
fn two_conflicting_out_of_module_declarations_fail_loud() {
    let src = r#"
        module leaf_f (inout p);
        endmodule
        module top_f1 ();
            electrical leaf_f.p;
        endmodule
        module top_f2 ();
            thermal leaf_f.p;
        endmodule
        "#;
    let msg = elab_err(src);
    assert!(msg.contains("leaf_f.p"), "error must name the conflicting target net, got: {msg}");
    assert!(msg.contains("electrical"), "error must name the first discipline, got: {msg}");
    assert!(msg.contains("thermal"), "error must name the second discipline, got: {msg}");
    assert!(msg.contains("3.10"), "error must cite §3.10, got: {msg}");
}
