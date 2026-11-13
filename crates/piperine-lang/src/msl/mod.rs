//! # MSL front end
//!
//! Parser for the Piperine mixed-signal language specified in `docs/spec/`
//! (Appendix A lexical structure, Appendix B.3 grammar): text to tokens to a
//! spanned AST, plus diagnostics. It shares nothing with the Verilog-AMS
//! modules of this crate.

pub mod diag;
pub mod span;

pub use diag::{Diagnostic, render};
pub use span::Span;
