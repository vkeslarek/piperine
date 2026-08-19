//! Source-level surface types: [`Attr`] (attribute instances), the root
//! [`SourceFile`] node, and the top-level [`Item`] dispatch.

use crate::error::PreprocessedSpan;

use super::expr::Expr;
use super::item::{ConfigDecl, ConnectrulesDecl, DisciplineDecl, ModuleDecl, NatureDecl, ParamsetDecl, PrimitiveDecl};

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

