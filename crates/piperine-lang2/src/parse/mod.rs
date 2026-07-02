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
//! let source = parse::parse_str("mod R (inout p: Electrical, inout n: Electrical);")?;
//! # Ok::<(), String>(())
//! ```

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use lexer::{Lexed, Lexer, Tok};
pub use parser::Parser;

/// Parse a PHDL source string into a [`SourceFile`].
///
/// This is the canonical entry point for the parse phase. It runs the lexer
/// then the parser and returns the raw AST on success.
pub fn parse_str(input: &str) -> Result<SourceFile, String> {
    let tokens = Lexer::new(input).tokenize()?;
    Parser::new(&tokens).parse_file()
}
