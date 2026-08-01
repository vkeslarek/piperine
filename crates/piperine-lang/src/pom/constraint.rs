//! POM constraint nodes — the `constraint` block (dv-core DVC-04/05) and
//! the pointwise/reduced quantity classification (DVC-09).

use std::collections::HashMap;

pub use crate::parse::ast::{ConstraintStmt, TargetCmp};
use crate::pom::net_type::ValueType;

/// A `cover` point's bin partition, resolved to literal `f64` edges at
/// elaboration (dv-tester T32/T33, DVT-42): the same shape as
/// [`crate::parse::ast::CoverBins`] but folded, so a consumer (the
/// elaboration-time cap check, T33's bin-mapper) never re-evaluates the
/// authored expressions.
///
/// `Eq` is bit-for-bit on the `f64` fields (same rationale as
/// [`ResolvedWindow`]'s manual impl below — keeps `StmtClassNode`'s
/// `#[derive(Eq)]` working without approximate float comparison).
#[derive(Debug, Clone)]
pub enum ResolvedCoverBins {
    /// `bins [lo:step:hi]`.
    Range { lo: f64, step: f64, hi: f64 },
    /// `in {e1, e2, …}`.
    Set(Vec<f64>),
}

impl PartialEq for ResolvedCoverBins {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Range { lo: l0, step: s0, hi: h0 }, Self::Range { lo: l1, step: s1, hi: h1 }) => {
                l0.to_bits() == l1.to_bits() && s0.to_bits() == s1.to_bits() && h0.to_bits() == h1.to_bits()
            }
            (Self::Set(a), Self::Set(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
            }
            _ => false,
        }
    }
}

impl Eq for ResolvedCoverBins {}

/// A resolved temporal window on a constraint scope (dv-core T29,
/// DVC-20): the result of evaluating an [`crate::parse::ast::EventWindow`]'s
/// `after` / `dur` expressions as elaboration constants. `after` opens
/// the window at `t = after`; `dur` (when present) closes it at
/// `t = after + dur`. A margin whose evaluation coordinate falls outside
/// the window is not evaluated — never a silent pass.
///
/// `Eq` is bit-for-bit on the `f64` bounds so the surrounding
/// `PartialEq`/`Eq` derives on [`StmtClass`] keep working (no approximate
/// float comparison).
#[derive(Debug, Clone, Copy)]
pub struct ResolvedWindow {
    /// Lower bound (`after = …`). The window opens at `t = after`.
    pub after: f64,
    /// Window duration (`dur = …`). `None` is open-ended — every point
    /// with `t >= after` is inside.
    pub dur: Option<f64>,
}

impl ResolvedWindow {
    /// `true` iff `time` is inside `[after, after + dur]` (or `[after, ∞)`
    /// when `dur` is `None`).
    pub fn contains(&self, time: f64) -> bool {
        if time < self.after {
            return false;
        }
        match self.dur {
            Some(d) => time <= self.after + d,
            None => true,
        }
    }
}

impl PartialEq for ResolvedWindow {
    fn eq(&self, other: &Self) -> bool {
        self.after.to_bits() == other.after.to_bits()
            && match (self.dur, other.dur) {
                (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
                (None, None) => true,
                _ => false,
            }
    }
}

impl Eq for ResolvedWindow {}

/// What elaboration proved about one **leaf** constraint statement's
/// quantity (dv-core T10, DVC-09): its pointwise/reduced class, the
/// analyses its helpers belong to — recorded so a later pass can refuse a
/// cross-analysis read (DVC-08 AC6) — and the temporal window an
/// enclosing `@ tran(after = …, dur = …) { … }` block scopes it to
/// (DVC-20). For a statement inside an event block (`@ dc { … }`,
/// DVC-19), `analyses` is the **union** of the helper analyses and every
/// enclosing event block's scope analyses — so a `require` in
/// `@ dc { … }` records `["dc"]` here regardless of what its helpers
/// belong to. The runtime evaluator gates each evaluation point on both
/// `analyses` (the running analysis must match) and `window` (the
/// evaluation coordinate must fall inside); a margin that never matches
/// stays at its never-evaluated sentinel — "not exercised".
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
    /// The temporal window an enclosing event block scopes this leaf to
    /// (DVC-20). `None` is "every evaluation point of an in-scope
    /// analysis"; `Some(w)` filters points to those inside `w`. Propagated
    /// from the innermost enclosing event block that carries a window.
    pub window: Option<ResolvedWindow>,
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
    /// applies to, the temporal window it opens (DVC-20), and the
    /// per-child classification tree.
    Event {
        /// Analyses this event block scopes its body to (e.g. `["dc"]`,
        /// `["dc", "tran"]`), sorted and deduplicated.
        analyses: Vec<String>,
        /// Temporal window opened by the scope (DVC-20). `None` is the
        /// common case (`@ dc { … }` has no window); `Some` opens a
        /// window that filters evaluation points in addition to the
        /// analyses filter.
        window: Option<ResolvedWindow>,
        /// Classification tree parallel to the event block's body
        /// statements.
        children: Vec<StmtClassNode>,
    },
    /// A `cover` point (dv-tester T32, DVT-42): the elaborator's proof
    /// that its expression is `Real`/pointwise, plus the bin partition
    /// folded from its literal edges — what T33's bin-mapper allocates
    /// and maps against, and what T32's own `cover cross` cap check reads
    /// the count of.
    Cover {
        class: StmtClass,
        /// The number of bins this point's partition declares.
        bin_count: usize,
        /// The partition's literal edges, folded to `f64` at elaboration
        /// (D10's premise: bin edges are literal, so this is knowable
        /// statically).
        bins: ResolvedCoverBins,
    },
    /// A `cover cross` (dv-tester T32, DVT-42): the joint (product) bin
    /// count of the named points, already checked against the
    /// elaboration-time cap (a `CoverCross` node only exists once that
    /// check has passed — the cap refusal is a hard elaboration error,
    /// never a node carrying an over-cap count).
    CoverCross {
        class: StmtClass,
        /// The named points this cross joins, in authored order.
        points: Vec<String>,
        /// The product of each named point's own bin count.
        joint_bin_count: usize,
    },
}

impl StmtClassNode {
    /// A leaf node's class, or `None` for an event block.
    pub fn as_leaf(&self) -> Option<&StmtClass> {
        match self {
            Self::Leaf(c) => Some(c),
            Self::Event { .. } | Self::Cover { .. } | Self::CoverCross { .. } => None,
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
    /// Walk every `cover`/`cover cross` statement, depth-first in authored
    /// order (descending into event blocks) — the T33 bin-mapper's own
    /// catalog source, parallel to [`Self::leaves`] but for the two node
    /// kinds `leaves()` does not yield (`Cover`/`CoverCross` classify to
    /// their own [`StmtClassNode`] variants, not `Leaf`).
    pub fn cover_points(&self) -> Vec<(&ConstraintStmt, &StmtClassNode)> {
        fn walk<'a>(
            stmts: &'a [ConstraintStmt],
            classes: &'a [StmtClassNode],
            out: &mut Vec<(&'a ConstraintStmt, &'a StmtClassNode)>,
        ) {
            for (stmt, node) in stmts.iter().zip(classes) {
                match (stmt, node) {
                    (ConstraintStmt::Event { body, .. }, StmtClassNode::Event { children, .. }) => {
                        walk(body, children, out);
                    }
                    (ConstraintStmt::Cover { .. }, StmtClassNode::Cover { .. })
                    | (ConstraintStmt::CoverCross { .. }, StmtClassNode::CoverCross { .. }) => {
                        out.push((stmt, node));
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.stmts, &self.stmt_classes, &mut out);
        out
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
