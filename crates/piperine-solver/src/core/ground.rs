//! The names of the ground reference.

/// The names that route a node to the ground reference (the netlist convention), whatever the case.
pub struct GroundName;

impl GroundName {
    /// The canonical spelling of the ground node.
    pub const TEXT: &'static str = "gnd";

    /// Whether `name` names the ground reference: `gnd`, `GND` (the label ground prints under), `vss` or `VSS`.
    pub fn names(name: &str) -> bool {
        matches!(name, "gnd" | "GND" | "vss" | "VSS")
    }
}
