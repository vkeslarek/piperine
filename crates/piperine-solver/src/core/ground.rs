//! The name of the ground reference.

/// The one name of the ground reference: `gnd` (S21). The spellings other netlists give ground are refused, never
/// taken for a node of their own.
pub struct GroundName;

impl GroundName {
    /// The spelling of the ground node.
    pub const TEXT: &'static str = "gnd";

    /// The spellings other netlists give ground, which a node name may not take.
    const REFUSED: [&'static str; 3] = ["GND", "vss", "VSS"];

    /// Whether `name` names the ground reference: `gnd` only.
    pub fn names(name: &str) -> bool {
        name == Self::TEXT
    }

    /// The refusal of `name` when it is another netlist's spelling of ground (`GND`, `vss`, `VSS`): it names the one
    /// spelling, `gnd`.
    pub fn refusal(name: &str) -> Option<String> {
        Self::REFUSED.contains(&name).then(|| format!("`{name}` does not name the ground: the ground is spelled `{}`", Self::TEXT))
    }
}
