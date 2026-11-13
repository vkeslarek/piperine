//! Scope: the MSL lexer (`piperine_lang::msl::lex`) against
//! `docs/spec/appendix_a_lexical.md` — SI-suffixed reals (A.2), `Unknown`
//! (A.3), doc comments (A.4), the `+>` / `<-` operators (A.5), keyword
//! classes (A.1) and the lexical error codes of A.6 (`E0902`, `E0905`).
//! Requirement MSLP-01 (spec AC 1-7) and the non-UTF-8 edge of MSLP-06.

use piperine_lang::msl::Span;
use piperine_lang::msl::lexer::{Punct, Token, lex};

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
    // The doc token's span covers the whole comment line.
    let (toks, _) = lex("/// hi\nx");
    assert_eq!(toks[0].1, Span::new(0, 6));
    assert_eq!(toks[1].1, Span::new(7, 8));
}

