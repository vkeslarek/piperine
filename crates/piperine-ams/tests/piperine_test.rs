//! Piperine-specific tests.

use piperine_ams::lexer::{Lexer, Tok};

#[test]
fn time_literals_lex() {
    let t: Vec<Tok> = Lexer::tokenize("1ns 5ms 1k 1n")
        .unwrap()
        .into_iter()
        .map(|l| l.tok)
        .collect();
    assert_eq!(
        t,
        vec![
            Tok::Real("1ns".into()),
            Tok::Real("5ms".into()),
            Tok::Real("1k".into()),
            Tok::Real("1n".into()),
        ]
    );
}
