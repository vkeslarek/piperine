//! The state of analog behaviour: which expressions are continuous, and how deep in a `ddx` the check is.

use crate::pom::{Design, ExprId};

use super::continuity::Continuity;

/// What checking analog behaviour (§14) keeps: which expressions are continuous, and how deep in a `ddx` the check is.
pub(in crate::types) struct AnalogTables {
    /// What the check knows of continuity, filled as it goes (`continuity.rs`).
    pub(super) continuity: Continuity,
    /// How many `ddx` values are being typed: a flow probe of a branch with no potential contribution may be read there,
    /// where it is a leaf the derivative reads symbolically (§14.2.3), though it is no value elsewhere (`probe`).
    pub(super) differentiated: u32,
}

impl AnalogTables {
    /// The empty state of a check of `design`.
    pub(in crate::types) fn new(design: &Design) -> Self {
        AnalogTables { continuity: Continuity::new(design), differentiated: 0 }
    }

    /// The expressions found continuous.
    pub(in crate::types) fn continuous_set(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.continuity.continuous_set()
    }
}
