//! The control state of the body being checked: which deferred locals are assigned (§1.7.5),
//! and, as later tasks add them, the loops and the functions the next `break` or `return`
//! leaves.

use std::collections::BTreeSet;

use crate::pom::DeclId;

/// Which locals declared without a value (`let x;`) are assigned at one point of a body.
#[derive(Debug, Clone, Default)]
pub(super) struct Assigned {
    /// The locals assigned on every path to the point; `None` once no path reaches it (after a
    /// `return`, `break` or `continue`), where every local counts as assigned.
    definite: Option<BTreeSet<DeclId>>,
    /// The locals assigned on at least one path to the point.
    possible: BTreeSet<DeclId>,
}

impl Assigned {
    /// The state at the start of a body: nothing is assigned yet.
    pub(super) fn start() -> Self {
        Assigned { definite: Some(BTreeSet::new()), possible: BTreeSet::new() }
    }

    /// Whether `local` is assigned on every path to the point.
    pub(super) fn definitely(&self, local: DeclId) -> bool {
        self.definite.as_ref().is_none_or(|set| set.contains(&local))
    }

    /// Whether `local` is assigned on some path to the point.
    pub(super) fn possibly(&self, local: DeclId) -> bool {
        self.definitely(local) || self.possible.contains(&local)
    }

    /// Records the assignment of `local` at the point.
    pub(super) fn assign(&mut self, local: DeclId) {
        if let Some(set) = &mut self.definite {
            set.insert(local);
        }
        self.possible.insert(local);
    }

    /// The state of a point no path reaches: the identity of `meet`.
    fn unreachable() -> Self {
        Assigned { definite: None, possible: BTreeSet::new() }
    }

    /// The state where all of `branches` meet; a point no branch reaches when there is none.
    pub(super) fn join(branches: impl IntoIterator<Item = Assigned>) -> Assigned {
        branches.into_iter().fold(Assigned::unreachable(), |all, branch| all.meet(&branch))
    }

    /// The state where two paths meet: assigned on every path of both, or on some path of either.
    pub(super) fn meet(&self, other: &Assigned) -> Assigned {
        let definite = match (&self.definite, &other.definite) {
            (None, side) | (side, None) => side.clone(),
            (Some(a), Some(b)) => Some(a.intersection(b).copied().collect()),
        };
        Assigned { definite, possible: self.possible.union(&other.possible).copied().collect() }
    }
}

/// The control state of one body (a module's behavior, a function or a closure): what is saved
/// and restored when the checker sets a body aside to check another.
#[derive(Debug, Clone)]
pub(super) struct Flow {
    /// Which deferred locals are assigned at the expression being checked.
    pub(super) assigned: Assigned,
    /// How many loops the expression being checked is inside.
    pub(super) loop_depth: usize,
}

impl Flow {
    /// Ends the branch just checked: the state it reached, with the state of the point the next
    /// branch starts from (`before`) restored.
    pub(super) fn end_branch(&mut self, before: &Assigned) -> Assigned {
        std::mem::replace(&mut self.assigned, before.clone())
    }
}

impl Default for Flow {
    fn default() -> Self {
        Flow { assigned: Assigned::start(), loop_depth: 0 }
    }
}
