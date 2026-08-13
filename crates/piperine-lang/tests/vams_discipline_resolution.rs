//! VAMS T26 (VAMS-17) — `ResolveDisciplines`: Annex F discipline
//! resolution for a port/net declared with no explicit discipline. This
//! pass implements **Basic discipline resolution (§7.4.4.1)**, the LRM's
//! own stated default of the two documented methods — see the pass's
//! `//!` module doc (`elab/lower/disciplines.rs`) for why Basic was
//! chosen over Detail.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const LEAF: &str = "
module leaf (inout electrical p, inout electrical n);
endmodule
";

#[test]
fn undeclared_wire_resolves_from_a_connected_instance_port() {
    // `mid` is never declared as a port or a net — LRM implicit-net
    // declaration (§3.7/1364) makes it a real, undeclared-discipline net;
    // it's wired to `leaf`'s `p` (electrical) through the instance
    // connection, so basic resolution assigns it `electrical`.
    let src = format!(
        "{LEAF}\nmodule top (inout electrical a, inout electrical b);\n\
         leaf l1(mid, b);\n\
         endmodule\n"
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let mid = top.wires().iter().find(|w| w.name == "mid").expect("mid wire");
    assert_eq!(mid.ty.discipline_name(), "electrical");
}

#[test]
fn undeclared_port_resolves_from_a_connected_instance_port() {
    let src = format!(
        "{LEAF}\nmodule top (a, b);\n\
         inout a;\n\
         inout electrical b;\n\
         leaf l1(a, b);\n\
         endmodule\n"
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let a = top.ports().iter().find(|p| p.name() == "a").expect("port a");
    assert_eq!(a.net_type().discipline_name(), "electrical");
}

#[test]
fn fully_unconnected_undeclared_net_fails_loud_naming_the_method() {
    let src = "
        module top (inout electrical a);
            inout floating;
        endmodule
    ";
    let msg = elab_err(src);
    assert!(msg.contains("floating"), "names the unresolved net: {msg}");
    assert!(msg.contains("basic discipline resolution"), "names the Annex F method applied: {msg}");
    assert!(msg.contains("7.4.4.1") || msg.contains("§7.4"), "cites the LRM clause: {msg}");
}

#[test]
fn conflicting_disciplines_from_two_connections_fail_loud() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module leaf_e (inout electrical p);
        endmodule

        module leaf_l (inout logic p);
        endmodule

        module top ();
            leaf_e e1(shared);
            leaf_l l1(shared);
        endmodule
    ";
    let msg = elab_err(src);
    assert!(msg.contains("shared"), "names the conflicting net: {msg}");
}
