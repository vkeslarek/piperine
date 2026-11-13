//! # MSL front end
//!
//! Parser for the Piperine mixed-signal language specified in `docs/spec/`
//! (Appendix A lexical structure, Appendix B.3 grammar): text to tokens to a
//! spanned AST, plus diagnostics. It shares nothing with the Verilog-AMS
//! modules of this crate.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod span;

pub use diag::{Diagnostic, render};
pub use lexer::{lex, lex_bytes};
pub use span::Span;
