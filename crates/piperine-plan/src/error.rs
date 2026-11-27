//! The error of planning a body.

use std::fmt;

/// A body the plan cannot lay out: an `Ir` that `Ir::check` refuses, built by a host through
/// `IrBuilder` and planned without being checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanError {
    /// The name of the body that was being planned.
    pub body: String,
    /// What the body holds that has no layout.
    pub detail: String,
}

impl PlanError {
    /// The error of body `body`, which holds `detail`.
    pub(crate) fn new(body: &str, detail: String) -> Self {
        Self { body: body.to_string(), detail }
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "body {}: {}; check() refuses it", self.body, self.detail)
    }
}

impl std::error::Error for PlanError {}
