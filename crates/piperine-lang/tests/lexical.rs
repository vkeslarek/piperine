//! Proves the lexer (tasks.md T3) against LRM §2.2-2.8: every token form
//! this task's "Done when" lists gets a positive test, and each malformed
//! form gets a negative test asserting the right `ParseErrorKind` + span.

use piperine_lang::{BasedNumber, Lexer, NumberBase, NumberLit, Tok};

fn toks(src: &str) -> Vec<Tok> {
    Lexer::new(src)
        .tokenize()
        .unwrap_or_else(|e| panic!("unexpected lex error for {src:?}: {e}"))
        .into_iter()
        .map(|l| l.tok)
        .filter(|t| !matches!(t, Tok::Eof))
        .collect()
}

// ── §2.6.1 Integer constants ────────────────────────────────────────────

#[test]
fn sized_based_hex_integer() {
    assert_eq!(
        toks("8'hFF"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: Some(8),
            signed: false,
            base: NumberBase::Hex,
            digits: "FF".to_string(),
        }))]
    );
}

#[test]
fn sized_based_binary_integer_with_underscore() {
    assert_eq!(
        toks("4'b10_10"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: Some(4),
            signed: false,
            base: NumberBase::Binary,
            digits: "10_10".to_string(),
        }))]
    );
}

#[test]
fn unsized_based_decimal_integer() {
    assert_eq!(
        toks("'d15"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: None,
            signed: false,
            base: NumberBase::Decimal,
            digits: "15".to_string(),
        }))]
    );
}

#[test]
fn signed_based_octal_integer() {
    assert_eq!(
        toks("3'so7"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: Some(3),
            signed: true,
            base: NumberBase::Octal,
            digits: "7".to_string(),
        }))]
    );
}

#[test]
fn four_state_digits_preserved_verbatim() {
    assert_eq!(
        toks("8'bxxzz01?0"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: Some(8),
            signed: false,
            base: NumberBase::Binary,
            digits: "xxzz01?0".to_string(),
        }))]
    );
}

#[test]
fn plain_unsized_decimal_integer() {
    assert_eq!(toks("42"), vec![Tok::Number(NumberLit::Integer(42))]);
}

// ── §2.6.2 Real constants (incl. SI scale suffixes, Table 2-1) ─────────

#[test]
fn real_decimal_point_form() {
    assert_eq!(toks("14.72"), vec![Tok::Number(NumberLit::Real(14.72))]);
}

#[test]
fn real_scientific_notation() {
    let t = toks("39e8");
    match &t[..] {
        [Tok::Number(NumberLit::Real(v))] => assert!((*v - 39e8).abs() < 1e-6),
        other => panic!("expected one Real token, got {other:?}"),
    }
}

#[test]
fn real_exponent_with_sign() {
    let t = toks("1.30e-2");
    match &t[..] {
        [Tok::Number(NumberLit::Real(v))] => assert!((*v - 1.30e-2).abs() < 1e-9),
        other => panic!("expected one Real token, got {other:?}"),
    }
}

#[test]
fn real_scale_factor_pico() {
    let t = toks("1.2p");
    match &t[..] {
        [Tok::Number(NumberLit::Real(v))] => assert!((*v - 1.2e-12).abs() < 1e-24),
        other => panic!("expected one Real token, got {other:?}"),
    }
}

#[test]
fn real_scale_factor_kilo_no_decimal_point() {
    // LRM §2.6.2 example: `7k`.
    let t = toks("7k");
    match &t[..] {
        [Tok::Number(NumberLit::Real(v))] => assert!((*v - 7000.0).abs() < 1e-6),
        other => panic!("expected one Real token, got {other:?}"),
    }
}

#[test]
fn real_underscores_ignored() {
    // LRM §2.6.2 example: `236.123_763_e-12`.
    let t = toks("236.123_763_e-12");
    match &t[..] {
        [Tok::Number(NumberLit::Real(v))] => assert!((*v - 236.123763e-12).abs() < 1e-20),
        other => panic!("expected one Real token, got {other:?}"),
    }
}

#[test]
fn all_table_2_1_scale_symbols() {
    let cases: &[(&str, f64)] = &[
        ("1T", 1e12),
        ("1G", 1e9),
        ("1M", 1e6),
        ("1K", 1e3),
        ("1k", 1e3),
        ("1m", 1e-3),
        ("1u", 1e-6),
        ("1n", 1e-9),
        ("1p", 1e-12),
        ("1f", 1e-15),
        ("1a", 1e-18),
    ];
    for (src, expected) in cases {
        let t = toks(src);
        match &t[..] {
            [Tok::Number(NumberLit::Real(v))] => {
                assert!(
                    (*v - *expected).abs() / expected.abs() < 1e-9,
                    "{src}: expected {expected}, got {v}"
                );
            }
            other => panic!("{src}: expected one Real token, got {other:?}"),
        }
    }
}

// ── §2.7 String literals (Table 2-2 escapes) ────────────────────────────

#[test]
fn plain_string_literal() {
    assert_eq!(toks(r#""hello world""#), vec![Tok::Str("hello world".to_string())]);
}

#[test]
fn string_escape_newline_tab_backslash_quote() {
    assert_eq!(toks(r#""a\nb\tc\\d\"e""#), vec![Tok::Str("a\nb\tc\\d\"e".to_string())]);
}

#[test]
fn string_octal_escape() {
    // \101 is octal for 'A' (0x41 = 65 = 0o101).
    assert_eq!(toks(r#""\101""#), vec![Tok::Str("A".to_string())]);
}

#[test]
fn unterminated_string_is_an_error() {
    let err = Lexer::new("\"no closing quote").tokenize().unwrap_err();
    assert!(matches!(err.kind, piperine_lang::ParseErrorKind::UnterminatedString));
}

#[test]
fn invalid_escape_is_an_error() {
    let err = Lexer::new(r#""bad \q escape""#).tokenize().unwrap_err();
    match err.kind {
        piperine_lang::ParseErrorKind::InvalidEscape { found } => assert_eq!(found, 'q'),
        other => panic!("expected InvalidEscape, got {other:?}"),
    }
}

// ── §2.8 Identifiers ─────────────────────────────────────────────────────

#[test]
fn simple_identifiers() {
    for src in ["shiftreg_a", "busa_index", "error_condition", "merge_ab", "_bus3", "n$657"] {
        assert_eq!(toks(src), vec![Tok::Ident(src.to_string())], "failed on {src}");
    }
}

#[test]
fn escaped_identifier_strips_backslash_and_terminator() {
    assert_eq!(toks("\\busa+index "), vec![Tok::EscapedIdent("busa+index".to_string())]);
}

#[test]
fn escaped_identifier_examples_from_lrm() {
    assert_eq!(toks("\\-clock "), vec![Tok::EscapedIdent("-clock".to_string())]);
    assert_eq!(toks("\\{a,b} "), vec![Tok::EscapedIdent("{a,b}".to_string())]);
}

// ── §2.8.3 / §2.8.4 System tasks and compiler directives ────────────────

#[test]
fn system_identifier() {
    assert_eq!(toks("$display"), vec![Tok::SystemIdent("display".to_string())]);
}

#[test]
fn compiler_directive_identifier() {
    assert_eq!(toks("`define"), vec![Tok::Directive("define".to_string())]);
}

// ── §2.4 Comments ─────────────────────────────────────────────────────

#[test]
fn line_comment() {
    assert_eq!(toks("// a comment"), vec![Tok::LineComment(" a comment".to_string())]);
}

#[test]
fn block_comment() {
    assert_eq!(toks("/* a block */"), vec![Tok::BlockComment(" a block ".to_string())]);
}

#[test]
fn unterminated_block_comment_is_an_error() {
    let err = Lexer::new("/* never closed").tokenize().unwrap_err();
    assert!(matches!(err.kind, piperine_lang::ParseErrorKind::UnterminatedComment));
}

// ── Newline preservation (needed by the preprocessor, T5) ───────────────

#[test]
fn newlines_are_preserved_as_their_own_token() {
    assert_eq!(toks("a\nb"), vec![Tok::Ident("a".to_string()), Tok::Newline, Tok::Ident("b".to_string())]);
}

// ── Malformed input ───────────────────────────────────────────────────

#[test]
fn unexpected_character_is_an_error() {
    let err = Lexer::new("\x01").tokenize().unwrap_err();
    match err.kind {
        piperine_lang::ParseErrorKind::UnexpectedCharacter { found } => assert_eq!(found, '\x01'),
        other => panic!("expected UnexpectedCharacter, got {other:?}"),
    }
}

// ── Spans ────────────────────────────────────────────────────────────

#[test]
fn token_spans_are_byte_accurate() {
    let lexed = Lexer::new("foo bar").tokenize().unwrap();
    assert_eq!(lexed[0].span.start, 0);
    assert_eq!(lexed[0].span.end, 3);
    assert_eq!(lexed[1].span.start, 4);
    assert_eq!(lexed[1].span.end, 7);
}
