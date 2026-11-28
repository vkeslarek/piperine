//! The state of generic items: the item being typed and the substitutions in force.

use crate::pom::DeclId;

use crate::types::Substitution;

/// What typing generic items (§3.8) is under: the generic item being checked and the substitutions in force.
#[derive(Default)]
pub(in crate::types) struct GenericsTables {
    /// The generic function whose definition is being checked or whose signature is being typed:
    /// what a definition defers is recorded under it.
    pub(super) generic_item: Option<DeclId>,
    /// The arguments of the generic module whose placement is being connected: the ports are typed under it (GN-13).
    pub(super) placement_substitution: Option<Substitution>,
    /// The arguments a bound writes for its trait, set for the call through it being typed (`bounds.rs`).
    pub(super) bound_substitution: Option<Substitution>,
}

impl GenericsTables {
    /// The generic item being checked.
    pub(in crate::types) fn generic_item(&self) -> Option<DeclId> {
        self.generic_item
    }

    /// Set the generic item being checked, and give the one it replaces.
    pub(in crate::types) fn replace_generic_item(&mut self, item: Option<DeclId>) -> Option<DeclId> {
        std::mem::replace(&mut self.generic_item, item)
    }

    /// Set the arguments of the generic module whose placement is being connected, and give the ones it replaces.
    pub(in crate::types) fn replace_placement_substitution(&mut self, substitution: Option<Substitution>) -> Option<Substitution> {
        std::mem::replace(&mut self.placement_substitution, substitution)
    }

    /// Take the arguments a bound writes for its trait, once.
    pub(in crate::types) fn take_bound_substitution(&mut self) -> Option<Substitution> {
        self.bound_substitution.take()
    }
}
