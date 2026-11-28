//! The state of analog lowering (§14): [`AnalogSites`], what a run keeps across the analog blocks it lowers, and
//! [`AnalogScope`], the branch and site context of the analog code being lowered. Both go to `elab/analog/` with the
//! methods that read them.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{Branch, Condition, ExprId as LoweredExpr, NatureId, Site, StmtId as LoweredStmt, Terminal};

use super::selectors::{ArmCondition, ContributionRecord};
use super::unroll::{CallCycles, Expansion};
use crate::pom::{DeclId, ExprId};

/// What one run keeps of the analog code of every body: the IR natures, the ammeter branches, and what unrolling has
/// learnt of loops and call cycles.
#[derive(Default)]
pub(in crate::elab) struct AnalogSites {
    /// The IR nature of each reached nature declaration (`nature.rs`).
    pub(super) natures: BTreeMap<DeclId, NatureId>,
    /// The ammeter branch of each port whose `inflow()` the body being lowered reads (`inflow.rs`).
    pub(super) ammeters: BTreeMap<DeclId, u32>,
    /// The placements inside a loop, by module (`unroll.rs`).
    pub(super) looped: BTreeMap<DeclId, BTreeSet<ExprId>>,
    /// The functions on a call cycle (`unroll.rs`).
    pub(super) cycles: CallCycles,
}

/// The branch and site context of the analog code being lowered: the `limit`, `ac_stim` and `bound_step` sites, the
/// branches and contributions, the arms the code sits in, and how a value a primitive evaluates on its own is read.
#[derive(Default)]
pub(in crate::elab) struct AnalogScope {
    /// The `limit` sites met so far.
    pub(in crate::elab) limits: Vec<Site>,
    /// The `ac_stim` sites met so far.
    pub(in crate::elab) stims: Vec<Site>,
    /// The branches the contributions name, in order of first contribution.
    pub(in crate::elab) branches: Vec<Branch>,
    /// How many closure bodies and `Option` method bodies the code being lowered sits in: a
    /// `limit`, `ac_stim` or `bound_step` there is refused (the conditions of `if` and `match` arms
    /// are `guards`).
    pub(in crate::elab) nesting: u32,
    /// The source form of the conditions of the arms the code sits in, outermost first: what a
    /// guarded site lowers standalone (`selectors.rs`).
    pub(in crate::elab) guards: Vec<ArmCondition>,
    /// The `bound_step` statements met under a condition or in a nested block, moved to the top of
    /// the analog block (`analog.rs`).
    pub(in crate::elab) hoisted_steps: Vec<LoweredStmt>,
    /// Plain blocks entered below the analog block's own statement list: a contribution there
    /// is not a top-level statement (Part IV §10 rule 8 makes it legal; M1 refuses it).
    pub(in crate::elab) block_depth: u32,
    /// The name the `let` being lowered gives the `limit` call that is its whole initializer.
    pub(in crate::elab) limit_name: Option<(ExprId, String)>,
    /// Code a core primitive evaluates at a converged solution (a `noise` density, a `delay` input): a `limit` there is
    /// its argument, since limiting changes no iterate that has stopped moving (§14.7.4).
    pub(super) at_solution: bool,
    /// Code a primitive evaluates on its own, outside every statement of the `analog` block (a `noise` density, a
    /// `delay` input, a `ddt` state): an `ac_stim` read there is also a statement of the block, which is where the
    /// stimulus site is written (`analog.rs`).
    pub(super) value_input: bool,
    /// Code that is differentiated symbolically (a `ddx` value): a `fn` called there is expanded where it stands
    /// (`expansion.rs`), not called.
    pub(in crate::elab) differentiated: bool,
    /// The branches whose flow a differentiated value reads as a leaf though the module gives them no potential
    /// contribution (`analog.rs`): the derivative may not read their value.
    pub(in crate::elab) flow_leaves: Vec<u32>,
    /// The parameters and `let`s of the `fn` being expanded in a differentiated value, each the lowered expression it holds
    /// (`expansion.rs`).
    pub(in crate::elab) values: BTreeMap<DeclId, LoweredExpr>,
    /// The `Option` parameters and `let`s of the `fn` being expanded: the flag and the payload each holds.
    pub(in crate::elab) option_values: BTreeMap<DeclId, (LoweredExpr, LoweredExpr)>,
    /// The forks the `fn` being expanded has made so far (`symbolic.rs`).
    pub(super) forks: u32,
    /// Whether the code being lowered reads `freq()` from the solve (§14.4.4): the analog block
    /// and the functions it calls (a function's `freq_` is the caller's; a reaction reads 0). An
    /// `emit!` there is a trace (`records.rs`).
    pub(in crate::elab) in_analog_code: bool,
    /// The `bound_step` statements met so far: the next one's site.
    pub(super) bounds: u32,
    /// The conditions of the arms the code being lowered sits in, outermost first: the selector
    /// of a contribution (§11), read off the same structure `Body::contributions` reads.
    pub(in crate::elab) conditions: Vec<Condition>,
    /// The body `let`s that bind a bare `ddt` call and are read only as whole addends.
    pub(in crate::elab) addend_lets: BTreeSet<DeclId>,
    /// The branches the body merges (`structural.rs`): contributions to them are not lowered.
    pub(in crate::elab) merged: Vec<(Terminal, Terminal)>,
    /// Every contribution lowered so far, for `selectors.rs` to decide the branch kinds.
    pub(super) contributions: Vec<ContributionRecord>,
    /// Whether the code being lowered is a statement of an `analog` block, not a function's body. The analog side reads
    /// constant elements of an array of carriers only, each through its mirror (`elements.rs`), so an array read whole
    /// or at a run-time element is read there as the array of its elements.
    pub(in crate::elab) in_analog_block: bool,
    /// The recursive calls being expanded, outermost first (`unroll.rs`).
    pub(in crate::elab) expanding: Vec<Expansion>,
}
