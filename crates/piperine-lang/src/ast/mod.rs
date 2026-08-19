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

/// One attribute instance entry, `name` or `name = value`, from an
/// `attr_spec` inside `(* ... *)` (§2.9.1, Syntax 2-11/2-12).
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub span: PreprocessedSpan,
    pub name: String,
    pub value: Option<Expr>,
}

/// The root of a parsed (and preprocessed) Verilog-AMS source file —
/// Annex A.1.1 `source_text`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceFile {
    pub items: Vec<Item>,
    /// Ch.10 compiler-directive state the preprocessor accumulated while
    /// scanning this source unit (design.md P0.3) — attached by
    /// [`crate::parser::Parser::parse`] after `Grammar::parse_source_file`
    /// returns (the grammar layer itself has no preprocessor access).
    pub directives: crate::directives::DirectiveState,
    /// Maps every `PreprocessedSpan` minted while lexing/parsing this
    /// source unit back to its real originating file (and, for
    /// macro-expanded text, invocation context) — built by
    /// `Preprocessor::run`, attached by [`crate::parser::Parser::parse`]
    /// alongside `directives` (preprocessor-span-mapping feature, T7).
    pub source_provenance: crate::source_provenance::SourceProvenance,
}

/// A top-level source item — Annex A.1.1 `description`.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Discipline(DisciplineDecl),
    Nature(NatureDecl),
    Module(ModuleDecl),
    Paramset(ParamsetDecl),
    Connectrules(ConnectrulesDecl),
    Config(ConfigDecl),
    Primitive(PrimitiveDecl),
    /// `const <name> = <constant_expression>;` — a file-scope global
    /// constant (a sanctioned Piperine extension: Annex A.1.1's
    /// `description` has no such production; MD-50 records it alongside
    /// §1.1's doc comments and §12's constraint block). Elaboration
    /// const-evals it into `Design::consts`, the surface `LowerCtx::
    /// const_irs` already reads into every module's analog scope.
    Const(crate::ast::ConstDecl),
}
