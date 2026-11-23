//! The control state of the body being checked: which deferred locals are assigned (§1.7.5),
//! and, as later tasks add them, the loops and the functions the next `break` or `return`
//! leaves.

use std::collections::BTreeSet;

use crate::pom::DeclId;

use super::Ty;

/// Which locals declared without a value (`let x;`) are assigned at one point of a body.
#[derive(Debug, Clone, Default)]
pub(super) struct Assigned {
    /// The locals assigned on every path to the point; `None` once no path reaches it (after a
    /// `return`, `break` or `continue`), where every local counts as assigned.
    definite: Option<BTreeSet<DeclId>>,
}

impl Assigned {
    /// The state at the start of a body: nothing is assigned yet.
    pub(super) fn start() -> Self {
        Assigned { definite: Some(BTreeSet::new()) }
    }

    /// Whether `local` is assigned on every path to the point.
    pub(super) fn definitely(&self, local: DeclId) -> bool {
        self.definite.as_ref().is_none_or(|set| set.contains(&local))
    }

    /// Records the assignment of `local` at the point.
    pub(super) fn assign(&mut self, local: DeclId) {
        if let Some(set) = &mut self.definite {
            set.insert(local);
        }
    }

    /// Marks the point as one no path reaches (after `return`, `break` and `continue`).
    pub(super) fn diverge(&mut self) {
        self.definite = None;
    }

    /// The state of a point no path reaches: the identity of `meet`.
    fn unreachable() -> Self {
        Assigned { definite: None }
    }

    /// The state where all of `branches` meet; a point no branch reaches when there is none.
    pub(super) fn join(branches: impl IntoIterator<Item = Assigned>) -> Assigned {
        branches.into_iter().fold(Assigned::unreachable(), |all, branch| all.meet(&branch))
    }

    /// The state where two paths meet: assigned on every path of both.
    pub(super) fn meet(&self, other: &Assigned) -> Assigned {
        let definite = match (&self.definite, &other.definite) {
            (None, side) | (side, None) => side.clone(),
            (Some(a), Some(b)) => Some(a.intersection(b).copied().collect()),
        };
        Assigned { definite }
    }
}

/// The control state of one body (a module's behavior, a function or a closure): what is saved
/// and restored when the checker sets a body aside to check another.
#[derive(Debug, Clone)]
pub(super) struct Flow {
    /// Which deferred locals are assigned at the expression being checked.
    pub(super) assigned: Assigned,
    /// The loops the expression being checked is inside, innermost last (§1.8.10).
    pub(super) loops: Vec<LoopFrame>,
    /// The function or closure whose result the next `return` gives (§1.8.11); `None` outside
    /// one.
    pub(super) exit: Option<ExitFrame>,
}

/// The function or closure being checked, as a `return` and a `?` see it.
#[derive(Debug, Clone)]
pub(super) struct ExitFrame {
    /// The result type it declares; `None` when its signature had a diagnostic, or when a closure
    /// has none written and none expected (`inferred`).
    pub(super) result: Option<Ty>,
    /// Whether the result is still to be found from the body: a closure with no written type.
    pub(super) inferred: bool,
    /// The type of the first `return value` met while the result is inferred.
    pub(super) returned: Option<Ty>,
}

impl ExitFrame {
    /// A function or closure with the declared `result`; with none, `inferred` says whether the
    /// body is to give it.
    pub(super) fn new(result: Option<Ty>, inferred: bool) -> Self {
        ExitFrame { result, inferred, returned: None }
    }
}

/// One loop being checked: what a `break` or a `continue` in it may do.
#[derive(Debug, Clone)]
pub(super) struct LoopFrame {
    /// Whether it is a `loop`, the one loop whose `break` carries a value (§1.8.10).
    pub(super) valued: bool,
    /// The type the loop's `break` values have: given by the position, else the first `break`'s.
    pub(super) value: Option<Ty>,
    /// Whether a `break` was met.
    pub(super) broke: bool,
    /// The assignment state at each `break`.
    pub(super) exits: Vec<Assigned>,
}

impl LoopFrame {
    /// A loop whose `break` values are expected to have the type `value`, when it is known.
    pub(super) fn new(valued: bool, value: Option<Ty>) -> Self {
        LoopFrame { valued, value, broke: false, exits: Vec::new() }
    }
}

impl Flow {
    /// The state a closure body starts from: what is assigned so far, no loop (`break` and
    /// `continue` do not cross it, §1.8.12) and its own result for `return`. Gives the state of
    /// the enclosing body to hand back to `leave_body`.
    pub(super) fn enter_body(&mut self, exit: ExitFrame) -> Flow {
        let inner = Flow { assigned: self.assigned.clone(), loops: Vec::new(), exit: Some(exit) };
        std::mem::replace(self, inner)
    }

    /// Back to the enclosing body: what the nested one assigned is dropped. Gives the nested
    /// body's exit frame, with what its `return`s found.
    pub(super) fn leave_body(&mut self, outer: Flow) -> Option<ExitFrame> {
        std::mem::replace(self, outer).exit
    }

    /// Ends the branch just checked: the state it reached, with the state of the point the next
    /// branch starts from (`before`) restored.
    pub(super) fn end_branch(&mut self, before: &Assigned) -> Assigned {
        std::mem::replace(&mut self.assigned, before.clone())
    }
}

impl Default for Flow {
    fn default() -> Self {
        Flow { assigned: Assigned::start(), loops: Vec::new(), exit: None }
    }
}
