//! Vector (bus) declarations — `lrm-conformance-wave3` Phase 1 (T1-T6).
//! Proves the leading-range declarator slot (Annex A.2.1.2/A.2.2.1's
//! `range`) on net (T1) declarations.
//!
//! LRM-COVERS: 3.6.3, A.2.1.2

use piperine_lang::pom::NetType;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration/parse error")
        .to_string()
}

fn module_wrapper(body: &str) -> String {
    format!("module top ();\n{body}\nendmodule\n")
}

// ── T1: leading range on net declarations, §3.6.3/A.2.1.2 ─────────────────

#[test]
fn leading_range_on_a_single_net_gives_array_width_four() {
    let d = elab(&module_wrapper("electrical [0:3] bus;"));
    let m = d.module("top").unwrap();
    let w = m.wire("bus").expect("bus declared");
    match w.net_type() {
        NetType::Array(inner, n) => {
            assert_eq!(*n, 4);
            assert_eq!(inner.discipline_name(), "electrical");
        }
        other => panic!("expected NetType::Array, got {other:?}"),
    }
}

#[test]
fn leading_range_applies_to_every_declarator_in_the_list() {
    let d = elab(&module_wrapper("electrical [3:0] a, b;"));
    let m = d.module("top").unwrap();
    for name in ["a", "b"] {
        let w = m.wire(name).unwrap_or_else(|| panic!("{name} declared"));
        match w.net_type() {
            NetType::Array(_, n) => assert_eq!(*n, 4, "{name} must be width 4"),
            other => panic!("{name}: expected NetType::Array, got {other:?}"),
        }
    }
}

#[test]
fn declarator_with_both_leading_and_own_range_fails_loud_naming_both_spans() {
    let err = elab_err(&module_wrapper("electrical [3:0] bus[1:0];"));
    assert!(err.contains("bus"), "must name the net: {err}");
    assert!(err.contains("leading range"), "must name the leading range: {err}");
    assert!(err.contains("declarator range"), "must name the declarator's own range: {err}");
    assert!(err.contains("3.6.3"), "must cite §3.6.3: {err}");
}

#[test]
fn ascending_and_descending_leading_ranges_both_give_width_four() {
    let d_desc = elab(&module_wrapper("electrical [3:0] bus;"));
    let d_asc = elab(&module_wrapper("electrical [0:3] bus;"));
    for d in [&d_desc, &d_asc] {
        let m = d.module("top").unwrap();
        let w = m.wire("bus").unwrap();
        assert_eq!(w.net_type().width(), 4);
    }
}

#[test]
fn non_constant_leading_range_bounds_fail_loud_citing_3_6_3() {
    let src = module_wrapper("integer i;\nelectrical [i:0] bus;\ninitial i = 0;");
    let err = elab_err(&src);
    assert!(err.contains("3.6.3"), "must cite §3.6.3: {err}");
}
