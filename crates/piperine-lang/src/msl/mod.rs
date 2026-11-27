//! # MSL front end
//!
//! Parser for the Piperine mixed-signal language specified in `docs/spec/`
//! (Appendix A lexical structure, Appendix B.3 grammar): text to tokens to a
//! spanned AST, plus diagnostics; the plain comments beside the tokens and
//! the formatter that keeps them.

pub mod ast;
pub mod comments;
pub mod diag;
pub mod format;
pub mod lexer;
pub mod parser;
pub mod print;
pub mod span;

pub use diag::{Diagnostic, render};
pub use lexer::Lexer;
pub use print::print;
pub use span::Span;
