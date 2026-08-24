//! LRM-COVERS: 7.4.1, 7.4.4.1, 7.4.4.2, 7.4.4.3
//! VAMS T26 (VAMS-17) — `ResolveDisciplines`: Annex F discipline
//! resolution for a port/net declared with no explicit discipline.
//! **Basic discipline resolution (§7.4.4.1)**, the LRM's own stated
//! default of the two documented methods, is exercised by every test in
//! this file that doesn't seed `DisciplineResolutionMode::Detail`
//! explicitly. T60 (LRMW2-56) adds **Detail (§7.4.4.2)** — see the
//! pass's `//!` module doc (`elab/lower/disciplines.rs`) for how the two
//! modes differ and how a caller selects one.

use piperine_lang::elab::registry::DisciplineResolutionMode;
use piperine_lang::{parse_and_elaborate, parse_and_elaborate_seeded, SourceMap};

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

// ── Detail discipline resolution (§7.4.4.2, LRMW2-56, T60) ─────────────

/// Two same-domain, genuinely incompatible disciplines (unrelated bound
/// potential natures, different `units`, no shared ancestor) whose
/// undeclared shared net can *only* resolve via an explicit `connect ...
/// resolveto` rule — there is no continuous/discrete mismatch, no shared
/// nature lineage, and no plain (non-`resolveto`) `connectrules` bridge
/// for `InsertConnectModules` to fall back on.
const INCOMPATIBLE_DISCRETE_PAIR: &str = r#"
    nature Volts;
        access = V;
        units = "V";
        abstol = 1e-6;
    endnature
    nature Amps;
        access = I;
        units = "A";
        abstol = 1e-12;
    endnature
    discipline disc_a;
        domain discrete;
        potential Volts;
    enddiscipline
    discipline disc_b;
        domain discrete;
        potential Amps;
    enddiscipline

    connectrules cr;
        connect disc_a, disc_b resolveto disc_a;
    endconnectrules

    module leaf_a (inout disc_a p);
    endmodule
    module leaf_b (inout disc_b p);
    endmodule
    module top ();
        leaf_a a1(shared);
        leaf_b b1(shared);
    endmodule
"#;

/// Basic mode (the default): §7.4.4.1's own resolveto path resolves the
/// conflict, exactly as `resolveto_forces_the_named_discipline_instead_of_failing_loud`
/// above — Detail's addition must not change Basic's own behavior where
/// the two agree (T60 Done-when #2).
#[test]
fn basic_mode_resolves_the_discrete_pair_via_resolveto() {
    let d = elab(INCOMPATIBLE_DISCRETE_PAIR);
    let top = d.module("top").expect("basic mode resolves via resolveto");
    let shared = top.wires().iter().find(|w| w.name == "shared").expect("shared wire");
    assert_eq!(shared.ty.discipline_name(), "disc_a");
}

/// Detail mode, same source: §7.4.4.2's own text — "the specified
/// resolveto connect statements are ignored in this mode" — so the exact
/// same `resolveto disc_a` rule that resolves Basic mode above is not
/// consulted; with no domain mismatch, no shared nature lineage, and no
/// plain connectrules bridge, elaboration fails loud naming Detail's own
/// method (T60 Done-when #1, discriminating: same source, same seed,
/// opposite outcome depending only on the selected mode).
#[test]
fn detail_mode_ignores_resolveto_and_fails_loud_on_the_same_discrete_pair() {
    let err = parse_and_elaborate_seeded(INCOMPATIBLE_DISCRETE_PAIR, &SourceMap::dummy(), |ctx| {
        ctx.discipline_resolution_mode = DisciplineResolutionMode::Detail;
    })
    .expect_err("Detail mode must not resolve this pair via resolveto");
    let msg = err.to_string();
    assert!(msg.contains("shared"), "names the conflicting net: {msg}");
    assert!(msg.contains("detail discipline resolution"), "names the Detail method: {msg}");
    assert!(msg.contains("7.4.4.2"), "cites the Detail clause: {msg}");
}

/// Continuous still wins over discrete in Detail mode exactly as in
/// Basic — the two modes only disagree on the `resolveto` path, not on
/// the domain-based continuous-over-discrete rule (§7.4.4.2's own
/// "continuous ... propagate up and then back down to meet discrete
/// disciplines" headline, still domain-based). Mirrors
/// `vams_annex_f_domain.rs`'s own Figure 7-3 reduction: `ResolveDisciplines`
/// resolving `netc` to `electrical` is proven by the *next* failure being
/// `InsertConnectModules`'s (no `connectrules` bridge for the resulting
/// cross-domain connection) rather than `ResolveDisciplines`'s own
/// `DisciplineResolutionFailed` — the same technique that file uses for
/// Basic mode, run here under Detail to prove the domain rule is
/// mode-independent.
#[test]
fn detail_mode_still_lets_continuous_win_over_discrete() {
    let src = "
        nature Voltage;
            units = \"V\";
            abstol = 1e-6;
            access = V;
        endnature
        nature Current;
            units = \"A\";
            abstol = 1e-12;
            access = I;
        endnature
        discipline electrical;
            domain continuous;
            potential Voltage;
            flow Current;
        enddiscipline
        discipline cmos_logic;
            domain discrete;
        enddiscipline

        module analog_drv (inout electrical p);
        endmodule
        module digital_recv (inout cmos_logic p);
        endmodule
        module top ();
            analog_drv a1(netc);
            digital_recv d1(netc);
        endmodule
    ";
    let msg = parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        ctx.discipline_resolution_mode = DisciplineResolutionMode::Detail;
    })
    .expect_err("no connectrules bridge declared — InsertConnectModules must still fail")
    .to_string();
    assert!(
        !msg.contains("incompatible disciplines"),
        "ResolveDisciplines must resolve netc to electrical under Detail too — got: {msg}"
    );
    assert!(
        msg.contains("E4005") || msg.contains("connect-module") || msg.contains("connectrules"),
        "the failure must be InsertConnectModules's, proving discipline resolution itself \
         succeeded under Detail mode: {msg}"
    );
}
