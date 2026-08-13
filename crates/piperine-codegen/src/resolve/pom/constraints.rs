//! Constraint-margin/cover-catalog *types* (DVC-08/DVT-40). The producer
//! that used to fill these (`ConstraintLowering`, walking PHDL's
//! `pom::ConstraintBlock`) is gone: T21's MD-45 gutting deleted
//! `pom/constraint.rs` (`ConstraintBlock`/`QuantityClass`/`Module::
//! constraints()`) outright along with the rest of PHDL's dv-core
//! `require`/`var`/`target`/`cover` surface — Verilog-AMS's Annex A grammar
//! has no constraint-block production to lower from, and a future phase
//! re-anchors this on VAMS `(* attribute *)` syntax (design.md P2 story,
//! `.specs/STATE.md` MD-45) rather than this task rebuilding it
//! speculatively.
//!
//! The types stay (kernel's constraint/coverage capabilities are already
//! `Option`-gated on `LoweredBody::constraints`/`::cover`, which
//! `lower_bodies` now always sets to `None` — see `body.rs`) so that
//! machinery doesn't need touching in this retarget; only the producer that
//! depended on the deleted POM type is removed.

use piperine_vams::ast::Expr as PomExpr;

/// A quantity's analysis class — kept as a plain local copy since
/// `piperine_lang::pom::QuantityClass` was deleted with `ConstraintBlock`
/// (MD-45); nothing constructs a non-default value today (the producer is
/// gone), but the field shape is kept stable for whichever future task
/// rebuilds the producer on VAMS attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuantityClass {
    #[default]
    Pointwise,
    Reduced,
}

/// A temporal window (DVC-20) an enclosing event block scopes a margin/cover
/// point to — `[after, after + dur)` in simulation time. Local copy of the
/// type `piperine_lang::pom::ResolvedWindow` used to be (deleted with
/// `pom/constraint.rs`, MD-45); kept because `kernel`/`device`'s constraint
/// and coverage capabilities still read it through `LoweredMargin`/
/// `LoweredCoverPoint`, even though nothing constructs a non-`None` value
/// until a future phase rebuilds the producer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedWindow {
    pub after: f64,
    pub dur: f64,
}

impl ResolvedWindow {
    /// Whether simulation time `t` falls inside `[after, after + dur)`.
    pub fn contains(&self, t: f64) -> bool {
        t >= self.after && t < self.after + self.dur
    }
}

/// A `cover` point's bin partition (D10: literal by construction) — local
/// copy of the type `piperine_lang::pom::ResolvedCoverBins` used to be
/// (deleted with `pom/constraint.rs`, MD-45), kept for the same reason as
/// [`ResolvedWindow`].
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedCoverBins {
    Range { lo: f64, step: f64, hi: f64 },
    Set(Vec<f64>),
}

/// What kind of statement a lowered margin came from — a `require` names
/// its finding by the authored label; a `target` is unlabeled by grammar
/// and is named by position (`target#<stmt index>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginKind {
    Require,
    Target,
}

/// One lowered constraint margin: the signed, scale-normalized expression
/// (`expr ≥ 0` ⟺ satisfied) with its provenance — the authored label, the
/// statement kind, the pointwise/reduced class, the analyses its helpers
/// belong to (DVC-09), and the temporal window an enclosing event block
/// scopes it to (DVC-20).
#[derive(Debug, Clone)]
pub struct LoweredMargin {
    pub label: String,
    pub kind: MarginKind,
    pub class: QuantityClass,
    pub analyses: Vec<String>,
    pub window: Option<ResolvedWindow>,
    pub expr: PomExpr,
}

/// A `cover` point lowered to its evaluable (resolved) expression (D9: a
/// **separate** lowering from [`LoweredMargin`] — coverage is not a
/// margin, no signed distance, no argmin, so it gets its own catalog
/// entry rather than a `MarginKind` variant riding the margin machinery).
#[derive(Debug, Clone)]
pub struct LoweredCoverPoint {
    pub label: String,
    pub bins: ResolvedCoverBins,
    pub analyses: Vec<String>,
    pub window: Option<ResolvedWindow>,
    pub expr: PomExpr,
}

/// A `cover cross` lowered — no expression of its own (T33's bin-mapper
/// combines the *already-mapped* bin indices of its named points
/// host-side; nothing here needs JIT compilation).
#[derive(Debug, Clone)]
pub struct LoweredCoverCross {
    pub label: String,
    pub points: Vec<String>,
    pub joint_bin_count: usize,
}
