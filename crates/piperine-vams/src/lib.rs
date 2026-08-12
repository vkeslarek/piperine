//! # piperine-vams
//!
//! Verilog-AMS text -> AST, and nothing else. Zero in-tree and zero external
//! dependencies (mechanically enforced by `tests/purity.rs`) — this crate
//! never sees the POM, the solver ABI, or Cranelift; diagnostic rendering
//! (source spans tied to file identity, `miette`) happens at the
//! `piperine-lang` boundary, which turns this crate's plain byte `Span`s
//! into user-facing diagnostics.
//!
//! Every grammar production this crate implements is written from
//! `VAMS-LRM-2-4.md` Annex A (the normative formal syntax) and cited by
//! clause in the implementing method's doc comment. No third-party
//! Verilog-A/AMS compiler — its code, its grammar files, or its comments —
//! is consulted as a source. See `.specs/features/verilog-ams-support/design.md`
//! §0 (F3) for why, and `tests/purity.rs` for the mechanical gate that
//! enforces it.

pub mod ast;
pub mod error;
pub mod grammar;
pub mod keywords;
pub mod lexer;
pub mod preprocessor;

pub use error::{ParseError, ParseErrorKind, Span};
pub use keywords::Keywords;
pub use lexer::{BasedNumber, Lexed, Lexer, NumberBase, NumberLit, Tok};
pub use preprocessor::{IncludeResolver, NoIncludes, Preprocessor};
pub use ast::{Attr, Item, SourceFile};
pub use grammar::Grammar;
