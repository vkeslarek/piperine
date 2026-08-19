//! Abstract syntax tree for Verilog-AMS LRM 2.4, written from Annex A
//! (normative formal syntax) — see the crate root `//!` header and
//! `.specs/features/verilog-ams-support/design.md` §0 (F3) for the
//! provenance constraint this module is gated by (`tests/purity.rs`).
//!
//! Split by area — [`expr`] (expressions, Annex A.8), [`stmt`] (procedural
//! and analog statements, Annex A.6), [`item`] (declarations, Annex A.1-A.5,
//! A.7, A.9) — all re-exported here so consumers can `use crate::ast::*`.
//! Attribute instances (`(* ... *)`, §2.9, Syntax 2-5..2-10) attach only to
//! the node kinds the LRM actually permits them on: each such node carries
//! its own `attrs: Vec<Attr>` field, so a node that cannot legally carry an
//! attribute simply has no field for one — an illegal attribute site is
//! unrepresentable rather than merely unchecked.

mod expr;
mod item;
mod stmt;

pub use expr::*;
pub use item::*;
pub use stmt::*;

pub use crate::error::PreprocessedSpan;

mod source;

pub use source::{Attr, Item, SourceFile};

