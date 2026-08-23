//! LRM-COVERS: 6.3.2
//!
//! LRMW2-43 (`lrm-conformance-wave2`, T49): §6.3.2 "Module instance
//! parameter value assignment by order" — `mod_name #(v0, v1, ...) inst
//! (...)` binds `v0, v1, ...` to the module's own declared, non-local
//! parameters in *declaration* order, alongside the already-implemented
//! named form (§6.3.3). Before this task `lower_instance` unconditionally
//! required every `#(...)` entry to carry a `.name(...)`, so a positional
//! override failed elaboration with a generic "must be named" error
//! instead of binding — the LRM's own §6.3.2 worked example
//! (`mosp #(2e-6,1e-6) weakp (out_a, in_a, clk);`, simplified here to a
//! self-contained fixture) is the primary fixture below.

use piperine_lang::pom::{ElabError, Value};
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// §6.3.2's own worked example structure: `mosp #(2e-6,1e-6) weakp (...)`
/// binds `gate_width` (declared first) and `gate_length` (declared
/// second) in that order — the LRM prints the module's parameters in the
/// opposite order from the override list on purpose, so a naive
/// name-based or reversed binding would fail this.
const FIXTURE: &str = r#"
    module mosp (inout electrical out, inout electrical in, inout electrical clk);
        parameter real gate_width = 4.0e-6;
        parameter real gate_length = 0.3e-6;
    endmodule

    module m ();
        electrical clk;
        electrical out_a, in_a;
        electrical out_b, in_b;

        // create an instance and set parameters
        mosp #(2e-6, 1e-6) weakp (out_a, in_a, clk);
        // create an instance leaving default values
        mosp plainp (out_b, in_b, clk);
    endmodule
"#;

#[test]
fn positional_overrides_bind_in_declaration_order() {
    let d = elab(FIXTURE);
    let m = d.module("m").expect("m elaborated");
    let weakp = m.instances.iter().find(|i| i.label.as_deref() == Some("weakp")).expect("instance weakp");
    assert_eq!(
        weakp.params.iter().find(|(n, _)| n == "gate_width").map(|(_, v)| v.clone()),
        Some(Value::Real(2.0e-6)),
        "the first positional override (2e-6) must bind to gate_width, the first-declared parameter"
    );
    assert_eq!(
        weakp.params.iter().find(|(n, _)| n == "gate_length").map(|(_, v)| v.clone()),
        Some(Value::Real(1.0e-6)),
        "the second positional override (1e-6) must bind to gate_length, the second-declared parameter"
    );
}

/// §6.3.2: "It is not necessary to assign values to all of the parameters
/// ... the left-most parameter assignment(s) can not be skipped" — an
/// instance with no override at all keeps every declared default.
#[test]
fn instance_with_no_override_keeps_declared_defaults() {
    let d = elab(FIXTURE);
    let m = d.module("m").expect("m elaborated");
    let plainp = m.instances.iter().find(|i| i.label.as_deref() == Some("plainp")).expect("instance plainp");
    assert!(plainp.params.is_empty(), "an instance with no `#(...)` override must record no param overrides");
}

/// §6.3.2's own text: a subset override only fills the left-most
/// parameters — `#(2e-6)` alone must bind only `gate_width`, leaving
/// `gate_length` at its declared default (never touching `params` for it).
#[test]
fn positional_subset_override_binds_only_the_leading_parameters() {
    let src = r#"
        module mosp (inout electrical out, inout electrical in, inout electrical clk);
            parameter real gate_width = 4.0e-6;
            parameter real gate_length = 0.3e-6;
        endmodule
        module m ();
            electrical out_a, in_a, clk;
            mosp #(2e-6) weakp (out_a, in_a, clk);
        endmodule
    "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    let weakp = m.instances.iter().find(|i| i.label.as_deref() == Some("weakp")).expect("instance weakp");
    assert_eq!(weakp.params.len(), 1, "only the leading positional override was given");
    assert_eq!(
        weakp.params.iter().find(|(n, _)| n == "gate_width").map(|(_, v)| v.clone()),
        Some(Value::Real(2.0e-6))
    );
}

/// A `localparam` is excluded from the positional binding order (§6.3.2:
/// "Local parameter declarations are not considered when assigning by
/// order") — with `gw_scale` declared `localparam` between the two
/// overridable parameters, a two-value positional override must still
/// bind to `gate_width`/`gate_length`, skipping over the localparam
/// entirely, not consuming a slot for it.
#[test]
fn localparam_is_skipped_in_positional_binding_order() {
    let src = r#"
        module mosp (inout electrical out, inout electrical in, inout electrical clk);
            parameter real gate_width = 4.0e-6;
            localparam real gw_scale = 1.0;
            parameter real gate_length = 0.3e-6;
        endmodule
        module m ();
            electrical out_a, in_a, clk;
            mosp #(2e-6, 1e-6) weakp (out_a, in_a, clk);
        endmodule
    "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    let weakp = m.instances.iter().find(|i| i.label.as_deref() == Some("weakp")).expect("instance weakp");
    assert_eq!(
        weakp.params.iter().find(|(n, _)| n == "gate_width").map(|(_, v)| v.clone()),
        Some(Value::Real(2.0e-6)),
        "gate_width (first non-local param) must get the first positional value"
    );
    assert_eq!(
        weakp.params.iter().find(|(n, _)| n == "gate_length").map(|(_, v)| v.clone()),
        Some(Value::Real(1.0e-6)),
        "gate_length (second non-local param) must get the second positional value, skipping the localparam"
    );
}

/// §6.3.2: too many positional overrides for the module's declared
/// parameter count fails loud citing the clause, never silently
/// truncating.
#[test]
fn too_many_positional_overrides_fails_loud_citing_6_3_2() {
    let src = r#"
        module mosp (inout electrical out, inout electrical in, inout electrical clk);
            parameter real gate_width = 4.0e-6;
            parameter real gate_length = 0.3e-6;
        endmodule
        module m ();
            electrical out_a, in_a, clk;
            mosp #(2e-6, 1e-6, 3e-6) weakp (out_a, in_a, clk);
        endmodule
    "#;
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("3 positional overrides against a 2-parameter module must fail elaboration (LRM §6.3.2)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    let msg = format!("{err}");
    assert!(msg.contains("6.3.2"), "expected the error to cite LRM §6.3.2, got: {msg}");
}

/// The LRM defines no combined form for one instance's `#(...)` list —
/// mixing a positional entry with a named entry fails loud rather than
/// guessing an interpretation.
#[test]
fn mixing_positional_and_named_overrides_fails_loud() {
    let src = r#"
        module mosp (inout electrical out, inout electrical in, inout electrical clk);
            parameter real gate_width = 4.0e-6;
            parameter real gate_length = 0.3e-6;
        endmodule
        module m ();
            electrical out_a, in_a, clk;
            mosp #(2e-6, .gate_length(1e-6)) weakp (out_a, in_a, clk);
        endmodule
    "#;
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("mixing positional and named overrides in one `#(...)` list must fail elaboration");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    let msg = format!("{err}");
    assert!(
        msg.contains("mix") || msg.contains("positional") || msg.contains("named"),
        "expected the error to name the positional/named mixing issue, got: {msg}"
    );
}
