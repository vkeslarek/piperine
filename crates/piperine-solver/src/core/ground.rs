//! The name of the ground reference.

/// The one name that routes a node to the ground reference: the spec knows `gnd` only.
pub struct GroundName;

impl GroundName {
    /// The spelling of the ground node.
    pub const TEXT: &'static str = "gnd";

    /// Whether `name` names the ground reference.
    pub fn names(name: &str) -> bool {
        name == Self::TEXT
    }
}
