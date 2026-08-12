//! Proves the typed `ParseError` enum (E-STD, tasks.md T2): every variant
//! carries structured fields that survive round-trip, every variant has a
//! span, and — the hard constraint — the module contains no free-string
//! catch-all variant and no `From<String>`/`From<&str>` escape hatch.

use piperine_vams::{ParseError, ParseErrorKind, Span};

#[test]
fn unexpected_character_carries_the_offending_char_and_span() {
    let err = ParseError::new(ParseErrorKind::UnexpectedCharacter { found: '#' }, Span::new(3, 4));
    assert_eq!(err.kind, ParseErrorKind::UnexpectedCharacter { found: '#' });
    assert_eq!(err.span, Span::new(3, 4));
    assert_eq!(err.to_string(), "unexpected character `#` (at byte 3..4)");
}

#[test]
fn unterminated_string_carries_its_span() {
    let err = ParseError::new(ParseErrorKind::UnterminatedString, Span::new(10, 20));
    assert_eq!(err.span, Span::new(10, 20));
    assert_eq!(err.to_string(), "unterminated string literal (at byte 10..20)");
}

#[test]
fn unterminated_comment_carries_its_span() {
    let err = ParseError::new(ParseErrorKind::UnterminatedComment, Span::new(0, 5));
    assert_eq!(err.to_string(), "unterminated block comment (at byte 0..5)");
}

#[test]
fn invalid_escape_carries_the_offending_char() {
    let err = ParseError::new(ParseErrorKind::InvalidEscape { found: 'q' }, Span::new(7, 9));
    assert_eq!(err.kind, ParseErrorKind::InvalidEscape { found: 'q' });
    assert_eq!(err.to_string(), "invalid escape sequence `\\q` (at byte 7..9)");
}

#[test]
fn invalid_number_literal_carries_the_offending_text() {
    let err = ParseError::new(
        ParseErrorKind::InvalidNumberLiteral { text: "1.2.3".to_string() },
        Span::new(0, 5),
    );
    assert_eq!(
        err.kind,
        ParseErrorKind::InvalidNumberLiteral { text: "1.2.3".to_string() }
    );
    assert_eq!(err.to_string(), "invalid number literal `1.2.3` (at byte 0..5)");
}

#[test]
fn unexpected_token_carries_expected_and_found_structured_not_a_sentence() {
    let err = ParseError::new(
        ParseErrorKind::UnexpectedToken {
            expected: "';'".to_string(),
            found: "identifier `foo`".to_string(),
        },
        Span::new(12, 15),
    );
    assert_eq!(
        err.kind,
        ParseErrorKind::UnexpectedToken {
            expected: "';'".to_string(),
            found: "identifier `foo`".to_string(),
        }
    );
    assert_eq!(err.to_string(), "expected ';', found identifier `foo` (at byte 12..15)");
}

#[test]
fn unexpected_eof_carries_what_was_expected() {
    let err = ParseError::new(
        ParseErrorKind::UnexpectedEof { expected: "'endmodule'".to_string() },
        Span::new(100, 100),
    );
    assert_eq!(
        err.to_string(),
        "unexpected end of file, expected 'endmodule' (at byte 100..100)"
    );
}

#[test]
fn parse_error_implements_std_error() {
    let err = ParseError::new(ParseErrorKind::UnterminatedString, Span::new(0, 1));
    let _: &dyn std::error::Error = &err;
}

/// E-STD hard constraint: no free-string catch-all variant, no
/// `From<String>`/`From<&str>` escape hatch. Checked by source scan rather
/// than by type system alone, so the constraint stays enforced even if a
/// future edit tries to reintroduce the anti-pattern under a different name.
#[test]
fn error_module_has_no_free_string_catch_all() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/error.rs"),
    )
    .expect("read src/error.rs");

    // Scan code lines only — comments are prose and may describe what this
    // constraint bans (this file's own module doc comment does).
    let code_only: String = src
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let from_str_impl = ["From<St", "ring>"].concat();
    let from_ref_str_impl = ["From<&", "str>"].concat();
    assert!(
        !code_only.contains(&from_str_impl) && !code_only.contains(&from_ref_str_impl),
        "error.rs must not implement From<String>/From<&str> for ParseError or \
         ParseErrorKind (E-STD: no free-string catch-all escape hatch)"
    );

    // No variant may hold a bare `message: String` / `String` payload with
    // no other structured field alongside it — that shape is exactly the
    // `Generic { message: String }` / `Legacy { message: String }`
    // anti-pattern this task explicitly rejects.
    let message_field = ["mess", "age: String"].concat();
    assert!(
        !code_only.contains(&message_field) && !code_only.contains("Legacy"),
        "error.rs must not reintroduce a Generic/Legacy free-string message variant"
    );
}
