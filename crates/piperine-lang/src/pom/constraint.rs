//! POM constraint nodes — the `constraint` block (dv-core DVC-04/05) and
//! the pointwise/reduced quantity classification (DVC-09).

use std::collections::HashMap;

pub use crate::parse::ast::{ConstraintStmt, TargetCmp};
use crate::pom::net_type::ValueType;

/// What elaboration proved about one **leaf** constraint statement's
/// quantity (dv-core T10, DVC-09): its pointwise/reduced class, and the
/// analyses its helpers belong to — recorded so a later pass can refuse a
/// cross-analysis read (DVC-08 AC6). For a statement inside an event
/// block (`@ dc { … }`, DVC-19), this is the **union** of the helper
/// analyses and every enclosing event block's scope analyses — so a
/// `require` in `@ dc { … }` records `["dc"]` here regardless of what
/// its helpers belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StmtClass {
    /// The statement's class. A `require` is always `Pointwise` (a reduced
    /// predicate is an elaboration error, not a class); a `var`/`target`
    /// carries the class of its expression.
    pub class: QuantityClass,
    /// The analyses this statement applies to (`"ac"`, `"dc"`, …), sorted
    /// and deduplicated. Empty for a top-level quantity built from probes
    /// and constants alone — readable in every analysis (D2). Non-empty
    /// when an enclosing `@ dc { … }` (DVC-19) or an analysis-bound helper
    /// scopes it: the analysis drivers evaluate such a margin only when
    /// the running analysis matches; an analysis that never ran leaves
    /// the margin "not exercised" (never a silent vacuous pass).
    pub analyses: Vec<String>,
}

/// One node in a constraint block's per-statement classification tree
/// (dv-core T28, DVC-19): a leaf statement's class, or an event block's
/// recursive children. Parallel in structure to the authored
/// [`ConstraintStmt`] tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtClassNode {
    /// A leaf statement's class — parallel to a
    /// `ConstraintStmt::Require`/`Var`/`Target`.
    Leaf(StmtClass),
    /// An event block (`@ dc { … }`, DVC-19): the analyses this scope
    /// applies to and the per-child classification tree.
    Event {
        /// Analyses this event block scopes its body to (e.g. `["dc"]`,
        /// `["dc", "tran"]`), sorted and deduplicated.
        analyses: Vec<String>,
        /// Classification tree parallel to the event block's body
        /// statements.
        children: Vec<StmtClassNode>,
    },
}

impl StmtClassNode {
    /// A leaf node's class, or `None` for an event block.
    pub fn as_leaf(&self) -> Option<&StmtClass> {
        match self {
            Self::Leaf(c) => Some(c),
            Self::Event { .. } => None,
        }
    }
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
    /// Per-top-level-statement classification, parallel to [`Self::stmts`]
    /// — a recursive tree mirroring the authored statements (DVC-09/19).
    pub stmt_classes: Vec<StmtClassNode>,
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
    /// Per-top-level-statement classification, parallel to [`Self::stmts`].
    pub fn classes(&self) -> &[StmtClassNode] { &self.stmt_classes }
    /// Walk every leaf statement (Require/Var/Target), depth-first in
    /// authored order, yielding `(stmt, class)` pairs. The class's
    /// `analyses` set is the union of the leaf's helper analyses and
    /// every enclosing event block's scope analyses (DVC-19) — the
    /// analysis drivers gate evaluation on this set.
    pub fn leaves(&self) -> Leaves<'_> {
        // The iterator pops from the stack top, so push in reverse to
        // yield depth-first in authored order.
        let stack: Vec<_> = self.stmts.iter().zip(&self.stmt_classes).rev().collect();
        Leaves { stack }
    }
    /// The classification of the leaf statement named `name` (a `require`
    /// or a `var`), if one exists. Walks nested event blocks (DVC-19).
    pub fn class_of(&self, name: &str) -> Option<&StmtClass> {
        self.leaves().find(|(stmt, _)| match stmt {
            ConstraintStmt::Require { name: n, .. } | ConstraintStmt::Var { name: n, .. } => n == name,
            _ => false,
        }).map(|(_, c)| c)
    }
}

/// Depth-first iterator over a [`ConstraintBlock`]'s leaf statements
/// (yielded in authored order, descending through `@ … { … }` event
/// blocks). See [`ConstraintBlock::leaves`].
pub struct Leaves<'a> {
    stack: Vec<(&'a ConstraintStmt, &'a StmtClassNode)>,
}

impl<'a> Iterator for Leaves<'a> {
    type Item = (&'a ConstraintStmt, &'a StmtClass);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((stmt, node)) = self.stack.pop() {
            match (stmt, node) {
                (ConstraintStmt::Event { body, .. }, StmtClassNode::Event { children, .. }) => {
                    // Push children in reverse so the next pop yields the
                    // first child — depth-first, authored order.
                    for pair in body.iter().zip(children).rev() {
                        self.stack.push(pair);
                    }
                }
                (_, StmtClassNode::Leaf(class)) => return Some((stmt, class)),
                // Defensive: a structural mismatch between stmts and
                // classes should never happen (the elaborator builds both
                // in lockstep); skip the entry rather than fail silently
                // panicking — the next leaf surfaces instead.
                _ => {}
            }
        }
        None
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
