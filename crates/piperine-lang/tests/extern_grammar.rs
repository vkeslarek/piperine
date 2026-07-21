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
    let ExternDecl::Type { span, name } = &decl;
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
