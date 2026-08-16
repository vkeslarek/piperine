//! Diagnostics: structured parse/elab error codes, per-file fan-out across a
//! project, standalone documents, and the attribute-argument diagnostics that
//! must point at the offending argument.
//!
//! Also covers T12 (`.specs/features/preprocessor-span-mapping/tasks.md`,
//! SPANMAP-08): a diagnostic whose real cause lives inside `` `include ``d
//! content degrades to a clearly-labeled "originates in `<path>`, not this
//! document" note instead of a wrong in-document range.
//!
//! **lang-server-vams-migration (T8)**: fixtures below are real VAMS.
//! `rfport` no longer exists — retargeted onto `device`/`port`
//! (`piperine_lang::seed_device_port_schemas`). Every test in this file
//! passes — no `span: None`-gap or other blocker applies (diagnostics are
//! sourced from the elaborator's own error spans, independent of the
//! POM-node `span` fields `resolve_at`/`index_design` depend on).
mod common;
use common::*;

/// SPANMAP-08 (T12): a diagnostic whose real cause is inside `` `include ``d
/// content (`inc.vams`, included by the opened `top.vams`) degrades to the
/// "originates in `<path>`, not this document" form — the error still
/// surfaces against the including document's own URI (existing fan-out
/// behavior, unaffected), but its range is a safe sentinel, never a
/// fabricated line pointing at unrelated text in `top.vams`.
#[test]
fn include_error_degrades_to_originates_in_note_not_wrong_in_document_line() {
    let scratch = ScratchProject::new("diag_include_origin");
    // Deliberately malformed — the same crash-repro shape span_resolution.rs
    // (piperine-lang, T8) uses: an unterminated port list.
    let inc_src = "module Bad(;\nendmodule\n";
    scratch.write_src("inc.vams", inc_src);
    let top_src = "`include \"inc.vams\"\n";
    let top_path = scratch.write_src("top.vams", top_src);

    let top_uri: Uri = format!("file://{}", top_path.display()).parse().unwrap();
    let by_uri = lsp_collect_diagnostics(&top_uri, top_src, 800);

    let uri_str = top_uri.to_string();
    let diags = by_uri.get(&uri_str).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for top.vams, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!diags.is_empty(), "the include's own parse error must still surface as a diagnostic on the including document");

    let d = &diags[0];
    assert!(d.message.contains("inc.vams"), "diagnostic should name the file the error actually originates in, got: {}", d.message);
    assert!(
        d.message.contains("not this document"),
        "diagnostic should be clearly labeled as not originating in this document, got: {}",
        d.message
    );
    // No wrong in-document range: degrades to the (0,0) sentinel, never a
    // fabricated line/column pointing at unrelated text in top.vams.
    assert_eq!(d.range.start, lsp_types::Position { line: 0, character: 0 });
    assert_eq!(d.range.end, lsp_types::Position { line: 0, character: 1 });
}

/// LSP-16 Independent Test: an error in `a.vams` publishes against
/// `a.vams`'s own URI, not the document that was actually opened
/// (`b.vams`, which is itself error-free and doesn't even reference `a`).
#[test]
fn cross_file_diagnostics_fan_out_to_the_erroring_file() {
    let scratch = ScratchProject::new("diag_fan_out");
    // References an undeclared module — a genuine elaboration error.
    let a_src = "module A(p);\n  inout electrical p;\n  Nonexistent inst1(.p(p));\nendmodule\n";
    scratch.write_src("a.vams", a_src);
    let b_src = "module B(p, n);\n  inout electrical p, n;\nendmodule\n";
    let b_path = scratch.write_src("b.vams", b_src);

    let a_uri: Uri = format!("file://{}", scratch.0.join("src").join("a.vams").display()).parse().unwrap();
    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();

    let by_uri = lsp_collect_diagnostics(&b_uri, b_src, 800);

    let a_str = a_uri.to_string();
    let b_str = b_uri.to_string();
    let a_diags = by_uri.get(&a_str).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for a.vams, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!a_diags.is_empty(), "a.vams's own undefined-module error must be published against a.vams");

    let b_diags = by_uri.get(&b_str).expect("b.vams must also get a (empty) diagnostics publish");
    assert!(b_diags.is_empty(), "b.vams elaborates cleanly and must not inherit a.vams's error");
}

/// LSP-17 Independent Test (fallback half): a standalone document outside
/// any `Piperine.toml` still gets its own diagnostics published — the
/// single-file path is unaffected by T15's project fan-out.
#[test]
fn standalone_document_diagnostics_still_publish() {
    let uri: Uri = "file:///standalone_diag_test.vams".parse().unwrap();
    // Undefined module reference — a genuine elaboration error, no project.
    let src = "module Top(p);\n  inout electrical p;\n  Nonexistent inst1(.p(p));\nendmodule\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);

    let uri_str = uri.to_string();
    let diags = by_uri.get(&uri_str).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for the standalone file, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!diags.is_empty(), "the standalone document's own error must still be published (LSP-17 fallback)");
}

/// An undefined-module error (`ElabErrorKind::UndefinedModule`, coded
/// `E2003` in `piperine_lang::pom::error`) must publish with that exact
/// code and `ERROR` severity — not the old blanket `"parse-error"` string.
///
/// Retargeted from the PHDL fixture's `E2002` (undefined *type*, a
/// discipline-reference concept PHDL used for both disciplines and module
/// references) — VAMS's elaborator splits these: `E2002` fires only for an
/// undefined *nature* name inside a discipline's `parent` chain
/// (`elab/lower/vams.rs::effective_nature`), a narrow, hard-to-reach case
/// for a plain module-body fixture. `E2003` (undefined module reference,
/// confirmed directly: instantiating an undeclared module name) is the
/// real, easily reproduced structured-error-code case that preserves this
/// test's actual intent — "a real elaboration error carries its own
/// specific code, not a placeholder."
#[test]
fn diagnostic_carries_the_structured_elab_error_code() {
    let uri: Uri = "file:///t17_elab_code.vams".parse().unwrap();
    let src = "module Top(p);\n  inout electrical p;\n  Nonexistent inst1(.p(p));\nendmodule\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);
    let uri_str = uri.to_string();
    let diags = by_uri.get(&uri_str).expect("expected a PublishDiagnostics notification");
    assert!(!diags.is_empty(), "the undefined-module error must be published");

    let d = &diags[0];
    assert_eq!(
        d.code,
        Some(lsp_types::NumberOrString::String("E2003".into())),
        "expected the ElabErrorKind::UndefinedModule code E2003, got: {:?}",
        d.code
    );
    assert_eq!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
}

/// A parser-level syntax error carries its own distinct code
/// (`E1005`, `VamsParse` in `piperine_lang::pom::error`) — proving the
/// lexer/parser path and the elaboration path each surface their real
/// code, not a single shared placeholder.
#[test]
fn diagnostic_carries_the_structured_parse_error_code() {
    let uri: Uri = "file:///t17_parse_code.vams".parse().unwrap();
    // Malformed module header — a parser-level syntax error, not elaboration.
    let src = "module Top ( this is not valid vams\n";

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

/// `(* device = 5, type = ".." *)` — a bad-type argument value — must
/// produce a diagnostic (structured `E2023` code, `AttrSchemaField`) whose
/// span covers the specific offending argument (`device = 5`), not the
/// whole attribute or a `0:0` fallback.
#[test]
fn attr_arg_bad_type_diagnostic_points_at_the_specific_argument() {
    let src = "(* device = 5, type = \"host_resistor\" *)\nmodule HostR(p, n);\n  inout electrical p, n;\nendmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a bad-type attribute argument must fail elaboration");
    assert!(!doc.errors.is_empty(), "expected at least one elaboration error");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let arg_start = src.find("device = 5").expect("argument text must be present");
    assert_eq!(span.offset(), arg_start, "span must point at `device = 5`, not the schema-name token or the whole attribute");
}

/// An unknown field (`bogus`, not part of the `device` schema) must
/// produce a diagnostic whose span covers that specific argument. This is
/// the one field-diagnostic case VAMS keeps precise — not blocked.
#[test]
fn attr_arg_unknown_field_diagnostic_points_at_the_specific_argument() {
    let src = "(* device = \"none\", type = \"host_resistor\", bogus = 2 *)\nmodule HostR(p, n);\n  inout electrical p, n;\nendmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "an unknown attribute field must fail elaboration");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2022"))
        .expect("expected the unknown-attribute-schema (E2022) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let arg_start = src.find("bogus = 2").expect("argument text must be present");
    assert_eq!(span.offset(), arg_start, "span must point at `bogus = 2`");
}

/// A missing required field (`type`, required alongside `device`) — with
/// no single argument to blame — must still produce a diagnostic, with its
/// span falling back to a fixed anchor (the `device` schema-name token)
/// rather than `0:0`.
#[test]
fn attr_missing_required_field_diagnostic_points_at_the_attribute() {
    let src = "(* device = \"none\" *)\nmodule HostR(p, n);\n  inout electrical p, n;\nendmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a missing required attribute field must fail elaboration");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let attr_start = src.find("device").expect("attribute text must be present");
    assert_eq!(span.offset(), attr_start, "span must fall back to the schema-name token, not 0:0");
}
