//! Vector (bus) declarations — `lrm-conformance-wave3` Phase 1 (T1-T6).
//! Proves the leading-range declarator slot (Annex A.2.1.2/A.2.2.1's
//! `range`) on net (T1), port (T2), and 1364 net/variable (T3)
//! declarations.
//!
//! LRM-COVERS: 3.6.3, A.2.1.2, 3.7, A.2.2.1

use piperine_lang::pom::{DigitalNetKind, NetType};
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

// ── T2: leading range on port declarations, §3.7/A.2.1.2 ──────────────────

#[test]
fn leading_range_on_an_input_port_gives_width_eight() {
    // A bare `input [0:7] p;` carries no discipline and would need Annex F
    // connectivity-based resolution (unavailable to an unconnected,
    // bodiless port); `wire` is the 1364 net-type keyword, not a
    // discipline, so this exercises the leading-range slot without that
    // dependency.
    let d = elab("module top (p);\ninput wire [0:7] p;\nendmodule\n");
    let m = d.module("top").unwrap();
    let p = m.port("p").expect("p declared");
    assert_eq!(p.net_type().width(), 8);
}

#[test]
fn leading_range_on_a_discipline_bearing_inout_port_gives_width_four() {
    let d = elab("module top (bus);\ninout electrical [3:0] bus;\nendmodule\n");
    let m = d.module("top").unwrap();
    let p = m.port("bus").expect("bus declared");
    assert_eq!(p.net_type().width(), 4);
    assert_eq!(p.net_type().discipline_name(), "electrical");
}

#[test]
fn port_list_form_and_body_redeclaration_form_agree_on_width() {
    let in_list = elab("module top (input wire [0:3] a);\nendmodule\n");
    let in_body = elab("module top (a);\ninput wire [0:3] a;\nendmodule\n");
    for d in [&in_list, &in_body] {
        let a = d.module("top").unwrap().port("a").expect("a declared");
        assert_eq!(a.net_type().width(), 4);
    }
}

// ── T3: leading range on 1364 net/variable declarations, A.2.2.1 ──────────

#[test]
fn leading_range_on_reg_gives_digital_array_width_four() {
    let d = elab(&module_wrapper("reg [3:0] q;"));
    let m = d.module("top").unwrap();
    let w = m.wire("q").expect("q declared");
    match w.net_type() {
        NetType::Array(inner, n) => {
            assert_eq!(*n, 4);
            assert!(matches!(**inner, NetType::Digital1364(DigitalNetKind::Reg)));
        }
        other => panic!("expected NetType::Array(Digital1364(Reg), 4), got {other:?}"),
    }
}

#[test]
fn leading_range_on_wire_gives_digital_array_width_eight() {
    let d = elab(&module_wrapper("wire [7:0] w;"));
    let m = d.module("top").unwrap();
    let w = m.wire("w").expect("w declared");
    match w.net_type() {
        NetType::Array(inner, n) => {
            assert_eq!(*n, 8);
            assert!(matches!(**inner, NetType::Digital1364(DigitalNetKind::Wire)));
        }
        other => panic!("expected NetType::Array(Digital1364(Wire), 8), got {other:?}"),
    }
}

#[test]
fn reg_vector_and_electrical_vector_of_equal_width_produce_equal_node_counts() {
    let d_reg = elab(&module_wrapper("reg [3:0] q;"));
    let d_elec = elab(&module_wrapper("electrical [3:0] bus;"));
    let reg_w = d_reg.module("top").unwrap().wire("q").unwrap().net_type().width();
    let elec_w = d_elec.module("top").unwrap().wire("bus").unwrap().net_type().width();
    assert_eq!(reg_w, elec_w);
    assert_eq!(reg_w, 4);
}

// ── T5: memories — packed range plus unpacked depth, A.2.2.1 ─────────────

#[test]
fn memory_with_leading_range_and_depth_elaborates_to_depth_8_times_width_4() {
    let d = elab(&module_wrapper("reg [3:0] mem [0:7];"));
    let m = d.module("top").unwrap();
    let w = m.wire("mem").expect("mem declared");
    match w.net_type() {
        NetType::Array(inner, depth) => {
            assert_eq!(*depth, 8, "outer dimension is depth");
            match inner.as_ref() {
                NetType::Array(elem, width) => {
                    assert_eq!(*width, 4, "inner dimension is the packed width");
                    assert!(matches!(**elem, NetType::Digital1364(DigitalNetKind::Reg)));
                }
                other => panic!("expected a nested Array(Digital1364(Reg), 4), got {other:?}"),
            }
        }
        other => panic!("expected NetType::Array(Array(Reg,4), 8), got {other:?}"),
    }
    assert_eq!(w.net_type().width(), 32, "width() reports the flattened 8*4 element count");
}

#[test]
fn memory_declared_with_only_a_depth_works() {
    let d = elab(&module_wrapper("reg mem [0:7];"));
    let m = d.module("top").unwrap();
    let w = m.wire("mem").expect("mem declared");
    match w.net_type() {
        NetType::Array(elem, depth) => {
            assert_eq!(*depth, 8);
            assert!(matches!(**elem, NetType::Digital1364(DigitalNetKind::Reg)), "each element is a scalar reg");
        }
        other => panic!("expected NetType::Array(Digital1364(Reg), 8), got {other:?}"),
    }
    assert_eq!(w.net_type().width(), 8);
}

#[test]
fn width_times_depth_above_the_cap_fails_loud_naming_the_declaration_and_the_cap() {
    let err = elab_err(&module_wrapper("reg [65535:0] huge [65535:0];"));
    assert!(err.contains("huge"), "must name the declaration: {err}");
    assert!(err.contains("65536"), "must name the configured cap: {err}");
}
