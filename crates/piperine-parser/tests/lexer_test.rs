//! Locks number lexing to match OpenVAF's cursor: scale chars consumed
//! unconditionally; no scale char after an exponent.

use cvaf::lexer::{tokenize, Tok};

fn toks(s: &str) -> Vec<Tok> {
    tokenize(s).unwrap().into_iter().map(|l| l.tok).collect()
}

#[test]
fn numbers() {
    assert_eq!(toks("123"), vec![Tok::Int("123".into())]);
    assert_eq!(toks("1.5"), vec![Tok::Real("1.5".into())]);
    assert_eq!(toks("2.0e-12"), vec![Tok::Real("2.0e-12".into())]);
    assert_eq!(toks("1.5p"), vec![Tok::Real("1.5p".into())]);
    assert_eq!(toks("123k"), vec![Tok::Real("123k".into())]);
    assert_eq!(toks("1e3"), vec![Tok::Real("1e3".into())]);

    // scale char glued to a number, trailing letters split off as an ident
    assert_eq!(toks("123kHz"), vec![Tok::Real("123k".into()), Tok::Ident("Hz".into())]);

    // no scale char after an exponent
    assert_eq!(toks("1e3k"), vec![Tok::Real("1e3".into()), Tok::Ident("k".into())]);

    // arithmetic shifts glue as three-char tokens
    assert_eq!(toks("a<<<b"), vec![Tok::Ident("a".into()), Tok::Shl, Tok::Ident("b".into())]);
    assert_eq!(toks("a>>>b"), vec![Tok::Ident("a".into()), Tok::Shr, Tok::Ident("b".into())]);
}
