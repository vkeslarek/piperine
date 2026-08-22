//! LRM-COVERS: 3.11.1
//! LRMW2-24 (`lrm-conformance-wave2`, T30): §3.11.1 discipline
//! compatibility follows nature derivation family + units, not discipline
//! *name* equality — `ResolveDisciplines`'s hard-fail fallback now checks
//! `disciplines_nature_compatible` (Base Nature Rule, Derived Nature Rule,
//! Units Value Rule) before declaring two differently-named,
//! same-domain disciplines incompatible. Fixtures reproduce §3.11.1's own
//! worked example verbatim: `nature highvoltage : Voltage;` (a derived
//! nature), `discipline highvolt` binding `potential highvoltage;`, and
//! `discipline electrical` binding `potential Voltage;` — the Base Nature
//! Rule makes the two disciplines compatible even though their names and
//! their bound potential natures' *names* both differ.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// §3.11.1's own worked example: `nature highvoltage: Voltage;` — a
/// nature derived from `Voltage` — bound as the potential of
/// `discipline highvolt`, alongside the ordinary `discipline electrical`
/// binding `potential Voltage;` directly. Both `flow Current;`.
const LRM_EXAMPLE: &str = r#"
    nature Voltage;
        access = V;
        units = "V";
        abstol = 1e-6;
    endnature
    nature Current;
        access = I;
        units = "A";
        abstol = 1e-12;
    endnature
    nature highvoltage: Voltage;
        abstol = 1;
    endnature
    discipline electrical;
        domain continuous;
        potential Voltage;
        flow Current;
    enddiscipline
    discipline highvolt;
        domain continuous;
        potential highvoltage;
        flow Current;
    enddiscipline
"#;

/// AC (T30 Done-when #1): a discipline derived from `Voltage` (`highvolt`,
/// via its bound `highvoltage` nature) connects to `electrical` — the two
/// disciplines have different names and different-named potential
/// natures, but the Base Nature Rule (`highvoltage` is derived from
/// `Voltage`) makes them compatible. Elaboration must succeed, not hit
/// `ResolveDisciplines`'s hard-fail fallback.
#[test]
fn discipline_derived_from_voltage_connects_to_electrical() {
    let src = format!(
        "{LRM_EXAMPLE}
        module drv_hv (inout highvolt p);
        endmodule
        module drv_e (inout electrical p);
        endmodule
        module top ();
            drv_hv h1(shared);
            drv_e e1(shared);
        endmodule
        "
    );
    let d = elab(&src);
    // MD-25: elaboration succeeding (no DisciplineResolutionFailed) is
    // itself the asserted outcome, but also confirm the authored instance
    // structure survived untouched — no synthetic discipline rewrite.
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances.len(), 2, "both instances present, unmodified by compatibility resolution");
}

/// Order-independence companion: swapping which side is `electrical` vs
/// `highvolt` must not change the outcome (the compatibility check is
/// symmetric, per §3.11.1's own rule set having no directionality).
#[test]
fn discipline_derived_from_voltage_connects_to_electrical_regardless_of_order() {
    let src = format!(
        "{LRM_EXAMPLE}
        module drv_e (inout electrical p);
        endmodule
        module drv_hv (inout highvolt p);
        endmodule
        module top ();
            drv_e e1(shared);
            drv_hv h1(shared);
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances.len(), 2);
}

/// Derived Nature Rule: two natures both derived from the same base
/// (neither is the other's direct base) are compatible with each other.
#[test]
fn two_natures_derived_from_the_same_base_are_compatible() {
    let src = r#"
        nature Voltage;
            access = V;
            units = "V";
            abstol = 1e-6;
        endnature
        nature highvoltage_a: Voltage;
            abstol = 1;
        endnature
        nature highvoltage_b: Voltage;
            abstol = 2;
        endnature
        discipline disc_a;
            domain continuous;
            potential highvoltage_a;
        enddiscipline
        discipline disc_b;
            domain continuous;
            potential highvoltage_b;
        enddiscipline
        module drv_a (inout disc_a p);
        endmodule
        module drv_b (inout disc_b p);
        endmodule
        module top ();
            drv_a a1(shared);
            drv_b b1(shared);
        endmodule
    "#;
    let d = elab(src);
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances.len(), 2, "siblings derived from the same base nature are compatible");
}

/// Units Value Rule: two *unrelated* natures (no shared base) sharing the
/// same declared `units` are still compatible.
#[test]
fn two_unrelated_natures_with_the_same_units_are_compatible() {
    let src = r#"
        nature Voltage;
            access = V;
            units = "V";
            abstol = 1e-6;
        endnature
        nature Potential2;
            access = V2;
            units = "V";
            abstol = 1e-6;
        endnature
        discipline disc_a;
            domain continuous;
            potential Voltage;
        enddiscipline
        discipline disc_b;
            domain continuous;
            potential Potential2;
        enddiscipline
        module drv_a (inout disc_a p);
        endmodule
        module drv_b (inout disc_b p);
        endmodule
        module top ();
            drv_a a1(shared);
            drv_b b1(shared);
        endmodule
    "#;
    let d = elab(src);
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances.len(), 2, "same units, unrelated lineage: still compatible per the Units Value Rule");
}

/// Negative case (spec Edge Case, required): two same-domain disciplines
/// whose potential natures share neither a base nor units are genuinely
/// incompatible — still fails loud, exactly as the pre-T30 name-equality
/// check did for this genuinely-differing pair.
#[test]
fn genuinely_unrelated_natures_still_fail_loud() {
    let src = r#"
        nature Voltage;
            access = V;
            units = "V";
            abstol = 1e-6;
        endnature
        nature Position;
            access = X;
            units = "m";
            abstol = 1e-6;
        endnature
        discipline electrical;
            domain continuous;
            potential Voltage;
        enddiscipline
        discipline mechanical;
            domain continuous;
            potential Position;
        enddiscipline
        module drv_e (inout electrical p);
        endmodule
        module drv_m (inout mechanical p);
        endmodule
        module top ();
            drv_e e1(shared);
            drv_m m1(shared);
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("shared"), "names the conflicting net: {msg}");
    assert!(msg.contains("incompatible disciplines"), "still reports genuine incompatibility: {msg}");
}
