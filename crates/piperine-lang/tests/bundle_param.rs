//! Tests for GAPS §I.14 — bundle-typed `param` field access.
//!
//! A `param model : XModel = XModel { .. };` is flattened at elaboration
//! into one scalar POM `Param` per bundle field, named `model_<field>`.
//! This matches `lowering/expr.rs`'s `Expr::Field` arm, which already turns
//! `model.rsh` into `IrExpr::Param("model_rsh")`.

use piperine_lang::{parse_str, parse_and_elaborate};

fn ppr_to_ir(design: &piperine_lang::Design) -> Result<std::collections::HashMap<String, piperine_codegen::resolve::LoweredBody>, piperine_codegen::resolve::LowerErrors> {
    piperine_codegen::resolve::lower_bodies(design)
}

const DISCIPLINE: &str = "
discipline Electrical { potential v : Real; flow i : Real; }
";

fn src(bundle: &str, mod_body: &str, analog_body: &str) -> String {
    format!(
        "{DISCIPLINE}
bundle ResModel {{
    {bundle}
}}
mod R(inout p: Electrical, inout n: Electrical) {{
    {mod_body}
}}
analog R {{
    {analog_body}
}}
"
    )
}

#[test]
fn bundle_param_default_flattens_to_scalar_params() {
    let s = src(
        "rsh: Real = 100.0, kf: Real = 0.0,",
        "param model : ResModel = ResModel {};",
        "I(p, n) <+ V(p, n) / model.rsh;",
    );
    let prog = parse_and_elaborate(&s, &piperine_lang::SourceMap::dummy()).expect("elab");
    let ir = ppr_to_ir(&prog).expect("lowering failed");
    let m = ir.get("R").expect("module");
    let rsh = m.symbols.params().map(|(_, p)| p).find(|p| p.name == "model_rsh").expect("model_rsh param");
    assert!(matches!(rsh.default, Some(piperine_lang::parse::ast::Expr::Literal(piperine_lang::parse::ast::Literal::Real(v))) if v == 100.0));
    assert!(m.symbols.params().map(|(_, p)| p).any(|p| p.name == "model_kf"));
}

#[test]
fn bundle_param_partial_literal_overrides_one_field() {
    let s = src(
        "rsh: Real = 100.0, kf: Real = 0.0,",
        "param model : ResModel = ResModel { .rsh = 5.0 };",
        "I(p, n) <+ V(p, n) / model.rsh;",
    );
    let prog = parse_and_elaborate(&s, &piperine_lang::SourceMap::dummy()).expect("elab");
    let ir = ppr_to_ir(&prog).expect("lowering failed");
    let m = ir.get("R").expect("module");
    let rsh = m.symbols.params().map(|(_, p)| p).find(|p| p.name == "model_rsh").expect("model_rsh param");
    assert!(matches!(rsh.default, Some(piperine_lang::parse::ast::Expr::Literal(piperine_lang::parse::ast::Literal::Real(v))) if v == 5.0));
    // Untouched field keeps the bundle's own default.
    let kf = m.symbols.params().map(|(_, p)| p).find(|p| p.name == "model_kf").expect("model_kf param");
    assert!(matches!(kf.default, Some(piperine_lang::parse::ast::Expr::Literal(piperine_lang::parse::ast::Literal::Real(v))) if v == 0.0));
}

#[test]
fn bundle_field_access_lowers_to_flattened_param() {
    let s = src(
        "rsh: Real = 100.0,",
        "param model : ResModel = ResModel {};",
        "I(p, n) <+ V(p, n) / model.rsh;",
    );
    let prog = parse_and_elaborate(&s, &piperine_lang::SourceMap::dummy()).expect("elab");
    let ir = ppr_to_ir(&prog).expect("lowering failed");
    let out = format!("{ir:?}");
    assert!(out.contains("model_rsh"), "output: {out}");
}

#[test]
fn instance_bundle_override_flattens_to_param_pairs() {
    let s = format!(
        "{DISCIPLINE}
bundle ResModel {{ rsh: Real = 100.0, kf: Real = 0.0, }}
mod R(inout p: Electrical, inout n: Electrical) {{
    param model : ResModel = ResModel {{}};
}}
analog R {{ I(p, n) <+ V(p, n) / model.rsh; }}
mod Top(inout p: Electrical, inout n: Electrical) {{
    r1 : R(p, n) {{ .model = ResModel {{ .rsh = 50.0 }} }};
}}
"
    );
    let prog = parse_and_elaborate(&s, &piperine_lang::SourceMap::dummy()).expect("elab");
    let top = prog.modules().find(|m| m.name == "Top").expect("Top module");
    let inst = &top.instances[0];
    assert!(
        inst.params.iter().any(|(n, _)| n == "model_rsh"),
        "expected flattened model_rsh override, got {:?}", inst.params
    );
}

#[test]
fn bundle_param_unknown_field_in_literal_fails_loud() {
    let s = src(
        "rsh: Real = 100.0,",
        "param model : ResModel = ResModel { .nonexistent = 1.0 };",
        "I(p, n) <+ V(p, n) / model.rsh;",
    );
    let err = parse_str(&s).expect("parse").elaborate(&piperine_lang::SourceMap::dummy()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("nonexistent"), "error: {msg}");
}

#[test]
fn bundle_param_field_without_default_fails_loud() {
    let s = src(
        "rsh: Real,",
        "param model : ResModel = ResModel {};",
        "I(p, n) <+ V(p, n) / model.rsh;",
    );
    let err = parse_str(&s).expect("parse").elaborate(&piperine_lang::SourceMap::dummy()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("rsh"), "error: {msg}");
}
