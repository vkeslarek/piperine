//! Diagnostics: structured parse/elab error codes, per-file fan-out across a
//! project, standalone documents, and the attribute-argument diagnostics that
//! must point at the offending argument.

mod common;
use common::*;

/// LSP-16 Independent Test: an error in `a.phdl` publishes against
/// `a.phdl`'s own URI, not the document that was actually opened
/// (`b.phdl`, which is itself error-free and doesn't even reference `a`).
#[test]
fn cross_file_diagnostics_fan_out_to_the_erroring_file() {
    let scratch = ScratchProject::new("diag_fan_out");
    // References an undeclared discipline — a genuine elaboration error.
    let a_src = "pub mod A (inout p: Nonexistent, inout n: Nonexistent) {}\n";
    scratch.write_src("a.phdl", a_src);
    let b_src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod B (inout p: Electrical, inout n: Electrical) {}\n";
    let b_path = scratch.write_src("b.phdl", b_src);

    let a_uri: Uri = format!("file://{}", scratch.0.join("src").join("a.phdl").display()).parse().unwrap();
    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();

    let by_uri = lsp_collect_diagnostics(&b_uri, b_src, 800);

    let a_str = a_uri.to_string();
    let b_str = b_uri.to_string();
    let a_diags = by_uri.get(&a_str).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for a.phdl, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!a_diags.is_empty(), "a.phdl's own undeclared-discipline error must be published against a.phdl");

    let b_diags = by_uri.get(&b_str).expect("b.phdl must also get a (empty) diagnostics publish");
    assert!(b_diags.is_empty(), "b.phdl elaborates cleanly and must not inherit a.phdl's error");
}

/// LSP-17 Independent Test (fallback half): a standalone document outside
/// any `Piperine.toml` still gets its own diagnostics published — the
/// single-file path is unaffected by T15's project fan-out.
#[test]
fn standalone_document_diagnostics_still_publish() {
    let uri: Uri = "file:///standalone_diag_test.phdl".parse().unwrap();
    // Undeclared discipline — a genuine elaboration error, no project.
    let src = "mod Top (inout p: Nonexistent) {}\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);

    let uri_str = uri.to_string();
    let diags = by_uri.get(&uri_str).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for the standalone file, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!diags.is_empty(), "the standalone document's own error must still be published (LSP-17 fallback)");
}

/// An undeclared-discipline error (`ElabErrorKind::UndefinedType`, coded
/// `E2002` in `piperine_lang::pom::error`) must publish with that exact
/// code and `ERROR` severity — not the old blanket `"parse-error"` string.
#[test]
fn diagnostic_carries_the_structured_elab_error_code() {
    let uri: Uri = "file:///t17_elab_code.phdl".parse().unwrap();
    let src = "mod Top (inout p: Nonexistent) {}\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);
    let uri_str = uri.to_string();
    let diags = by_uri.get(&uri_str).expect("expected a PublishDiagnostics notification");
    assert!(!diags.is_empty(), "the undefined-type error must be published");

    let d = &diags[0];
    assert_eq!(
        d.code,
        Some(lsp_types::NumberOrString::String("E2002".into())),
        "expected the ElabErrorKind::UndefinedType code E2002, got: {:?}",
        d.code
    );
    assert_eq!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
}

/// A parser-level syntax error carries its own distinct code family
/// (`E1xxx`, `piperine_lang::parse::error::ParseError`) — proving the
/// lexer/parser path and the elaboration path each surface their real
/// code, not a single shared placeholder.
#[test]
fn diagnostic_carries_the_structured_parse_error_code() {
    let uri: Uri = "file:///t17_parse_code.phdl".parse().unwrap();
    // Malformed module header — a parser-level syntax error, not elaboration.
    let src = "mod Top ( this is not valid phdl\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);
    let uri_str = uri.to_string();
    let diags = by_uri.get(&uri_str).expect("expected a PublishDiagnostics notification");
    assert!(!diags.is_empty(), "the syntax error must be published");

    let d = &diags[0];
    let code = match &d.code {
        Some(lsp_types::NumberOrString::String(s)) => s.clone(),
        other => panic!("expected a string code, got: {other:?}"),
    };
    assert!(code.starts_with("E1"), "expected an E1xxx parser code, got: {code}");
    assert_ne!(code, "parse-error", "the blanket placeholder must be gone");
    assert_eq!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
}

/// `@rfport(num = "x", ...)` — a bad-type argument value — must produce a
/// diagnostic (structured `E2023` code, `AttrSchemaField`) whose span covers
/// the specific offending argument (`num = "x"`), not the whole attribute
/// or a `0:0` fallback.
#[test]
fn attr_arg_bad_type_diagnostic_points_at_the_specific_argument() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = \"x\", z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a bad-type attribute argument must fail elaboration");
    assert!(!doc.errors.is_empty(), "expected at least one elaboration error");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let arg_start = src.find("num = \"x\"").expect("argument text must be present");
    assert_eq!(span.offset(), arg_start, "span must point at `num = \"x\"`, not the whole attribute or 0:0");
}

/// An unknown field (`bogus`, not part of the `rfport` schema) must produce
/// a diagnostic whose span covers that specific argument.
#[test]
fn attr_arg_unknown_field_diagnostic_points_at_the_specific_argument() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = 1, bogus = 2) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "an unknown attribute field must fail elaboration");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let arg_start = src.find("bogus = 2").expect("argument text must be present");
    assert_eq!(span.offset(), arg_start, "span must point at `bogus = 2`");
}

/// A missing required field (`num`, `rfport`'s only required field) — with
/// no single argument to blame — must still produce a diagnostic, with its
/// span falling back to the whole `@rfport(...)` attribute rather than
/// `0:0`.
#[test]
fn attr_missing_required_field_diagnostic_points_at_the_attribute() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a missing required attribute field must fail elaboration");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let attr_start = src.find("@rfport(z0 = 50)").expect("attribute text must be present");
    assert_eq!(span.offset(), attr_start, "span must fall back to the whole attribute, not 0:0");
}
