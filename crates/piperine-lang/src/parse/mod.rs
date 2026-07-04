//! # Parse phase
//!
//! Converts raw PHDL source text into an unresolved [`SourceFile`] AST.
//!
//! ```text
//! &str  ──Lexer──▶  Vec<Lexed>  ──Parser──▶  SourceFile (parse AST)
//! ```
//!
//! ## What this phase produces
//!
//! A syntactically valid tree. Every syntactic construct in the PHDL grammar
//! (§2–§8 of the grammar spec) is represented. **No semantic guarantees** are
//! made:
//!
//! | Not checked here | Where it is checked |
//! |------------------|---------------------|
//! | Type names resolved | [`crate::elab`] |
//! | Array dimensions are const | [`crate::elab::const_eval`] |
//! | `<+` only in `analog` | [`crate::elab::validate`] |
//! | Event names are valid | [`crate::elab::event`] |
//! | Bundle ports are net-capable | [`crate::elab::lower`] |
//! | Generic params substituted | [`crate::elab::lower`] |
//!
//! ## Entry point
//!
//! ```rust
//! use piperine_lang::parse;
//!
//! let source = parse::parse_str("mod R (inout p: Electrical, inout n: Electrical);").map_err(|e| e.to_string())?;
//! # Ok::<(), String>(())
//! ```

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod error;
pub mod format;
pub mod predict;

pub use ast::*;
pub use lexer::{Lexed, Lexer, Tok};
pub use parser::Parser;

/// Parse a PHDL source string into a [`SourceFile`].
///
/// This is the canonical entry point for the parse phase. It runs the lexer
/// then the parser and returns the raw AST on success.
pub fn parse_str(input: &str) -> Result<SourceFile, error::ParseError> {
    let tokens = Lexer::new(input).tokenize().map_err(|e| error::ParseError::Legacy { message: e })?;
    let (file, errors) = Parser::new(&tokens).parse_file();
    if let Some(err) = errors.into_iter().next() {
        return Err(err);
    }
    Ok(file)
}

pub fn parse_str_tolerant(input: &str) -> (SourceFile, Vec<error::ParseError>) {
    match Lexer::new(input).tokenize() {
        Ok(tokens) => Parser::new(&tokens).parse_file(),
        Err(e) => (SourceFile { items: vec![] }, vec![error::ParseError::Legacy { message: e }]),
    }
}

pub fn predict_at_cursor(input: &str, cursor_offset: usize) -> Vec<predict::ExpectedSyntax> {
    let tokens = match Lexer::new(input).tokenize() {
        Ok(t) => t,
        Err(_) => return vec![], // fallback
    };
    let mut parser = Parser::with_cursor(&tokens, cursor_offset);
    let _ = parser.parse_file();
    parser.expectations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predict_at_cursor_mod_body() {
        let source = "mod Res2 (inout p: Electrical, inout n: Electrical) {\n    \n}";
        // offset 58 is right after the spaces on the second line
        let expectations = predict_at_cursor(source, 58);
        println!("EXPECTATIONS: {:#?}", expectations);
        assert!(!expectations.is_empty(), "Should not be empty!");
    }

    #[test]
    fn test_port_prediction() {
        let source = "mod Res2 ( )";
        // offset 10 is inside ( )
        let cursor = 10;
        let expected = predict_at_cursor(source, cursor);
        println!("EXPECTATIONS 10: {:#?}", expected);
    }
}
