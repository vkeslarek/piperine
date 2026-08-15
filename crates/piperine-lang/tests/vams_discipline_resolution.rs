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

// ── connect ... resolveto (§7.4.4.3, LRMC-C7-04/C10-20, T33) ────────────

/// The exact same genuine same-domain conflict as
/// `conflicting_disciplines_from_two_connections_fail_loud` above, but
/// now with an explicit `connect logic, electrical resolveto electrical;`
/// rule in force — §7.4.4.3: "if an interconnect is assigned a
/// discipline, that discipline shall be used unless the `resolveto`
/// connect statement overrides the discipline." The rule forces `shared`
/// to `electrical` instead of hard-failing.
#[test]
fn resolveto_forces_the_named_discipline_instead_of_failing_loud() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        connectrules cr;
            connect logic, electrical resolveto electrical;
        endconnectrules

        module leaf_e (inout electrical p);
        endmodule

        module leaf_l (inout logic p);
        endmodule

        module top ();
            leaf_e e1(shared);
            leaf_l l1(shared);
        endmodule
    ";
    let d = elab(src);
    let top = d.module("top").expect("resolveto forces resolution instead of failing");
    let shared = top.wires().iter().find(|w| w.name == "shared").expect("shared wire");
    assert_eq!(shared.ty.discipline_name(), "electrical");
}

/// `resolveto exclude` — the pair stays forbidden, but the error now
/// names the real §7.7.2 mechanism instead of the generic basic-
/// resolution failure text.
#[test]
fn resolveto_exclude_still_fails_loud_naming_the_mechanism() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        connectrules cr;
            connect logic, electrical resolveto exclude;
        endconnectrules

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
    assert!(msg.contains("excluded"), "names the resolveto exclude mechanism: {msg}");
}

/// LRM §7.7.2 Example 1's own worked precedence: a narrower two-
/// discipline rule (`x,y resolveto x`) wins over a broader three-
/// discipline rule (`x,y,a resolveto a`) naming the same pair.
#[test]
fn narrowest_resolveto_rule_wins_over_a_broader_one() {
    let src = "
        discipline x;
            domain discrete;
        enddiscipline
        discipline y;
            domain discrete;
        enddiscipline

        connectrules cr;
            connect x,y,electrical resolveto electrical;
            connect x,y resolveto x;
        endconnectrules

        module leaf_x (inout x p);
        endmodule

        module leaf_y (inout y p);
        endmodule

        module top ();
            leaf_x e1(shared);
            leaf_y l1(shared);
        endmodule
    ";
    let d = elab(src);
    let top = d.module("top").expect("narrowest x,y rule applies");
    let shared = top.wires().iter().find(|w| w.name == "shared").expect("shared wire");
    assert_eq!(shared.ty.discipline_name(), "x", "the narrower `x,y resolveto x` rule must win over the broader one");
}
