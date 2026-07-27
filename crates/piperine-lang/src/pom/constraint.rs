//! POM constraint nodes — the `constraint` block (dv-core DVC-04/05) and
//! the pointwise/reduced quantity classification (DVC-09).

use std::collections::HashMap;

pub use crate::parse::ast::{ConstraintStmt, TargetCmp};
use crate::pom::net_type::ValueType;

/// A `constraint Name { ... }` block attached to a module (dv-core DVC-05).
/// Like [`Behavior`][crate::pom::Behavior], the block stays the *authored*
/// surface form — the statements are the parse AST's
/// [`ConstraintStmt`]s; what elaboration adds lives in side tables
/// ([`ConstraintBlock::var_types`], mirroring `Behavior::var_types`).
/// Never a flattened structure (MD-25).
#[derive(Debug, Clone)]
pub struct ConstraintBlock {
    pub span: Option<miette::SourceSpan>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07). Not
    /// serde-carried: the block holds compiled AST and does not cross the
    /// serialization boundary (same rule as `Behavior`).
    pub doc: Option<String>,
    /// The module this block attaches to.
    pub name: String,
    /// The block's statements in authored order.
    pub stmts: Vec<ConstraintStmt>,
    /// Resolved value types of the block's `var` annotations, keyed by
    /// name — the side table elaboration adds on top of the surface AST.
    pub var_types: HashMap<String, ValueType>,
}

impl ConstraintBlock {
    /// The module this block attaches to.
    pub fn name(&self) -> &str { &self.name }
    /// The block's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
    /// The block's statements in authored order.
    pub fn stmts(&self) -> &[ConstraintStmt] { &self.stmts }
    /// The resolved value type of a `var` declared in this block.
    pub fn var_type(&self, name: &str) -> Option<&ValueType> { self.var_types.get(name) }
}

/// Whether a constraint quantity exists at every evaluation point of an
/// analysis or only once the analysis has run to completion (design §C2 —
/// the class is a property of the *declaration*, read off
/// `headers/constraints.phdl`'s `@quantity(class, analysis)` marker, never
/// inferred from a use site).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum QuantityClass {
    /// Evaluable at every point (`ac_gain`, `V()`, `I()`) — lowers to a
    /// per-point margin kernel.
    Pointwise,
    /// Exists once per analysis (`ac_unity_gain_freq`, `ac_phase_margin`) —
    /// a host-side reduction over the analysis's points, never a kernel.
    Reduced,
}

impl std::fmt::Display for QuantityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointwise => write!(f, "pointwise"),
            Self::Reduced => write!(f, "reduced"),
        }
    }
}
