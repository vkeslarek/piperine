//! `Net` — the unified public identity of a solved signal.
//!
//! Analog nodes, analog branch currents, and digital nets are named different
//! ways inside the solver ([`AnalogReference`] over an [`AnalogVariable`], and
//! the digital engine's net ids). Those stay as the fast-path types the hot
//! loops use. `Net` is the one *public* identity over all of them: a
//! dense index for the fast path paired with a [`NetKind`] and a stable label,
//! so diagnostics, queries, and result mapping treat `v(out)`, `i(vsrc)`, a
//! digital net, and `gnd` symmetrically.

use std::fmt;
use std::sync::Arc;

use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, Netlist};

use crate::core::ground::GroundName;

/// What kind of solved signal a [`Net`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetKind {
    /// An analog node voltage.
    Node,
    /// An analog branch current.
    Branch,
    /// A digital logic net.
    Digital,
    /// A reference signal with no solved unknown of its own (e.g. ground).
    Pseudo,
}

/// The unified identity of a solved signal. Pairs the fast dense index with a
/// [`NetKind`] and a stable label. For analog nets, also retains the
/// originating [`AnalogVariable`] so result types can look up the solved
/// value by `Net` without an extra map. `dense == usize::MAX` means the signal
/// has no solved unknown (a pseudo net such as ground, or an as-yet-unmapped
/// variable).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Net {
    dense: usize,
    kind: NetKind,
    label: String,
    /// Set when the net originated from an [`AnalogVariable`] (the typical
    /// case for solver results); `None` for digital and pseudo nets where the
    /// dense index already identifies the signal.
    analog: Option<Arc<AnalogVariable>>,
}

impl Net {
    const NONE: usize = usize::MAX;

    /// An analog node voltage at dense index `dense`.
    pub fn node(dense: usize, label: impl Into<String>) -> Self {
        Self {
            dense,
            kind: NetKind::Node,
            label: label.into(),
            analog: None,
        }
    }

    /// An analog branch current at dense index `dense`.
    pub fn branch(dense: usize, label: impl Into<String>) -> Self {
        Self {
            dense,
            kind: NetKind::Branch,
            label: label.into(),
            analog: None,
        }
    }

    /// A digital net at dense index `dense`.
    pub fn digital(dense: usize, label: impl Into<String>) -> Self {
        Self {
            dense,
            kind: NetKind::Digital,
            label: label.into(),
            analog: None,
        }
    }

    /// The ground pseudo net: no solved unknown, label `gnd` (its one spelling, [`GroundName`]).
    pub fn ground() -> Self {
        Self {
            dense: Self::NONE,
            kind: NetKind::Pseudo,
            label: GroundName::TEXT.into(),
            analog: None,
        }
    }

    /// The dense solve index, or `None` for a pseudo/unmapped net.
    pub fn dense(&self) -> Option<usize> {
        (self.dense != Self::NONE).then_some(self.dense)
    }

    pub fn kind(&self) -> NetKind {
        self.kind
    }

    /// Replaces the label (a node the builder was given a name for).
    pub(crate) fn relabel(&mut self, label: String) {
        self.label = label;
    }

    /// The stable source-level label, for diagnostics and result mapping.
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_ground(&self) -> bool {
        self.kind == NetKind::Pseudo && self.dense == Self::NONE
    }

    /// The originating [`AnalogVariable`] if this is an analog net. `None` for
    /// digital and pseudo nets.
    pub fn analog_variable(&self) -> Option<&Arc<AnalogVariable>> {
        self.analog.as_ref()
    }
}

impl fmt::Display for Net {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The `v(node)` / `i(branch)` label an analog variable carries as a [`Net`].
fn analog_label(variable: &AnalogVariable) -> String {
    match variable {
        AnalogVariable::Node(node) => format!("v({node})"),
        AnalogVariable::Branch(branch) => match &branch.name {
            Some(name) => format!("i({}.{})", branch.component, name),
            None => format!("i({})", branch.component),
        },
        AnalogVariable::Time => "time".into(),
        AnalogVariable::Frequency => "freq".into(),
        AnalogVariable::Iteration => "iter".into(),
    }
}

impl From<&AnalogReference> for Net {
    fn from(reference: &AnalogReference) -> Self {
        let label = analog_label(reference.variable());
        let dense = reference.idx().unwrap_or(Net::NONE);
        let variable: Arc<AnalogVariable> = reference.variable().clone();
        let kind = match reference.variable().as_ref() {
            AnalogVariable::Node(node) if node.is_ground() => return Net::ground(),
            AnalogVariable::Branch(_) => NetKind::Branch,
            _ => NetKind::Node,
        };
        Net {
            dense,
            kind,
            label,
            analog: Some(variable),
        }
    }
}

impl Net {
    /// Every analog variable of a netlist as a unified [`Net`] — the public naming
    /// layer for result mapping, queries, and diagnostics. A digital net is named
    /// with [`Net::digital`].
    pub fn analog_nets(netlist: &Netlist) -> Vec<Net> {
        netlist.all_references().into_iter().map(Into::into).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, NodeIdentifier};
    use std::sync::Arc;

    #[test]
    fn analog_node_and_branch_map_to_named_nets() {
        let node = AnalogReference::new(
            Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(12))),
            3,
        );
        let net: Net = (&node).into();
        assert_eq!(net.kind(), NetKind::Node);
        assert_eq!(net.dense(), Some(3));
        assert_eq!(net.label(), "v(n12)");
        assert!(!net.is_ground());

        let branch = AnalogReference::new(
            Arc::new(AnalogVariable::Branch(BranchIdentifier::from_component("vsrc"))),
            5,
        );
        let net: Net = (&branch).into();
        assert_eq!(net.kind(), NetKind::Branch);
        assert_eq!(net.dense(), Some(5));
        assert_eq!(net.label(), "i(vsrc)");
    }

    #[test]
    fn ground_is_a_pseudo_net_with_no_index() {
        let net: Net = (&AnalogReference::ground()).into();
        assert_eq!(net.kind(), NetKind::Pseudo);
        assert_eq!(net.dense(), None);
        assert!(net.is_ground());
        assert_eq!(net.label(), "gnd");
    }

    #[test]
    fn digital_net_maps_symmetrically() {
        let net = Net::digital(7, "d7");
        assert_eq!(net.kind(), NetKind::Digital);
        assert_eq!(net.dense(), Some(7));
        assert_eq!(net.label(), "d7");
    }

    #[test]
    fn analog_net_carries_its_variable_for_lookup() {
        let node = AnalogReference::new(
            Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(0))),
            0,
        );
        let net: Net = (&node).into();
        let var = net.analog_variable().expect("analog net must carry its variable");
        assert_eq!(var.as_ref(), &AnalogVariable::Node(NodeIdentifier::Anonymous(0)));
    }

    #[test]
    fn digital_and_pseudo_nets_have_no_analog_variable() {
        let digital = Net::digital(3, "d3");
        assert!(digital.analog_variable().is_none());

        let ground = Net::ground();
        assert!(ground.analog_variable().is_none());
    }
}
