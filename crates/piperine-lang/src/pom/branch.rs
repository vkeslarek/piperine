//! POM [`Branch`] — a path between two nets, LRM §3.12 / §5.
//!
//! PHDL had no first-class branch type: contributions (`<+`) resolved
//! through net/terminal references directly. Verilog-AMS introduces
//! branches as an explicit, nameable abstraction (LRM §3.12) plus the
//! implicit branches access functions (`V(a,b)`, `I(br)`) create on first
//! use (§5.4.1). This module is the POM-side type those constructs lower
//! into — the codegen contribution-splitting pass (resistive / charge /
//! `ac_stim`) keys off it (design.md §5.2).
//!
//! A branch contributed as both potential and flow is a fail-loud error
//! (LRM §5.4.2), with **switch branches** (§5.6.5) as the one legal
//! exception. The exception is modelled as a distinct [`BranchKind`]
//! variant rather than by relaxing the conflict check — a switch branch
//! is *allowed* to toggle kinds at runtime, a regular branch is not.

use crate::pom::net_type::NetRef;
use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named};

/// How a [`Branch`] is driven (or merely probed), LRM §5.4.2 / §5.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BranchKind {
    /// A probe branch — read by an access function but not driven by any
    /// contribution statement. LRM §5.4.2: a branch with no contribution
    /// is neither a potential source nor a flow source. The starting
    /// state of every branch before the first contribution is seen.
    Probe,
    /// A potential source — driven by `V(p,n) <+ expr` or `V(br) <+ expr`.
    /// LRM §5.4.2 / §5.6.
    Potential,
    /// A flow source — driven by `I(p,n) <+ expr` or `I(br) <+ expr`.
    /// LRM §5.4.2 / §5.6.
    Flow,
    /// A switch branch — toggles between potential and flow source at
    /// runtime through conditional contribution statements. LRM §5.6.5:
    /// the **one** legal exception to the "a branch cannot simultaneously
    /// be both a potential and a flow source" rule. Modelled explicitly
    /// here, never by relaxing the [`BranchKindConflict`][super::error::ElabErrorKind::BranchKindConflict]
    /// check on the other two kinds.
    Switch,
}

impl BranchKind {
    /// Whether this kind is a source branch (drives the branch) rather
    /// than a probe (only reads it). LRM §5.4.2.
    pub fn is_source(self) -> bool {
        matches!(self, Self::Potential | Self::Flow | Self::Switch)
    }
    /// Whether this kind is a potential source specifically.
    pub fn is_potential(self) -> bool {
        matches!(self, Self::Potential)
    }
    /// Whether this kind is a flow source specifically.
    pub fn is_flow(self) -> bool {
        matches!(self, Self::Flow)
    }
    /// Whether this branch may legitimately contribute both potential
    /// and flow (the §5.6.5 switch-branch exception).
    pub fn allows_both_potential_and_flow(self) -> bool {
        matches!(self, Self::Switch)
    }
}

/// The serde default for [`BranchKind`] — a fresh branch is a probe until
/// the first contribution is seen (LRM §5.4.2).
impl Default for BranchKind {
    fn default() -> Self {
        Self::Probe
    }
}

/// A Verilog-AMS branch — a named or implicit path between two nets that
/// access functions (`V(...)`/`I(...)`) resolve through and contribution
/// statements (`<+`) drive. LRM §3.12, §5.4, §5.6.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Branch {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    /// The declared name (`branch (a,b) br;` → `Some("br")`), or `None`
    /// for an implicit branch created by an access function on a net
    /// pair. LRM §3.12: "There shall be at most one unnamed branch
    /// between any two nets".
    #[serde(default)]
    pub name: Option<String>,
    /// The positive terminal. Always present — every branch has at
    /// least one net.
    pub pos: NetRef,
    /// The negative terminal. `None` ⇒ ground-referenced (single-net
    /// branch). LRM §3.12: "Only one net need be specified, in which
    /// case the second net defaults to ground".
    #[serde(default)]
    pub neg: Option<NetRef>,
    /// How the branch is driven. Set by the first contribution seen
    /// during elaboration; the default before any contribution is
    /// [`BranchKind::Probe`].
    #[serde(default)]
    pub kind: BranchKind,
    /// `true` for a §3.12.1 port branch (`branch (<p>) name;`) — `pos`
    /// names the port, `neg` is filled in with the synthetic internal net
    /// `elab/lower/vams.rs::apply_port_branches` creates to split the
    /// port's upper/lower connections. Kept as a plain field rather than a
    /// `BranchKind::Port` variant (the design's first sketch): the branch
    /// is driven by an ordinary synthetic `V(p, inner) <+ 0;` contribution
    /// during elaboration, which must transition it through the normal
    /// [`BranchKind::Potential`] path — a dedicated `Port` kind would
    /// immediately conflict with that transition in
    /// [`Branch::record_contribution`].
    #[serde(default)]
    pub is_port: bool,
}

impl Branch {
    /// The branch's declared name, if named. `None` for implicit branches.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// The positive terminal.
    pub fn pos(&self) -> &NetRef {
        &self.pos
    }
    /// The negative terminal, if not ground-referenced.
    pub fn neg(&self) -> Option<&NetRef> {
        self.neg.as_ref()
    }
    /// Whether this branch is ground-referenced (single-net).
    pub fn is_ground_referenced(&self) -> bool {
        self.neg.is_none()
    }
    /// Whether this branch is implicit (created by an access function
    /// rather than declared by `branch (a,b) name;`).
    pub fn is_implicit(&self) -> bool {
        self.name.is_none()
    }
    /// How the branch is driven.
    pub fn kind(&self) -> BranchKind {
        self.kind
    }
    /// Record a contribution to this branch, returning an error if the
    /// new contribution's kind conflicts with an already-recorded one
    /// and the branch is not a switch branch. LRM §5.4.2 / §5.6.5.
    ///
    /// This is the structural pre-check; the elaborator's first-contribution
    /// wiring (T23) calls into here. T18 ships the check itself plus the
    /// POM type; T23 hooks it into the contribution lowering path.
    pub fn record_contribution(
        &mut self,
        new_kind: BranchKind,
        second_span: Option<miette::SourceSpan>,
        module: &str,
    ) -> Result<(), crate::pom::error::ElabError> {
        let conflict = match (self.kind, new_kind) {
            // Switch branches may receive both — no conflict possible.
            (BranchKind::Switch, _) | (_, BranchKind::Switch) => false,
            // A probe records whatever kind arrives.
            (BranchKind::Probe, _) => false,
            // Same-kind contributions are normal (multiple V() writes).
            (a, b) if a == b => false,
            // Potential ↔ Flow on a non-switch branch — the §5.4.2 violation.
            _ => true,
        };

        if conflict {
            return Err(crate::pom::error::ElabError::new(
                crate::pom::error::ElabErrorKind::BranchKindConflict {
                    module: module.to_string(),
                    branch: self.name.clone().unwrap_or_else(|| unnamed_label(&self.pos, self.neg.as_ref())),
                    first: self.span,
                    second: second_span,
                },
            ));
        }

        // First-time contribution on a probe promotes the branch to the
        // recorded kind. A later contribution of the other kind on a
        // non-switch branch is the conflict case handled above; on a
        // switch branch it stays Switch (set by the elaborator when it
        // detects the §5.6.5 conditional-contribution pattern, T23).
        if self.kind == BranchKind::Probe {
            self.kind = new_kind;
        }
        Ok(())
    }
    /// Mark this branch as a §5.6.5 switch branch (the elaborator calls
    /// this when it sees the conditional-contribution pattern).
    pub fn mark_switch(&mut self) {
        self.kind = BranchKind::Switch;
    }
    /// Attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> {
        self.doc.as_deref()
    }
}

/// Synthesize a readable label for an unnamed branch (`(a,b)` or `(a,)`).
fn unnamed_label(pos: &NetRef, neg: Option<&NetRef>) -> String {
    match neg {
        None => format!("({})", pos),
        Some(n) => format!("({},{})", pos, n),
    }
}

impl Named for Branch {
    fn name(&self) -> &str {
        // Implicit branches have no authored name — return the empty
        // string, matching `Behavior`'s convention for unnamed nodes.
        self.name.as_deref().unwrap_or("")
    }
}

impl Kinded for Branch {
    fn kind(&self) -> Kind {
        Kind::Branch
    }
}
