//! LRM-COVERS: 3.8, 7.4.4.1, 3.11.1, 7.4.3, 7.4.5
//! Wave-2 (T69) re-reading, no new tests (added by this pass): §7.4.3
//! "Connection of continuous-time disciplines" ("It shall be an error to
//! connect incompatible continuous disciplines together") and §7.4.5
//! "Discipline resolution of continuous signals" (resolution among nets
//! that are all continuous, "there are no discrete disciplines to
//! propagate upwards") are exactly `same_domain_incompatible_
//! disciplines_still_hard_fail`'s own fixture below — two same-domain
//! (continuous) disciplines, `electrical` and `other_continuous`.
//! §3.8 "Default discipline"
//! states the `` `default_discipline `` directive "specifies a default
//! discrete discipline to be applied to any discrete net which does not
//! have an explicit discipline declaration as part of discipline
//! resolution" — `unresolved_net_falls_back_to_default_discipline_
//! directive` is exactly this (an otherwise-unresolvable net actually
//! resolves through the in-force directive), paired with `unresolved_net_
//! without_default_discipline_still_fails_loud` proving the fallback is
//! consulted only when present.
//! Annex F §7.4.4.1's continuous-over-discrete resolution rule and the
//! `` `default_discipline `` fallback (LRMC-C10-18/19) — `ResolveDisciplines`
//! reads each conflicting discipline's declared domain instead of
//! hard-failing on any mismatch. Fixtures are minimal reproductions of the
//! LRM's own Figure 7-3 `NetC`/`NetD` worked example ("`NetC` resolves to
//! electrical based on continuous (electrical) winning over discrete
//! (`cmos2`)").
//!
//! LRMW2-24 (`lrm-conformance-wave2`, T30): `same_domain_incompatible_
//! disciplines_still_hard_fail` doubles as the required §3.11.1 negative
//! case — a genuinely nature-incompatible same-domain pair still errors.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const NATURES_AND_DISCIPLINES: &str = "
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
";

/// LRM Figure 7-3's own worked example, reduced to its essential shape: an
/// undeclared net connects an `electrical` (continuous) driver and a
/// `cmos_logic` (discrete) receiver. `ResolveDisciplines` itself must now
/// resolve the net to `electrical` (continuous winning over discrete)
/// instead of hard-failing E4003 — proven here by the fact that
/// elaboration's *next* failure is `InsertConnectModules`'s E4005 ("no
/// `connectrules` rule"), which can only be reached once discipline
/// resolution has already succeeded and produced a real, still-mixed-domain
/// connection needing a connect module (LRMC-C7-01, a separate, later
/// task's scope — this fixture declares no `connectrules`, so that step
/// fails as expected; what matters here is *which* pass fails and why).
#[test]
fn continuous_wins_over_discrete_matching_figure_7_3() {
    let src = format!(
        "{NATURES_AND_DISCIPLINES}
        module analog_drv (inout electrical p);
        endmodule
        module digital_recv (inout cmos_logic p);
        endmodule
        module top ();
            analog_drv a1(netc);
            digital_recv d1(netc);
        endmodule
        "
    );
    let msg = elab_err(&src);
    assert!(
        !msg.contains("incompatible disciplines"),
        "ResolveDisciplines must no longer hard-fail this cross-domain pair — got: {msg}"
    );
    assert!(
        msg.contains("E4005") || msg.contains("connect-module") || msg.contains("connectrules"),
        "the next failure must be InsertConnectModules's (proving discipline resolution itself \
         succeeded and moved past ResolveDisciplines), got: {msg}"
    );
}

/// The same rule, discrete-then-continuous connection order (`NetD`'s own
/// shape in the LRM figure) — order must not matter.
#[test]
fn continuous_wins_regardless_of_connection_order() {
    let src = format!(
        "{NATURES_AND_DISCIPLINES}
        module digital_recv (inout cmos_logic p);
        endmodule
        module analog_drv (inout electrical p);
        endmodule
        module top ();
            digital_recv d1(netd);
            analog_drv a1(netd);
        endmodule
        "
    );
    let msg = elab_err(&src);
    assert!(
        !msg.contains("incompatible disciplines"),
        "ResolveDisciplines must no longer hard-fail this cross-domain pair regardless of \
         connection order — got: {msg}"
    );
    assert!(
        msg.contains("E4005") || msg.contains("connect-module") || msg.contains("connectrules"),
        "the next failure must be InsertConnectModules's, got: {msg}"
    );
}

/// Spec Edge Case: two *same-domain* incompatible disciplines still
/// hard-fail exactly as today — the fix narrows the false-positive case
/// (cross-domain), it does not loosen genuine same-domain incompatibility
/// detection.
///
/// LRMW2-24 (T30) re-verification note: this fixture originally bound
/// `other_continuous` to the *same* `Voltage`/`Current` natures as
/// `electrical`, differing only in discipline *name* — under name-equality
/// compatibility that correctly hard-failed, but per §3.11.1's actual
/// rules (Self Rule for Nature: identical bound natures ARE compatible
/// regardless of the disciplines' own names) that pair is not genuinely
/// incompatible at all. `other_continuous` now binds an unrelated
/// `Position` nature (no shared base, no shared `units` with `Voltage`)
/// so this stays a real §3.11.1 negative case after T30's fix.
#[test]
fn same_domain_incompatible_disciplines_still_hard_fail() {
    let src = format!(
        "{NATURES_AND_DISCIPLINES}
        nature Position;
            units = \"m\";
            abstol = 1e-6;
            access = X;
        endnature
        discipline other_continuous;
            domain continuous;
            potential Position;
            flow Current;
        enddiscipline
        module drv_e (inout electrical p);
        endmodule
        module drv_o (inout other_continuous p);
        endmodule
        module top ();
            drv_e e1(shared);
            drv_o o1(shared);
        endmodule
        "
    );
    let msg = elab_err(&src);
    assert!(msg.contains("shared"), "names the conflicting net: {msg}");
    assert!(
        msg.contains("incompatible disciplines"),
        "still reports the same-domain incompatibility, not a silent resolution: {msg}"
    );
}

/// LRMC-C10-19: an unresolved net (no connection at all supplies a
/// discipline) consults an in-force `` `default_discipline `` directive
/// before being declared unknown.
#[test]
fn unresolved_net_falls_back_to_default_discipline_directive() {
    let src = format!(
        "`default_discipline cmos_logic\n{NATURES_AND_DISCIPLINES}
        module top (inout electrical a);
            inout floating;
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let floating = top.ports().iter().find(|p| p.name() == "floating").expect("floating port");
    assert_eq!(floating.net_type().discipline_name(), "cmos_logic");
}

/// Without a `` `default_discipline `` directive, a fully unresolved net
/// still hard-fails exactly as before (regression guard for the
/// pre-existing `fully_unconnected_undeclared_net_fails_loud_naming_the_method`
/// behavior in `vams_discipline_resolution.rs`).
#[test]
fn unresolved_net_without_default_discipline_still_fails_loud() {
    let src = "
        module top (inout electrical a);
            inout floating;
        endmodule
    ";
    let msg = elab_err(src);
    assert!(msg.contains("floating"), "names the unresolved net: {msg}");
    assert!(msg.contains("basic discipline resolution"), "names the Annex F method: {msg}");
}
