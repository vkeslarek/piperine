//! VAMS pre-T21 deviations commit — closes the two SPEC_DEVIATIONs left by
//! T19 (From<piperine_vams::ParseError>) and T20 (Design::paramsets).
//!
//! Both deviations required the piperine-vams dependency, which this
//! commit wires in. The tests prove:
//!
//! 1. `From<piperine_vams::ParseError> for ElabError` carries the typed
//!    `ParseErrorKind` through unchanged and converts the byte span into
//!    a miette `SourceSpan` (VAMS-05).
//! 2. The wrapping `ElabErrorKind::VamsParse` variant renders with the
//!    E1005 code, a descriptive label, and a help text that names LRM
//!    ch. 2 / Annex A.
//! 3. `Design::paramsets` carries top-level `paramset` declarations
//!    (LRM §6.4), with the same accessor/mutator shape as `natures`.

use miette::Diagnostic;
use piperine_lang::pom::{Design, ElabError, ElabErrorKind};
use piperine_vams::{ParseError, ParseErrorKind, Span};

#[test]
fn from_vams_parse_error_carries_typed_kind_through_unchanged() {
    // VAMS-05: a parse failure surfaces as an ElabError without losing
    // the typed cause. The variant's `kind` field IS the original
    // ParseErrorKind — round-tripped.
    let pe = ParseError::new(
        ParseErrorKind::UnexpectedToken {
            expected: "';'".into(),
            found: "'+'".into(),
        },
        Span::new(42, 43),
    );
    let elab: ElabError = pe.into();
    let ElabErrorKind::VamsParse { kind, span } = &elab.kind else {
        panic!("expected VamsParse variant, got {:?}", elab.kind);
    };
    match kind {
        ParseErrorKind::UnexpectedToken { expected, found } => {
            assert_eq!(expected, "';'");
            assert_eq!(found, "'+'");
        }
        other => panic!("expected UnexpectedToken, got {other:?}"),
    }
    // Span round-trips through the byte→SourceSpan conversion.
    let span = span.expect("span must be carried");
    assert_eq!(span.offset(), 42);
    assert_eq!(span.len(), 1);
}

#[test]
fn from_vams_parse_error_handles_zero_length_span() {
    // saturating_sub on a zero-length span (start == end) yields a
    // zero-length SourceSpan, not a panic.
    let pe = ParseError::new(ParseErrorKind::UnexpectedEof { expected: "'endmodule'".into() }, Span::new(100, 100));
    let elab: ElabError = pe.into();
    let ElabErrorKind::VamsParse { span, .. } = &elab.kind else { panic!() };
    let span = span.expect("span carried");
    assert_eq!(span.offset(), 100);
    assert_eq!(span.len(), 0);
}

#[test]
fn vams_parse_variant_renders_with_e1005_code_and_clause_citation() {
    // E-STD 2: numbered diagnostic code. E-STD 4: #[help] present.
    // E-STD 5: LRM clause cited for parse failures (ch. 2 / Annex A).
    let pe = ParseError::new(
        ParseErrorKind::InvalidNumberLiteral { text: "1.2Q".into() },
        Span::new(7, 11),
    );
    let elab: ElabError = pe.into();
    assert_eq!(
        Diagnostic::code(&elab.kind).map(|c| c.to_string()),
        Some("E1005".into()),
    );
    let help = Diagnostic::help(&elab.kind).map(|h| h.to_string()).expect("E1005 has help");
    assert!(
        help.contains("ch. 2") || help.contains("Annex A"),
        "E1005 help must cite LRM ch. 2 / Annex A: {help}",
    );
    // The variant's #[error] uses the inner kind's Display.
    let message = elab.kind.to_string();
    assert!(message.contains("1.2Q"), "message must surface the offending literal: {message}");
}

#[test]
fn vams_parse_variant_renders_every_distinct_kind_with_descriptive_text() {
    // Each ParseErrorKind variant renders distinct text via its Display.
    // Spot-check three common causes spanning lexer, parser, and
    // preprocessor — proves the wrapping variant preserves the data
    // rather than flattening to a generic message.
    let cases: [(ParseErrorKind, &str); 3] = [
        (ParseErrorKind::UnterminatedString, "unterminated string literal"),
        (ParseErrorKind::IncludeNotFound { path: "foo.vams".into() }, "`include \"foo.vams\""),
        (ParseErrorKind::MacroArityMismatch { name: "FOO".into(), expected: 2, found: 1 }, "expects 2 argument(s)"),
    ];
    for (kind, needle) in cases {
        let pe = ParseError::new(kind, Span::new(0, 1));
        let elab: ElabError = pe.into();
        let msg = elab.kind.to_string();
        assert!(msg.contains(needle), "rendered message must contain `{needle}`: {msg}");
    }
}

#[test]
fn design_paramsets_collection_default_empty() {
    // LRM §6.4: paramsets are top-level, named, reusable parameter sets
    // bound to a module. A fresh Design carries none; the accessor and
    // lookup return empty/None. Population-by-mutator is unit-tested
    // inline in `design.rs` (`pub(crate)` mutator visibility).
    let d = Design::new();
    assert!(d.paramsets().next().is_none(), "paramsets default empty");
    assert!(d.paramset("nmos_fast").is_none(), "lookups on empty design return None");
}

#[test]
fn design_paramsets_collection_is_serde_skipped() {
    // Paramsets on Design are #[serde(skip)] — same rule as disciplines/
    // natures: declaration-shaped data the deserialized Design carries
    // empty and re-elaboration rebuilds. The serialized form must NOT
    // carry paramsets.
    let d = Design::new();
    let json = serde_json::to_string(&d).expect("serialize fresh Design");
    assert!(
        !json.contains("paramsets"),
        "Design::paramsets is #[serde(skip)] — must not appear in serialized form: {json}",
    );
    // Round-trip is idempotent on an empty design.
    let _: Design = serde_json::from_str(&json).expect("deserialize fresh Design");
}
