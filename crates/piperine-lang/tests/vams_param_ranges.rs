//! VAMS T24 (VAMS-07, R6) — the elaboration-constant evaluator retarget
//! (`elab::const_eval::ConstEnv`, self-contained since T21) applied to
//! `parameter` value ranges (§3.4.2) and `localparam`/`aliasparam`
//! (§3.4.3/§3.4.7).

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

#[test]
fn default_within_a_from_range_elaborates() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 from [1.0:1.0e6];
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.params()[0].default, Some(piperine_lang::pom::Value::Real(1.0e3)));
}

#[test]
fn default_outside_the_declared_from_range_fails_loud_naming_param_and_value() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 0.5 from [1.0:1.0e6];
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains('r'), "names the parameter: {msg}");
    assert!(msg.contains("0.5"), "names the offending value: {msg}");
    assert!(msg.contains("3.4.2"), "cites the LRM clause: {msg}");
}

#[test]
fn default_matching_an_excluded_value_fails_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 0.0 exclude 0.0;
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains('r'));
    assert!(msg.contains("exclude"), "names the exclude reason: {msg}");
}

#[test]
fn default_outside_an_excluded_value_elaborates() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0 exclude 0.0;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.params()[0].default, Some(piperine_lang::pom::Value::Real(1.0)));
}

#[test]
fn localparam_is_marked_non_overridable() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            localparam real scale = 2.0;
            parameter real r = 1.0;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let scale = m.params().iter().find(|p| p.name == "scale").expect("scale param");
    assert!(scale.is_local(), "localparam sets is_local");
    let r = m.params().iter().find(|p| p.name == "r").expect("r param");
    assert!(!r.is_local(), "parameter (non-local) stays false");
}

#[test]
fn aliasparam_lowers_with_its_target_name() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0;
            aliasparam ralias = r;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let alias = m.params().iter().find(|p| p.name == "ralias").expect("ralias param");
    assert_eq!(alias.bundle_origin, Some(("aliasparam".to_string(), "r".to_string())));
}

// ── LRMC-C1-07: aliasparam inherits the aliased parameter's type ───────

#[test]
fn aliasparam_over_an_integer_parameter_carries_integer_type() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter integer n_fingers = 4;
            aliasparam fingers = n_fingers;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let alias = m.params().iter().find(|p| p.name == "fingers").expect("fingers param");
    assert_eq!(alias.ty, piperine_lang::pom::ValueType::Integer);
}

#[test]
fn aliasparam_over_a_string_parameter_carries_string_type() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter string kind = "NPN";
            aliasparam device_kind = kind;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let alias = m.params().iter().find(|p| p.name == "device_kind").expect("device_kind param");
    assert_eq!(alias.ty, piperine_lang::pom::ValueType::Str);
}

#[test]
fn aliasparam_over_a_real_parameter_still_carries_real_type() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0;
            aliasparam ralias = r;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let alias = m.params().iter().find(|p| p.name == "ralias").expect("ralias param");
    assert_eq!(alias.ty, piperine_lang::pom::ValueType::Real);
}

#[test]
fn aliasparam_of_an_undeclared_target_fails_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            aliasparam ralias = nonexistent;
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("nonexistent"), "names the undeclared target: {msg}");
}

// ── override-site validation (§3.4.2: a range constrains every value
//    the parameter takes, not just the declared default) ────────────────

#[test]
fn override_outside_the_declared_from_range_fails_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 from [1.0:1.0e6];
        endmodule
        module top ();
            electrical a, b;
            res #(.r(0.5)) r1 (.p(a), .n(b));
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains('r'), "names the parameter: {msg}");
    assert!(msg.contains("0.5"), "names the offending value: {msg}");
    assert!(msg.contains("3.4.2"), "cites the LRM clause: {msg}");
}

#[test]
fn override_within_the_declared_from_range_elaborates_with_the_value() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 from [1.0:1.0e6];
        endmodule
        module top ();
            electrical a, b;
            res #(.r(2.0e3)) r1 (.p(a), .n(b));
        endmodule
    "#;
    let d = elab(src);
    let top = d.module("top").expect("top elaborated");
    let r1 = top.instances.iter().find(|i| i.label.as_deref() == Some("r1")).expect("instance r1");
    assert!(
        r1.params.iter().any(|(n, v)| n == "r" && *v == piperine_lang::pom::Value::Real(2.0e3)),
        "the override value travels: {:?}",
        r1.params
    );
}

#[test]
fn override_matching_an_excluded_value_fails_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 exclude 0.0;
        endmodule
        module top ();
            electrical a, b;
            res #(.r(0.0)) r1 (.p(a), .n(b));
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("exclude"), "names the exclude reason: {msg}");
}

#[test]
fn paramset_statement_outside_the_module_range_fails_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 from [1.0:1.0e6];
        endmodule
        paramset rcard (res);
            parameter real rnom = 1.0e3;
            .r = 0.5;
        endparamset
        module top ();
            electrical a, b;
            rcard r1 (.p(a), .n(b));
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains('r'), "names the module parameter: {msg}");
    assert!(msg.contains("0.5"), "names the offending value: {msg}");
}

#[test]
fn paramset_override_outside_the_paramsets_own_range_fails_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 from [1.0:1.0e6];
        endmodule
        paramset rcard (res);
            parameter real r = 1.0e3 from [1.0:1.0e6];
            .r = r;
        endparamset
        module top ();
            electrical a, b;
            rcard #(.r(0.5)) r1 (.p(a), .n(b));
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains('r'), "names the paramset parameter: {msg}");
    assert!(msg.contains("0.5"), "names the offending value: {msg}");
}
