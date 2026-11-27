//! MSL parser: spanned tokens to AST, built from `chumsky` combinators.
//!
//! The grammar is split by area: `ty` (types, paths, generics), `pat`
//! (patterns), `expr` (expressions and statements), `item` (Rust items).
//! Mutually recursive parts (an expression holds types, a type holds
//! constant expressions) reach each other through the declared [`Handles`].

use chumsky::prelude::*;

use super::ast::*;
use super::diag::Diagnostic;
use super::lexer::{Kw, Lexer, Punct, Token};
use super::span::Span;

mod base;
mod check;
mod diagnostics;
mod entry;
pub mod expr;
pub mod item;
pub mod msl;
pub mod pat;
pub mod ty;
mod input;
mod recovery;

pub(crate) use base::*;
pub(crate) use diagnostics::ParseDiagnostics;
pub(crate) use input::TokenInput;
pub(crate) use recovery::Recovery;
pub use base::Custom;
pub use expr::{BlockGrammar, ExpressionGrammar};
pub use msl::ModuleGrammar;
pub(crate) use check::ParseCheck;
pub use item::ItemGrammar;
pub use pat::PatternGrammar;
pub use ty::{GenericsGrammar, TypeGrammar};
pub use entry::{MAX_CHAIN, MAX_DEPTH, ParseResult, SourceParser};
