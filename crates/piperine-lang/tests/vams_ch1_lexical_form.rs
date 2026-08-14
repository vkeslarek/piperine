//! LRM Conformance Closure — Phase 1a (`.specs/features/lrm-conformance-closure/`,
//! T10-T15): ch.1-4 lexical/grammar FORM findings LRMC-C1-01, C1-03, C1-04,
//! C1-05, C1-06. Each test is derived from the LRM's own §2.6.1/§2.9 worked
//! examples (spec.md's Edge Cases + Fixture rule), not from this crate's
//! implementation.

use piperine_lang::{BasedNumber, Lexer, NumberBase, NumberLit, ParseErrorKind, Tok};

fn toks(src: &str) -> Vec<Tok> {
    Lexer::new(src)
        .tokenize()
        .unwrap_or_else(|e| panic!("unexpected lex error for {src:?}: {e}"))
        .into_iter()
        .map(|l| l.tok)
        .filter(|t| !matches!(t, Tok::Eof))
        .collect()
}

// ── LRMC-C1-01: per-base digit validation (§2.6.1) ──────────────────────

#[test]
fn illegal_digit_for_declared_base_is_a_typed_lex_error() {
    // `'d1F` — `F` is not a decimal_digit (§2.6.1's `decimal_digit`
    // production is `0`-`9` only).
    let err = Lexer::new("'d1F").tokenize().unwrap_err();
    match err.kind {
        ParseErrorKind::IllegalBasedDigit { base, digit } => {
            assert_eq!(base, 'd');
            assert_eq!(digit, 'F');
        }
        other => panic!("expected IllegalBasedDigit, got {other:?}"),
    }

    // `'o89` — `8`/`9` are not octal_digits (§2.6.1's `octal_digit`
    // production is `0`-`7` plus x_digit/z_digit).
    let err = Lexer::new("'o89").tokenize().unwrap_err();
    match err.kind {
        ParseErrorKind::IllegalBasedDigit { base, digit } => {
            assert_eq!(base, 'o');
            assert_eq!(digit, '8');
        }
        other => panic!("expected IllegalBasedDigit, got {other:?}"),
    }

    // `'b29a` — `2`/`9`/`a` are not binary_digits (§2.6.1's `binary_digit`
    // production is `0`/`1` plus x_digit/z_digit).
    let err = Lexer::new("'b29a").tokenize().unwrap_err();
    match err.kind {
        ParseErrorKind::IllegalBasedDigit { base, digit } => {
            assert_eq!(base, 'b');
            assert_eq!(digit, '2');
        }
        other => panic!("expected IllegalBasedDigit, got {other:?}"),
    }
}

#[test]
fn four_state_digits_remain_legal_in_every_base() {
    // §2.6.1: `x_digit`/`z_digit` (`x`/`X`/`z`/`Z`/`?`) are part of every
    // one of `binary_digit`/`octal_digit`/`decimal_digit`/`hex_digit`, so
    // per-base validation must not reject them.
    let cases: &[(&str, NumberBase, &str)] = &[
        ("4'bx1z0", NumberBase::Binary, "x1z0"),
        ("3'o7xz", NumberBase::Octal, "7xz"),
        ("8'hFFxz", NumberBase::Hex, "FFxz"),
        ("'dx", NumberBase::Decimal, "x"),
        ("'dz", NumberBase::Decimal, "z"),
        ("'d?", NumberBase::Decimal, "?"),
    ];
    for (src, base, digits) in cases {
        assert_eq!(
            toks(src),
            vec![Tok::Number(NumberLit::Based(BasedNumber {
                size: if src.starts_with('\'') { None } else { Some(src.split('\'').next().unwrap().parse().unwrap()) },
                signed: false,
                base: *base,
                digits: digits.to_string(),
            }))],
            "failed on {src}"
        );
    }
}

#[test]
fn hex_digits_remain_fully_legal_in_hex_base() {
    // Every hex_digit (0-9, a-f, A-F) must still lex under `'h` — the
    // per-base restriction must not become over-strict for the base it's
    // actually legal in.
    assert_eq!(
        toks("8'hFf09"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: Some(8),
            signed: false,
            base: NumberBase::Hex,
            digits: "Ff09".to_string(),
        }))]
    );
}
