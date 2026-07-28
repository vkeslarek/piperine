//! dv-core T10 (DVC-09) — pointwise/reduced classification: every `var`/
//! `target` carries its class in the POM; a `require` or a pointwise
//! `target` reading a reduced quantity fails loud with the spec's
//! wording; a mixed expression classifies reduced; and the analysis a
//! helper belongs to is recorded for later cross-analysis refusal
//! (DVC-08 AC6).

use piperine_lang::pom::QuantityClass;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &SourceMap::dummy())
}

fn elab_err(src: &str) -> String {
    elaborate(src).map(|_| ()).expect_err("fixture must fail elaboration").to_string()
}

const SPEC_WORDING: &str = "a reduced quantity is checkable once per analysis, not per point";

/// DVC-09: every `var`/`target` carries its class in the POM — pointwise
/// for probe/helper-of-the-moment quantities, reduced for the declared
/// reduced helpers — and a `require` classifies pointwise.
#[test]
fn vars_and_targets_carry_their_class_in_the_pom() {
    let design = elaborate(
        "mod Amp ( inout inp : Electrical, inout out : Electrical, inout gnd : Electrical ) {\n\
             param gain_min : Real = 1.0;\n\
         }\n\
         constraint Amp {\n\
             var g : Real = ac_gain(V(out, gnd));\n\
             var pm : Real = ac_phase_margin(V(out, gnd));\n\
             target ac_gain(V(out, gnd)) >= gain_min;\n\
             target ac_unity_gain_freq(V(out, gnd)) >= 10e6 tol 1e6;\n\
             require no_clip : V(out, gnd) <= 3.3;\n\
         }",
    )
    .expect("classified block must elaborate");
    let block = &design.module("Amp").expect("Amp elaborated").constraints()[0];

    let leaves: Vec<_> = block.leaves().collect();
    assert_eq!(leaves.len(), 5, "one class entry per leaf statement");
    let class_at = |i: usize| leaves[i].1;
    assert_eq!(block.class_of("g").map(|c| c.class), Some(QuantityClass::Pointwise));
    assert_eq!(block.class_of("pm").map(|c| c.class), Some(QuantityClass::Reduced));
    // Targets are unlabeled — read them positionally by leaf index.
    assert_eq!(class_at(2).class, QuantityClass::Pointwise, "a pointwise target");
    assert_eq!(class_at(3).class, QuantityClass::Reduced, "a reduced target");
    assert_eq!(block.class_of("no_clip").map(|c| c.class), Some(QuantityClass::Pointwise));
}

/// DVC-09 AC3: a `require` reading a reduced helper fails loud with the
/// spec's wording.
#[test]
fn require_reading_a_reduced_helper_fails_loud() {
    let msg = elab_err(
        "mod Amp ( inout out : Electrical, inout gnd : Electrical ) { }\n\
         constraint Amp {\n\
             require stable : ac_phase_margin(V(out, gnd)) >= 45.0;\n\
         }",
    );
    assert!(msg.contains(SPEC_WORDING), "error carries the spec's wording: {msg}");
    assert!(msg.contains("require `stable`"), "error names the use site: {msg}");
}

/// DVC-09 AC3: a `require` reading a reduced **`var`** fails loud with
/// the same wording — the class propagates through the block's var table.
#[test]
fn require_reading_a_reduced_var_fails_loud() {
    let msg = elab_err(
        "mod Amp ( inout out : Electrical, inout gnd : Electrical ) { }\n\
         constraint Amp {\n\
             var pm : Real = ac_phase_margin(V(out, gnd));\n\
             require stable : pm >= 45.0;\n\
         }",
    );
    assert!(msg.contains(SPEC_WORDING), "error carries the spec's wording: {msg}");
}

/// DVC-09 AC3: a pointwise `target` whose **level** reads a reduced
/// quantity fails loud with the same wording — the level must exist per
/// point just like the operand.
#[test]
fn pointwise_target_with_a_reduced_level_fails_loud() {
    let msg = elab_err(
        "mod Amp ( inout out : Electrical, inout gnd : Electrical ) { }\n\
         constraint Amp {\n\
             target ac_gain(V(out, gnd)) >= ac_unity_gain_freq(V(out, gnd));\n\
         }",
    );
    assert!(msg.contains(SPEC_WORDING), "error carries the spec's wording: {msg}");
}

/// DVC-09: a mixed expression — pointwise arithmetic over a reduced
/// value — classifies reduced, not pointwise.
#[test]
fn mixed_expression_classifies_reduced() {
    let design = elaborate(
        "mod Amp ( inout out : Electrical, inout gnd : Electrical ) { }\n\
         constraint Amp {\n\
             var mixed : Real = ac_gain(V(out, gnd)) + ac_phase_margin(V(out, gnd));\n\
         }",
    )
    .expect("a mixed expression is legal as a reduced var");
    let block = &design.module("Amp").expect("Amp elaborated").constraints()[0];
    assert_eq!(
        block.class_of("mixed").map(|c| c.class),
        Some(QuantityClass::Reduced),
        "pointwise arithmetic over a reduced value is reduced (design §C2)"
    );
}

/// DVC-08 AC6 (recording half): the analysis a statement's helpers belong
/// to is recorded on the statement — a probe-and-constant quantity
/// records no analysis (readable everywhere), an `ac_*` helper records
/// `ac`, and a mixed statement unions them.
#[test]
fn the_analysis_a_helper_belongs_to_is_recorded() {
    let design = elaborate(
        "mod Amp ( inout out : Electrical, inout gnd : Electrical ) { }\n\
         constraint Amp {\n\
             var plain : Real = V(out, gnd) * 2.0;\n\
             var g : Real = ac_gain(V(out, gnd));\n\
             var pm : Real = ac_phase_margin(V(out, gnd));\n\
         }",
    )
    .expect("fixture must elaborate");
    let block = &design.module("Amp").expect("Amp elaborated").constraints()[0];

    assert_eq!(
        block.class_of("plain").map(|c| c.analyses.as_slice()),
        Some(&[][..]),
        "a probe-and-constant quantity belongs to no single analysis"
    );
    assert_eq!(
        block.class_of("g").map(|c| c.analyses.as_slice()),
        Some(&["ac".to_string()][..]),
        "an ac helper records the ac analysis"
    );
    assert_eq!(
        block.class_of("pm").map(|c| c.analyses.as_slice()),
        Some(&["ac".to_string()][..])
    );
}
