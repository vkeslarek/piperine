//! POM constraint nodes — the `constraint` block (dv-core DVC-04/05) and
//! the pointwise/reduced quantity classification (DVC-09).

use std::collections::HashMap;

pub use crate::parse::ast::{ConstraintStmt, TargetCmp};
use crate::pom::net_type::ValueType;

/// What elaboration proved about one constraint statement's quantity
/// (dv-core T10, DVC-09): its pointwise/reduced class, and the analyses
/// its helpers belong to — recorded so a later pass can refuse a
/// cross-analysis read (DVC-08 AC6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StmtClass {
    /// The statement's class. A `require` is always `Pointwise` (a reduced
    /// predicate is an elaboration error, not a class); a `var`/`target`
    /// carries the class of its expression.
    pub class: QuantityClass,
    /// The analyses this statement's helpers belong to (`"ac"`, …),
    /// sorted and deduplicated. Empty for a quantity built from probes and
    /// constants alone — readable in every analysis.
    pub analyses: Vec<String>,
}

/// A `constraint Name { ... }` block attached to a module (dv-core DVC-05).
/// Like [`Behavior`][crate::pom::Behavior], the block stays the *authored*
/// surface form — the statements are the parse AST's
/// [`ConstraintStmt`]s; what elaboration adds lives in side tables
/// ([`ConstraintBlock::var_types`], [`ConstraintBlock::stmt_classes`],
/// mirroring `Behavior::var_types`). Never a flattened structure (MD-25).
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
    /// Per-statement classification, parallel to [`Self::stmts`] (DVC-09).
    pub stmt_classes: Vec<StmtClass>,
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
    /// Per-statement classifications, parallel to [`Self::stmts`].
    pub fn classes(&self) -> &[StmtClass] { &self.stmt_classes }
    /// The classification of the statement named `name` (a `require` or a
    /// `var`), if one exists.
    pub fn class_of(&self, name: &str) -> Option<&StmtClass> {
        self.stmts.iter().zip(&self.stmt_classes).find_map(|(stmt, class)| match stmt {
            ConstraintStmt::Require { name: n, .. } | ConstraintStmt::Var { name: n, .. } if n == name => Some(class),
            _ => None,
        })
    }
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
