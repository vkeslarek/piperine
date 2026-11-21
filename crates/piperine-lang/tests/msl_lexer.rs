//! Scope: the MSL lexer (`piperine_lang::msl::lex`) against
//! `docs/spec/appendix_a_lexical.md` — SI-suffixed reals (A.2), `Unknown`
//! (A.3), doc comments (A.4), the `+>` / `<-` operators (A.5), keyword
//! classes (A.1) and the lexical error codes of A.6 (`E0902`, `E0905`).
//! Requirement MSLP-01 (spec AC 1-7) and the non-UTF-8 edge of MSLP-06.

use piperine_lang::msl::lexer::{Punct, Token, lex, lex_bytes};
use piperine_lang::msl::{Diagnostic, Span};

fn tokens(src: &str) -> Vec<Token> {
    let (toks, diags) = lex(src);
    assert!(diags.is_empty(), "unexpected diagnostics for {src:?}: {diags:?}");
    toks.into_iter().map(|(t, _)| t).collect()
}

fn one_token(src: &str) -> Token {
    let mut t = tokens(src);
    assert_eq!(t.len(), 1, "{src:?} should lex to one token, got {t:?}");
    t.remove(0)
}

fn diagnostics(src: &str) -> Vec<Diagnostic> {
    lex(src).1
}

fn plain(text: &str) -> Token {
    Token::Str { text: text.to_string(), raw_hashes: None }
}

fn raw(text: &str, hashes: u8) -> Token {
    Token::Str { text: text.to_string(), raw_hashes: Some(hashes) }
}

fn float(value: f64, raw: &str) -> Token {
    Token::Float { value, raw: raw.to_string() }
}

#[test]
fn si_literal_scaled_value_and_raw_text() {
    // Appendix A.2 AC 1: one float token, scaled value, original text kept.
    assert_eq!(one_token("50m"), float(0.05, "50m"));
    assert_eq!(one_token("1p"), float(1e-12, "1p"));
    // Rule 6: the nearest f64 to the exact decimal product, i.e. the
    // literal 2.2e-6, not the rounded product 2.2 * 1e-6.
    assert_eq!(one_token("2.2u"), float(2.2e-6, "2.2u"));
    // Rule 1: integer-looking SI literals are f64 too.
    assert_eq!(one_token("1k"), float(1000.0, "1k"));
    // Rule 2: case-sensitive, `m` milli vs `M` mega.
    assert_eq!(one_token("1m"), float(1e-3, "1m"));
    assert_eq!(one_token("1M"), float(1e6, "1M"));
    // The whole multiplier table.
    for (suffix, mult) in [
        ("T", "e12"), ("G", "e9"), ("M", "e6"), ("k", "e3"), ("m", "e-3"),
        ("u", "e-6"), ("n", "e-9"), ("p", "e-12"), ("f", "e-15"), ("a", "e-18"),
    ] {
        let src = format!("2.5{suffix}");
        let expected: f64 = format!("2.5{mult}").parse().unwrap();
        assert_eq!(one_token(&src), float(expected, &src), "suffix {suffix}");
    }
    // Underscores are part of DEC_DIGITS.
    assert_eq!(one_token("1_000k"), float(1e6, "1_000k"));
    // Rule 3: a letter followed by identifier characters is a type suffix.
    assert_eq!(one_token("1u8"), Token::Int("1u8".to_string()));
    assert_eq!(one_token("1f64"), float(1.0, "1f64"));
    assert_eq!(one_token("1u"), float(1e-6, "1u"));
    // Rule 7: a leading `-` is the negation operator.
    assert_eq!(tokens("-1k"), vec![Token::Punct(Punct::Minus), float(1000.0, "1k")]);
    // Plain literals stay plain.
    assert_eq!(one_token("42"), Token::Int("42".to_string()));
    assert_eq!(one_token("1.5"), float(1.5, "1.5"));
    assert_eq!(one_token("1e-3"), float(1e-3, "1e-3"));
    // A range and a method call on an integer are not floats.
    assert_eq!(
        tokens("1..2"),
        vec![Token::Int("1".into()), Token::Punct(Punct::DotDot), Token::Int("2".into())]
    );
    // An exponent with no digits is an invalid literal, as in Rust.
    assert_eq!(diagnostics("1e+x")[0].code, "E0900");
}

#[test]
fn malformed_si_literal_reports_e0902() {
    // Appendix A.2 rules 4-5: exponent, type suffix, non-decimal base.
    for src in ["1e3m", "5mf64", "0x10m", "1e3k", "1k_f32", "0b1a"] {
        let d = diagnostics(src);
        assert_eq!(d.len(), 1, "{src:?}: {d:?}");
        assert_eq!(d[0].code, "E0902", "{src:?}");
        assert_eq!(d[0].span(), Span::new(0, src.len()), "{src:?} span");
    }
    // Located at the literal, not at the start of the line.
    let d = diagnostics("let x = 1e3m;");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].code, "E0902");
    assert_eq!(d[0].span(), Span::new(8, 12));
}

#[test]
fn dropped_token_class_reports_e0905() {
    // Appendix A.1 resolution + A.6: char, byte, byte string, lifetime.
    for src in ["'a'", "b'a'", "b\"x\"", "'a", "'static"] {
        let d = diagnostics(src);
        assert_eq!(d.len(), 1, "{src:?}: {d:?}");
        assert_eq!(d[0].code, "E0905", "{src:?}");
        assert_eq!(d[0].span(), Span::new(0, src.len()), "{src:?} span");
    }
    // The lifetime of a reference is part of the removed reference form (B.3.6): one E0905 and no
    // token, so the reference reports itself.
    let (toks, diags) = lex("&'static str");
    assert_eq!(toks.into_iter().map(|(t, _)| t).collect::<Vec<_>>(), vec![Token::Punct(Punct::And), Token::Ident("str".into())]);
    assert_eq!(diags.len(), 1);
}

#[test]
fn contribution_and_commit_operators_are_single_tokens() {
    // Appendix A.5: longest match, one token each.
    assert_eq!(one_token("+>"), Token::Punct(Punct::PlusGt));
    assert_eq!(one_token("<-"), Token::Punct(Punct::LtMinus));
    // Rule 3: `a<-b` is a commit; `a < -b` is a comparison.
    assert_eq!(
        tokens("a<-b"),
        vec![Token::Ident("a".into()), Token::Punct(Punct::LtMinus), Token::Ident("b".into())]
    );
    assert_eq!(
        tokens("a < -b"),
        vec![
            Token::Ident("a".into()),
            Token::Punct(Punct::Lt),
            Token::Punct(Punct::Minus),
            Token::Ident("b".into()),
        ]
    );
    // Rule 5: `a+>b` is `a`, `+>`, `b`.
    assert_eq!(
        tokens("a+>b"),
        vec![Token::Ident("a".into()), Token::Punct(Punct::PlusGt), Token::Ident("b".into())]
    );
    // Neighbours keep their own longest match.
    assert_eq!(one_token("+="), Token::Punct(Punct::PlusEq));
    assert_eq!(one_token("->"), Token::Punct(Punct::Arrow));
    assert_eq!(one_token("<="), Token::Punct(Punct::Le));
    assert_eq!(one_token("<<="), Token::Punct(Punct::ShlEq));
}

#[test]
fn keywords_reserved_vs_contextual() {
    // Appendix A.1 keyword table.
    let added = ["module", "nature", "signal", "on", "out", "flip", "after"];
    let rust_2024 = [
        // strict
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false",
        "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn",
        // reserved for future use
        "abstract", "become", "box", "do", "final", "macro", "override", "priv",
        "typeof", "unsized", "virtual", "yield", "try", "gen",
    ];
    for word in added.iter().chain(rust_2024.iter()) {
        match one_token(word) {
            Token::Kw(k) => assert_eq!(k.as_str(), *word),
            other => panic!("{word:?} should be a reserved keyword, got {other:?}"),
        }
    }
    // Contextual keywords and prelude names are ordinary identifiers.
    for word in [
        "analog", "digital", "potential", "flow", "value", "resolve", "gnd", "settle",
        "inf", "emit", "include_str", "macro_rules", "union", "u8", "str_", "outer",
    ] {
        assert_eq!(one_token(word), Token::Ident(word.to_string()), "{word}");
    }
    // `Unknown` is a literal token, not an identifier (A.3); `_` is the wildcard.
    assert_eq!(one_token("Unknown"), Token::Unknown);
    assert_eq!(one_token("unknown"), Token::Ident("unknown".into()));
    assert_eq!(one_token("_"), Token::Punct(Punct::Underscore));
    assert_eq!(one_token("_x"), Token::Ident("_x".into()));
}

#[test]
fn doc_comments_are_tokens() {
    // Appendix A.4: `///` and `//!` carry their text; `//` and `/* */` vanish.
    let src = "/// outer doc\n//! inner doc\n// plain\n/* block */ x";
    assert_eq!(
        tokens(src),
        vec![
            Token::DocOuter(" outer doc".into()),
            Token::DocInner(" inner doc".into()),
            Token::Ident("x".into()),
        ]
    );
    // Rust rule: four slashes is an ordinary comment.
    assert_eq!(tokens("//// not doc\ny"), vec![Token::Ident("y".into())]);
    // Block comments nest, as in Rust.
    assert_eq!(tokens("/* a /* b */ c */ z"), vec![Token::Ident("z".into())]);
    // Block doc comments are doc tokens too, as in Rust: `/** */` outer,
    // `/*! */` inner; `/***`, `/**/` and `/* */` are plain comments.
    assert_eq!(
        tokens("/** a */ /*! b */ /*** c */ /**/ /* d */ x"),
        vec![
            Token::DocOuterBlock(" a ".into()),
            Token::DocInnerBlock(" b ".into()),
            Token::Ident("x".into()),
        ]
    );
    // The doc token's span covers the whole comment line.
    let (toks, _) = lex("/// hi\nx");
    assert_eq!(toks[0].1, Span::new(0, 6));
    assert_eq!(toks[1].1, Span::new(7, 8));
}

#[test]
fn unterminated_comment_and_string_one_diagnostic() {
    // Appendix A / MSLP-01 AC 7: one diagnostic at the opening position,
    // and lexing continues to end of file without another one.
    let src = "a /* never closed\nb c";
    let (toks, diags) = lex(src);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].span().start, 2);
    assert_eq!(toks[0].0, Token::Ident("a".into()));
    // The rest of the file is inside the comment: only `a` was lexed.
    assert_eq!(toks.len(), 1, "{toks:?}");

    let src = "x = \"never closed\ny";
    let (toks, diags) = lex(src);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].span().start, 4);
    // `x`, `=`, then one error token standing for the string, which
    // swallowed the rest of the file.
    let kinds: Vec<&Token> = toks.iter().map(|(t, _)| t).collect();
    assert_eq!(kinds, vec![&Token::Ident("x".into()), &Token::Punct(Punct::Eq), &Token::Error]);

    // A terminated string with an escaped quote is fine.
    assert_eq!(tokens(r#""a\"b""#), vec![plain(r#"a\"b"#)]);
}

#[test]
fn non_utf8_one_diagnostic() {
    // MSLP-06: invalid UTF-8 yields one diagnostic and no tokens.
    let (toks, diags) = lex_bytes(b"let x = \xff\xfe;");
    assert!(toks.is_empty());
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].span().start, 8);
    // Valid UTF-8 bytes lex like the string.
    let (toks, diags) = lex_bytes("x".as_bytes());
    assert!(diags.is_empty());
    assert_eq!(toks.len(), 1);
}

#[test]
fn inherited_lexical_forms_lex() {
    // Appendix A.1: identifiers, literals and the value syntax are Rust's.
    // Raw strings: one string token, spanning `r#*"` to the closing run.
    let (toks, diags) = lex(r###"r"a\b" r#"q"q"# r##"x"#y"##"###);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(toks.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>(), vec![
        raw("a\\\\b", 0),
        raw("q\\\"q", 1),
        raw("x\\\"#y", 2),
    ]);
    assert_eq!(toks[0].1, Span::new(0, 6));
    // An unterminated raw string is one diagnostic at its opener.
    let d = diagnostics("a r#\"never closed");
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!((&*d[0].code, d[0].span()), ("E0900", Span::new(2, 5)));
    // A raw byte string is a dropped class.
    assert_eq!(diagnostics("br\"x\"")[0].code, "E0905");
    // `1.` is one float token; the dot of `1..`, `1.f` and `1._` stays apart.
    assert_eq!(one_token("1."), float(1.0, "1."));
    assert_eq!(tokens("1..2").len(), 3);
    assert_eq!(tokens("1.max").len(), 3);
    // Non-ASCII identifiers, and a byte order mark that is not a token.
    assert_eq!(one_token("café"), Token::Ident("café".into()));
    assert_eq!(one_token("µ"), Token::Ident("µ".into()));
    let (toks, diags) = lex("\u{FEFF}x");
    assert!(diags.is_empty());
    assert_eq!(toks, vec![(Token::Ident("x".into()), Span::new(3, 4))]);
}

#[test]
fn raw_identifier_of_an_added_keyword_is_an_identifier() {
    // POL-10 AC 1 / A.1 rule 6: `r#word` for each of the seven added keywords
    // is one raw identifier token holding the word without the prefix.
    for word in ["module", "nature", "signal", "on", "out", "flip", "after"] {
        let src = format!("r#{word}");
        let (toks, diags) = lex(&src);
        assert!(diags.is_empty(), "{src}: {diags:?}");
        assert_eq!(toks.len(), 1, "{src}: {toks:?}");
        assert_eq!(toks[0].0, Token::RawIdent(word.to_string()), "{src}");
        assert_eq!(toks[0].1, Span::new(0, src.len()), "{src}");
    }
    // The raw-string openers keep lexing as strings.
    assert_eq!(one_token("r#\"a\"#"), raw("a", 1));
    assert_eq!(one_token("r\"a\""), raw("a", 0));
    // A bare added keyword is still a keyword.
    assert!(matches!(one_token("out"), Token::Kw(_)));
}

fn only_code(src: &str, code: &str, span: Span) {
    let (toks, diags) = lex(src);
    assert_eq!(diags.len(), 1, "{src:?}: {diags:?}");
    assert_eq!(diags[0].code, code, "{src:?}");
    assert_eq!(diags[0].span(), span, "{src:?}");
    // The token is still produced, so the parse goes on.
    assert_eq!(toks.len(), 1, "{src:?}: {toks:?}");
}

#[test]
fn float_literal_out_of_range_is_e0909() {
    // POL-13 AC 2.
    only_code("1e999", "E0909", Span::new(0, 5));
    let big = "9".repeat(310);
    let src = format!("{big}T");
    only_code(&src, "E0909", Span::new(0, src.len()));
    let src = format!("{big}.5T");
    only_code(&src, "E0909", Span::new(0, src.len()));
    assert_eq!(one_token("1.7976931348623157e308"), float(f64::MAX, "1.7976931348623157e308"));
    assert!(matches!(one_token("5e-324"), Token::Float { .. }));
}

#[test]
fn integer_literal_out_of_range_is_e0909() {
    // POL-13 AC 3: both sides of each boundary.
    for src in ["256u8", "2u1", "129i8", "65536u16", "340282366920938463463374607431768211456", "0x100u8"] {
        only_code(src, "E0909", Span::new(0, src.len()));
    }
    for src in [
        "255u8", "128i8", "1u1", "340282366920938463463374607431768211455", "0xffu8",
        "0xffffffffffffffffffffffffffffffff",
    ] {
        assert!(matches!(one_token(src), Token::Int(_)), "{src}");
    }
}

#[test]
fn invalid_string_escape_is_e0900() {
    // POL-13 AC 5: one E0900 at each undefined escape.
    for (src, span) in [
        (r#""\q""#, Span::new(1, 3)),
        (r#""ab\x9""#, Span::new(3, 6)),
        (r#""\u{110000}""#, Span::new(1, 11)),
    ] {
        let (_, diags) = lex(src);
        assert_eq!(diags.len(), 1, "{src}: {diags:?}");
        assert_eq!(diags[0].code, "E0900", "{src}");
        assert_eq!(diags[0].span(), span, "{src}");
    }
    // Two bad escapes, two diagnostics; the defined ones are clean.
    assert_eq!(lex(r#""\q\z""#).1.len(), 2);
    assert!(
        diagnostics(r#""\n\r\t\\\0\'\"\x41\u{1F600}\u{41}""#).is_empty(),
        "every Rust escape is defined"
    );
    // A raw string has no escapes.
    assert!(diagnostics(r##"r#"\q \u{110000}"#"##).is_empty());
}

fn literal(src: &str) -> piperine_lang::msl::ast::Lit {
    use piperine_lang::msl::ast::ExprKind;
    let (expr, diags) = piperine_lang::msl::parser::parse_expr(src);
    let _ = diags;
    match expr.expect("an expression").kind {
        ExprKind::Lit(l) => l,
        other => panic!("{src}: not a literal: {other:?}"),
    }
}

#[test]
fn literal_accessors_return_cooked_values() {
    // POL-13 AC 7.
    assert_eq!(literal(r#""a\tb\n\u{41}\x42""#).str_value().as_deref(), Some("a\tb\nAB"));
    assert_eq!(literal(r##"r#"a"b\c"#"##).str_value().as_deref(), Some(r#"a"b\c"#));
    assert_eq!(literal("1").str_value(), None);
    assert_eq!(literal("0xFF").int_value(), Some(255));
    assert_eq!(literal("0b1010_1u8").int_value(), Some(0b10101));
    assert_eq!(literal("0o17").int_value(), Some(15));
    assert_eq!(literal("1_000_000i32").int_value(), Some(1_000_000));
    assert_eq!(literal("340282366920938463463374607431768211455").int_value(), Some(u128::MAX));
    assert_eq!(literal("1.5").int_value(), None);
    // A literal E0909 rejects has no value.
    assert_eq!(literal("256u8").int_value(), None);
    assert_eq!(literal("340282366920938463463374607431768211456").int_value(), None);
    assert_eq!(literal("128i8").int_value(), Some(128));
    assert_eq!(literal("255u8").suffix(), Some("u8"));
    assert_eq!(literal("0xffi64").suffix(), Some("i64"));
    assert_eq!(literal("7").suffix(), None);
    assert_eq!(literal("1.5f32").suffix(), Some("f32"));
    assert_eq!(literal("2.2u").suffix(), Some("u"));
    assert_eq!(literal("1e3").suffix(), None);
    assert_eq!(literal(r#""s""#).suffix(), None);
}
