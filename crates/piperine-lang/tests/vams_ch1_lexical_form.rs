//! LRM Conformance Closure — Phase 1a (`.specs/features/lrm-conformance-closure/`,
//! T10-T15): ch.1-4 lexical/grammar FORM findings LRMC-C1-01, C1-03, C1-04,
//! C1-05, C1-06. Each test is derived from the LRM's own §2.6.1/§2.9 worked
//! examples (spec.md's Edge Cases + Fixture rule), not from this crate's
//! implementation.

use piperine_lang::ast::Expr;
use piperine_lang::{BasedNumber, Grammar, Lexer, NumberBase, NumberLit, ParseErrorKind, Tok, UnderscorePosition};

fn parse_expr(src: &str) -> Expr {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    let mut g = Grammar::new(toks);
    g.parse_expr().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"))
}

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

// ── LRMC-C1-03: whitespace between base letter and digits (§2.6.1) ─────

#[test]
fn whitespace_after_base_letter_is_accepted() {
    // LRM §2.6.1 Example 2's own worked example: `` 'D 3 `` — "a 5-bit
    // decimal number" (unsized here since no size prefix in the example).
    assert_eq!(
        toks("'D 3"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: None,
            signed: false,
            base: NumberBase::Decimal,
            digits: "3".to_string(),
        }))]
    );

    // LRM §2.6.1 Example 1: `` 'h 837FF `` — "is a hexadecimal number".
    assert_eq!(
        toks("'h 837FF"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: None,
            signed: false,
            base: NumberBase::Hex,
            digits: "837FF".to_string(),
        }))]
    );

    // LRM §2.6.1 Example 5: `` 32 'h 12ab_f001 `` — sized, with whitespace
    // after the base letter and underscores in the digit run.
    assert_eq!(
        toks("32'h 12ab_f001"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: Some(32),
            signed: false,
            base: NumberBase::Hex,
            digits: "12ab_f001".to_string(),
        }))]
    );
}

#[test]
fn newline_after_base_letter_still_errors() {
    // Whitespace is legal there per §2.6.1's own wording ("optionally
    // preceded by white space"), but a line break is not inline
    // whitespace — the digit run still can't cross it, so this must
    // still fail (as "at least one digit after the base" — a newline
    // right after the base letter leaves zero digits before it).
    let err = piperine_lang::Lexer::new("'h\nFF").tokenize().unwrap_err();
    assert!(
        matches!(err.kind, ParseErrorKind::UnexpectedEof { .. }),
        "expected UnexpectedEof, got {:?}",
        err.kind
    );
}

// ── LRMC-C1-04: underscore placement rules (§2.6.1/§2.6.2) ─────────────

#[test]
fn underscore_immediately_after_decimal_point_is_rejected() {
    // `1._5` — §2.6.2: underscore illegal as "the first character after
    // the decimal point".
    let err = Lexer::new("1._5").tokenize().unwrap_err();
    assert!(
        matches!(
            err.kind,
            ParseErrorKind::IllegalUnderscorePlacement(UnderscorePosition::AfterDecimalPoint)
        ),
        "expected IllegalUnderscorePlacement(AfterDecimalPoint), got {:?}",
        err.kind
    );
}

#[test]
fn underscore_as_first_based_digit_is_rejected() {
    // `` 'h_FF `` — §2.6.1: underscore illegal "as the first character"
    // of the digit run.
    let err = Lexer::new("'h_FF").tokenize().unwrap_err();
    assert!(
        matches!(
            err.kind,
            ParseErrorKind::IllegalUnderscorePlacement(UnderscorePosition::FirstBasedDigit)
        ),
        "expected IllegalUnderscorePlacement(FirstBasedDigit), got {:?}",
        err.kind
    );
}

#[test]
fn legal_underscore_separators_still_accepted() {
    // `1_000` — a plain decimal integer with a mid-run separator.
    assert_eq!(toks("1_000"), vec![Tok::Number(NumberLit::Integer(1000))]);

    // `` 'h FF_FF `` — a based literal with whitespace after the base
    // letter (LRMC-C1-03) and a mid-run separator in the digit string.
    assert_eq!(
        toks("'h FF_FF"),
        vec![Tok::Number(NumberLit::Based(BasedNumber {
            size: None,
            signed: false,
            base: NumberBase::Hex,
            digits: "FF_FF".to_string(),
        }))]
    );
}

// ── LRMC-C1-06 (FORM half): assignment pattern `'{...}` (§4.2.14) ──────

#[test]
fn plain_list_assignment_pattern_parses_from_lrm_3_4_4() {
    // §3.4.4's own worked example: `parameter real poles[0:3] = '{ 1.0,
    // 3.198, 4.554, 2.00 };` — the initializer expression alone.
    let e = parse_expr("'{ 1.0, 3.198, 4.554, 2.00 }");
    match e {
        Expr::AssignmentPattern { replicate, parts, .. } => {
            assert!(replicate.is_none(), "plain list form must not carry a replication count");
            assert_eq!(parts.len(), 4);
            for (part, expected) in parts.iter().zip([1.0, 3.198, 4.554, 2.00]) {
                match part {
                    Expr::Number { value: NumberLit::Real(v), .. } => assert_eq!(*v, expected),
                    other => panic!("expected Real({expected}), got {other:?}"),
                }
            }
        }
        other => panic!("expected Expr::AssignmentPattern, got {other:?}"),
    }
}

#[test]
fn string_valid_value_list_assignment_pattern_parses_from_lrm_3_4_6() {
    // §3.4.6's own worked example: `parameter string transistortype =
    // "NPN" from '{ "NPN", "PNP" };` — the `from` clause's list.
    let e = parse_expr(r#"'{ "NPN", "PNP" }"#);
    match e {
        Expr::AssignmentPattern { replicate, parts, .. } => {
            assert!(replicate.is_none());
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0], Expr::Str { span: parts[0].span(), value: "NPN".to_string() });
            assert_eq!(parts[1], Expr::Str { span: parts[1].span(), value: "PNP".to_string() });
        }
        other => panic!("expected Expr::AssignmentPattern, got {other:?}"),
    }
}

#[test]
fn replicated_assignment_pattern_parses() {
    // §4.2.14's own second example: `parameter real data2[0:4] = '{
    // 5{0.0} };` — the replicated form, `count` = 5.
    let e = parse_expr("'{ 5{0.0} }");
    match e {
        Expr::AssignmentPattern { replicate, parts, .. } => {
            match replicate.as_deref() {
                Some(Expr::Number { value: NumberLit::Integer(5), .. }) => {}
                other => panic!("expected replicate count 5, got {other:?}"),
            }
            assert_eq!(parts.len(), 1);
            match &parts[0] {
                Expr::Number { value: NumberLit::Real(v), .. } => assert_eq!(*v, 0.0),
                other => panic!("expected Real(0.0), got {other:?}"),
            }
        }
        other => panic!("expected Expr::AssignmentPattern, got {other:?}"),
    }
}

#[test]
fn multidimensional_nested_assignment_pattern_parses_from_lrm_3_4_8() {
    // §3.4.8's own worked example: `real distort[0:2][0:2] = '{ 3{ '{3{0.0}}}};`
    // — a replicated pattern whose replicated part is itself a nested
    // assignment pattern (not a Concat/Replication).
    let e = parse_expr("'{ 3{ '{3{0.0}} } }");
    match e {
        Expr::AssignmentPattern { replicate, parts, .. } => {
            match replicate.as_deref() {
                Some(Expr::Number { value: NumberLit::Integer(3), .. }) => {}
                other => panic!("expected outer replicate count 3, got {other:?}"),
            }
            assert_eq!(parts.len(), 1);
            match &parts[0] {
                Expr::AssignmentPattern { replicate: inner_rep, parts: inner_parts, .. } => {
                    match inner_rep.as_deref() {
                        Some(Expr::Number { value: NumberLit::Integer(3), .. }) => {}
                        other => panic!("expected inner replicate count 3, got {other:?}"),
                    }
                    assert_eq!(inner_parts.len(), 1);
                }
                other => panic!("expected a nested Expr::AssignmentPattern, got {other:?}"),
            }
        }
        other => panic!("expected Expr::AssignmentPattern, got {other:?}"),
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
