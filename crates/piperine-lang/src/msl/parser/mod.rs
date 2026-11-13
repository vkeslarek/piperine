//! MSL parser: spanned tokens to AST, built from `chumsky` combinators.
//!
//! The grammar is split by area: `ty` (types, paths, generics), `pat`
//! (patterns), `expr` (expressions and statements), `item` (Rust items).
//! Mutually recursive parts (an expression holds types, a type holds
//! constant expressions) reach each other through the declared [`Handles`].

use chumsky::prelude::*;

use super::ast::*;
use super::diag::Diagnostic;
use super::lexer::{Kw, Punct, Token, lex, lex_bytes};
use super::span::Span;

mod base;
mod check;
mod entry;
pub mod expr;
pub mod item;
pub mod msl;
pub mod pat;
pub mod ty;

pub(crate) use base::*;
pub use base::Custom;
pub use entry::{MAX_DEPTH, ParseResult, parse, parse_block, parse_bytes, parse_expr, parse_generics, parse_module_item, parse_pat, parse_type};
