//! `Design::introspection_meta` — the POM-side resolver for the
//! device-introspection metadata sidecar (phdl-introspection-attributes T2).
//! Mirrors `rfport.rs`'s coverage shape for `Design::rfports`: each spec AC
//! for the resolve layer (placement matrix, `@kind` enum membership per
//! placement, duplicate `@name`, attribute-absent fallback) gets a dedicated
//! test. The codegen-bridge ACs (PIA-01/05/06/10/11) are exercised end-to-end
//! in `crates/piperine-codegen/tests/`; this file is the validation gate.

use piperine_lang::{
    parse_and_elaborate,
    pom::{IntrospectionMeta, ModelId, VAR_KINDS, TERMINAL_KINDS},
    SourceMap,
};

fn elab(src: &str) -> piperine_lang::pom::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).expect("PHDL parses + elaborates")
}
fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration to fail loud")
        .to_string()
}
fn meta(src: &str, module: &str) -> IntrospectionMeta {
    let design = elab(src);
    design.introspection_meta(module).expect("introspection_meta resolves")
}
fn resolve_err(src: &str, module: &str) -> String {
    let design = elab(src);
    design
        .introspection_meta(module)
        .expect_err("expected introspection_meta to fail loud")
        .to_string()
}

const DISCIPLINE: &str = "discipline Electrical { potential v: Real; flow i: Real; }";

// ── PIA-01: @model populates meta.model ────────────────────────────────────

#[test]
fn at_model_populates_model_id() {
    let src = format!(
        "
        {DISCIPLINE}
        @model(type = \"mos\", version = \"3\")
        mod M ( inout p : Electrical ) {{ }}
        "
    );
    let m = meta(&src, "M");
    assert_eq!(m.model, Some(ModelId { type_id: "mos".into(), version: "3".into() }));
}

#[test]
fn at_model_absent_yields_no_model_id() {
    // PIA-02 (resolve half): a module with no @model has None — codegen then
    // falls back to the module-name echo (asserted in codegen's model_descriptor tests).
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ }}
        "
    );
    let m = meta(&src, "M");
    assert!(m.model.is_none());
}

// ── PIA-05/08: var @unit/@description resolve; absent → empty ──────────────

#[test]
fn var_unit_and_description_resolve() {
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{
            @unit(value = \"S\") @description(value = \"transconductance\")
            var gm : Real;
        }}
        "
    );
    let m = meta(&src, "M");
    let gm = m.vars.get("gm").expect("gm var present in sidecar");
    assert_eq!(gm.unit.as_deref(), Some("S"));
    assert_eq!(gm.description.as_deref(), Some("transconductance"));
    assert!(gm.name.is_none() && gm.kind.is_none());
}

#[test]
fn var_without_attrs_not_in_sidecar() {
    // Sparse sidecar: a var with no introspection attrs is absent (codegen
    // falls back to the kernel-derived default, PIA-08).
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ var gm : Real; }}
        "
    );
    let m = meta(&src, "M");
    assert!(m.vars.is_empty() && m.terminals.is_empty() && m.model.is_none());
}

// ── PIA-14: @kind placement-resolution (var→ObservableKind, terminal→TerminalKind)

#[test]
fn at_kind_on_var_accepts_observable_kind_values_case_insensitive() {
    for v in VAR_KINDS {
        let src = format!(
            "
            {DISCIPLINE}
            mod M ( inout p : Electrical ) {{ @kind(value = \"{v}\") var x : Real; }}
            "
        );
        let m = meta(&src, "M");
        assert_eq!(m.vars.get("x").unwrap().kind.as_deref(), Some(*v), "lowercase {v} should resolve");
    }
    // Case-insensitive: "State" canonicalizes to "state".
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ @kind(value = \"State\") var x : Real; }}
        "
    );
    let m = meta(&src, "M");
    assert_eq!(m.vars.get("x").unwrap().kind.as_deref(), Some("state"));
}

#[test]
fn at_kind_on_terminal_accepts_terminal_kind_values() {
    for k in TERMINAL_KINDS {
        let src = format!(
            "
            {DISCIPLINE}
            mod M ( inout p : Electrical ) {{ @kind(value = \"{k}\") wire w : Electrical; }}
            "
        );
        let m = meta(&src, "M");
        assert_eq!(m.terminals.get("w").unwrap().kind.as_deref(), Some(*k));
    }
}

// ── PIA-09: @kind on var rejects non-ObservableKind ────────────────────────

#[test]
fn at_kind_on_var_with_terminal_value_fails_loud() {
    // "auxiliary" is a valid TerminalKind but NOT an ObservableKind — placement
    // selects the enum, so a terminal value on a var fails (PIA-09/14).
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ @kind(value = \"auxiliary\") var x : Real; }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("kind"), "error should name the kind field: {err}");
    assert!(err.contains("auxiliary"), "error should name the offending value: {err}");
}

#[test]
fn at_kind_on_var_with_bogus_value_fails_loud() {
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ @kind(value = \"Bogus\") var x : Real; }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("Bogus"), "error should name the offending value: {err}");
}

// ── PIA-13: @kind on terminal rejects non-TerminalKind ─────────────────────

#[test]
fn at_kind_on_terminal_with_var_value_fails_loud() {
    // "state" is a valid ObservableKind but NOT a TerminalKind.
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ @kind(value = \"state\") wire w : Electrical; }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("state"), "error should name the offending value: {err}");
}

// ── PIA-03/19: placement errors ────────────────────────────────────────────

#[test]
fn at_model_on_var_fails_loud() {
    // PIA-03: @model targets a module; on a var it is misplaced.
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{
            @model(type = \"mos\", version = \"3\") var v : Real;
        }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("model"), "error should name the misplaced schema: {err}");
    assert!(err.contains("var"), "error should name the node kind: {err}");
}

#[test]
fn at_unit_on_module_fails_loud() {
    // PIA-19: @unit targets a var; on a module it is misplaced.
    let src = format!(
        "
        {DISCIPLINE}
        @unit(value = \"S\")
        mod M ( inout p : Electrical ) {{ }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("unit"), "error should name the misplaced schema: {err}");
    assert!(err.contains("module"), "error should name the node kind: {err}");
}

#[test]
fn at_unit_on_port_fails_loud() {
    // @unit targets a var; on a port it is misplaced.
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( @unit(value = \"S\") inout p : Electrical ) {{ }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("unit") && err.contains("port"), "error should name schema+node: {err}");
}

#[test]
fn any_introspection_attr_on_param_fails_loud() {
    // PIA-19: none of the five introspection schemas target a param. Each is
    // a placement error on a param.
    for schema in ["model", "name", "unit", "description", "kind"] {
        let arg = if schema == "model" {
            "(type = \"x\", version = \"1\")".to_string()
        } else {
            "(value = \"x\")".to_string()
        };
        let src = format!(
            "
            {DISCIPLINE}
            mod M ( inout p : Electrical ) {{ @{schema}{arg} param r : Real = 1.0; }}
            "
        );
        let err = resolve_err(&src, "M");
        assert!(
            err.contains(schema) && err.contains("param"),
            "@{schema} on a param must fail loud naming schema+param: {err}"
        );
    }
}

// ── PIA-20: duplicate @name on vars fails loud ─────────────────────────────

#[test]
fn duplicate_at_name_on_vars_fails_loud() {
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{
            @name(value = \"i\") var a : Real;
            @name(value = \"i\") var b : Real;
        }}
        "
    );
    let err = resolve_err(&src, "M");
    assert!(err.contains("duplicate"), "error should flag the duplicate: {err}");
    assert!(err.contains("\"i\"") || err.contains('`') , "error should name the colliding value: {err}");
}

// ── PIA-11: @name/@kind on internal wire ───────────────────────────────────

#[test]
fn internal_wire_name_and_kind_resolve() {
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{
            @name(value = \"cp\") @kind(value = \"internal\") wire cp : Electrical;
        }}
        "
    );
    let m = meta(&src, "M");
    let cp = m.terminals.get("cp").expect("cp wire present in sidecar");
    assert_eq!(cp.name.as_deref(), Some("cp"));
    assert_eq!(cp.kind.as_deref(), Some("internal"));
}

// ── Unknown module ─────────────────────────────────────────────────────────

#[test]
fn unknown_module_fails_loud() {
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ }}
        "
    );
    let err = resolve_err(&src, "DoesNotExist");
    assert!(err.contains("DoesNotExist"), "error should name the module: {err}");
}

// ── A non-introspection attribute (e.g. @rfport) is ignored, not rejected ──

#[test]
fn non_introspection_attribute_is_ignored() {
    // The resolver only looks at the 5 introspection schemas; @rfport on a wire
    // must not trip a placement error here (it has its own resolver, rfports()).
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{
            @rfport(num = 1, z0 = 50) wire rf : Electrical;
        }}
        "
    );
    let m = meta(&src, "M");
    assert!(m.is_empty(), "@rfport must not appear in the introspection sidecar: {m:?}");
}

// ── A schema-field type error is still raised by the shared elab path ──────

#[test]
fn bad_field_type_fails_loud_at_elaboration() {
    // @unit(value = 5) — value is declared String; a Nat fails the shared
    // convert_attribute type check at elaboration (before introspection_meta).
    let src = format!(
        "
        {DISCIPLINE}
        mod M ( inout p : Electrical ) {{ @unit(value = 5) var x : Real; }}
        "
    );
    let err = elab_err(&src);
    assert!(err.contains("value"), "error should name the field: {err}");
}
