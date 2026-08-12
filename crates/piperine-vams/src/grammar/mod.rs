//! Recursive-descent + Pratt grammar for LRM 2.4, Annex A — one method per
//! production, split by area: [`cursor`] (the shared token cursor and
//! lookahead/expect helpers every production is written against),
//! `expr` (Annex A.8, chapter 4), `stmt` (Annex A.6, chapter 5 + inherited
//! 1364 procedural), `item` (Annex A.1-A.5/A.7/A.9, declarations).
//!
//! Each production method's doc comment names the Annex A production it
//! implements — this is what `tests/annex_a_grammar.rs` (VAMS-23) indexes
//! to prove grammar completeness mechanically rather than by inspection.

pub(crate) mod cursor;
mod expr;
mod item;
mod stmt;

pub use cursor::Grammar;
