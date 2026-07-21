//! Grammar tests for the six `extern`-modified declaration forms
//! (`.specs/features/declared-language-surface/spec.md` P2, DLS-08..14).
//!
//! These are pure-parse tests (`piperine_lang::parse_str`) — the elaborator
//! does not yet consult `extern` declarations (that's a later phase), so
//! these tests only assert the AST shape and spans the parser produces.

use piperine_lang::parse::ast::{ExternDecl, Item};
use piperine_lang::parse_str;

/// Extracts the single `ExternDecl` from a source that declares exactly one
/// top-level item, panicking with a useful message otherwise.
fn parse_one_extern(src: &str) -> ExternDecl {
    let file = parse_str(src).expect("expected source to parse");
    assert_eq!(file.items.len(), 1, "expected exactly one top-level item, got {:?}", file.items.len());
    match file.items.into_iter().next().unwrap() {
        Item::ExternDecl(e) => e,
        other => panic!("expected Item::ExternDecl, got {other:?}"),
    }
}

// ─────────────────────────── T1: extern type (DLS-08) ─────────────────────

#[test]
fn extern_type_parses_with_correct_decl_span() {
    let src = "extern type Real;";
    let decl = parse_one_extern(src);
    let ExternDecl::Type { span, name } = &decl else {
        panic!("expected ExternDecl::Type, got {decl:?}");
    };
    assert_eq!(name, "Real");
    let span = span.expect("extern type must carry a decl_span");
    // decl_span covers the full `extern type Real;` declaration.
    assert_eq!(span.offset(), 0);
    assert_eq!(span.offset() + span.len(), src.len());
}

#[test]
fn extern_type_with_body_is_a_parse_error_naming_the_declaration() {
    let src = "extern type Real { }";
    let err = parse_str(src).expect_err("a body on `extern type` must be a parse error");
    let msg = err.to_string();
    assert!(msg.contains("extern type Real"), "error should name the offending declaration, got: {msg}");
}

// ─────────────────────────── T2: extern fn (DLS-09) ────────────────────────

#[test]
fn extern_fn_parses_with_correct_decl_span() {
    let src = "extern fn sin(x: Real) -> Real;";
    let decl = parse_one_extern(src);
    let ExternDecl::Fn(sig) = &decl else {
        panic!("expected ExternDecl::Fn, got {decl:?}");
    };
    assert_eq!(sig.name, "sin");
    assert_eq!(sig.params.len(), 1);
    assert_eq!(sig.ret.name, "Real");
    let span = sig.span.expect("extern fn must carry a decl_span");
    assert_eq!(span.offset(), 0);
    assert_eq!(span.offset() + span.len(), src.len());
}

#[test]
fn extern_fn_with_body_is_a_parse_error_naming_the_declaration() {
    let src = "extern fn sin(x: Real) -> Real { x }";
    let err = parse_str(src).expect_err("a body on `extern fn` must be a parse error");
    let msg = err.to_string();
    assert!(msg.contains("extern fn sin"), "error should name the offending declaration, got: {msg}");
}

// ─────────────────────────── T3: extern task (DLS-10) ──────────────────────

#[test]
fn extern_task_parses_with_correct_decl_span_and_dollar_prefixed_name() {
    let src = "extern task $temperature() -> Real;";
    let decl = parse_one_extern(src);
    let ExternDecl::Task(sig) = &decl else {
        panic!("expected ExternDecl::Task, got {decl:?}");
    };
    // The `$`-prefixed system-task name form is preserved (spec: "name
    // retains the $-prefixed form").
    assert_eq!(sig.name, "$temperature");
    assert!(sig.params.is_empty());
    assert_eq!(sig.ret.name, "Real");
    let span = sig.span.expect("extern task must carry a decl_span");
    assert_eq!(span.offset(), 0);
    assert_eq!(span.offset() + span.len(), src.len());
}

#[test]
fn extern_task_with_body_is_a_parse_error_naming_the_declaration() {
    let src = "extern task $temperature() -> Real { 300.0 }";
    let err = parse_str(src).expect_err("a body on `extern task` must be a parse error");
    let msg = err.to_string();
    assert!(msg.contains("extern task $temperature"), "error should name the offending declaration, got: {msg}");
}

// ─────────────────────────── T4: extern operator (DLS-11) ──────────────────

#[test]
fn extern_operator_parses_with_correct_decl_span() {
    let src = "extern operator ddt(x: Real) -> Real;";
    let decl = parse_one_extern(src);
    let ExternDecl::Operator(sig) = &decl else {
        panic!("expected ExternDecl::Operator, got {decl:?}");
    };
    assert_eq!(sig.name, "ddt");
    assert_eq!(sig.params.len(), 1);
    assert_eq!(sig.ret.name, "Real");
    let span = sig.span.expect("extern operator must carry a decl_span");
    assert_eq!(span.offset(), 0);
    assert_eq!(span.offset() + span.len(), src.len());
}

#[test]
fn extern_operator_with_body_is_a_parse_error_naming_the_declaration() {
    let src = "extern operator ddt(x: Real) -> Real { x }";
    let err = parse_str(src).expect_err("a body on `extern operator` must be a parse error");
    let msg = err.to_string();
    assert!(msg.contains("extern operator ddt"), "error should name the offending declaration, got: {msg}");
}

// ─────────────────────────── T5: extern attribute (DLS-12) ─────────────────

#[test]
fn extern_attribute_parses_with_field_decl_spans() {
    let src = "extern attribute device { plugin: String, type: String }";
    let decl = parse_one_extern(src);
    let ExternDecl::Attribute { span, name, fields } = &decl else {
        panic!("expected ExternDecl::Attribute, got {decl:?}");
    };
    assert_eq!(name, "device");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "plugin");
    assert_eq!(fields[0].ty.name, "String");
    assert_eq!(fields[1].name, "type");
    assert_eq!(fields[1].ty.name, "String");
    // Each field carries its own decl_span, distinct from the schema's.
    let schema_span = span.expect("extern attribute must carry a decl_span");
    for field in fields {
        let field_span = field.span.expect("each field must carry its own decl_span");
        assert_ne!(field_span.offset(), schema_span.offset(), "field span should not equal the schema's span");
        assert!(field_span.offset() >= schema_span.offset() && field_span.offset() < schema_span.offset() + schema_span.len());
    }
}

// ─────────────────────────── T6: extern impl (DLS-13, DLS-14) ──────────────

#[test]
fn extern_impl_parses_block_and_each_method_with_distinct_decl_spans() {
    let src = "extern impl Real { fn from(x: Integer) -> Real; fn from(x: Boolean) -> Real; }";
    let decl = parse_one_extern(src);
    let ExternDecl::Impl { span, capability, target, methods } = &decl else {
        panic!("expected ExternDecl::Impl, got {decl:?}");
    };
    assert_eq!(target, "Real");
    assert!(capability.is_none(), "no `for` clause — this is an inherent-method impl");
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "from");
    assert_eq!(methods[0].params.len(), 1);
    assert_eq!(methods[0].ret.name, "Real");
    assert_eq!(methods[1].name, "from");

    let block_span = span.expect("extern impl block must carry a decl_span");
    let m0_span = methods[0].span.expect("each method must carry its own decl_span");
    let m1_span = methods[1].span.expect("each method must carry its own decl_span");
    // The block's span and each method's span are distinct, and each
    // method's span is nested inside the block's span — ctrl+click on
    // `.method(...)` and on the block itself both resolve to different lines.
    assert_ne!(m0_span.offset(), block_span.offset());
    assert_ne!(m0_span.offset(), m1_span.offset());
    for m_span in [m0_span, m1_span] {
        assert!(m_span.offset() >= block_span.offset());
        assert!(m_span.offset() + m_span.len() <= block_span.offset() + block_span.len());
    }
}

#[test]
fn extern_impl_capability_for_type_parses() {
    let src = "extern impl Add for Real { fn add(self, other: Real) -> Real; }";
    let decl = parse_one_extern(src);
    let ExternDecl::Impl { capability, target, methods, .. } = &decl else {
        panic!("expected ExternDecl::Impl, got {decl:?}");
    };
    assert_eq!(capability.as_deref(), Some("Add"));
    assert_eq!(target, "Real");
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "add");
}

#[test]
fn extern_impl_method_with_body_is_a_parse_error_naming_the_method() {
    let src = "extern impl Map { fn get(self, k: K) -> V { k } }";
    let err = parse_str(src).expect_err("a body on an `extern impl` method must be a parse error");
    let msg = err.to_string();
    assert!(msg.contains("extern impl Map") && msg.contains("get"), "error should name the offending method, got: {msg}");
}
