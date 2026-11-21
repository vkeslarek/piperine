//! The potential drivers of a signal-flow node (part II §6 rule 2 and its Resolution): a node
//! whose natures all lack `flow` takes potential contributions to `(n, gnd)` from exactly one
//! instance, or it is `E0242`.
//!
//! The check runs on the finished IR, after placement and node merging, because only then is it
//! known which nature meets at which node: a node any of whose natures has `flow` is a
//! conservative node (§4 rule 13 Resolution) and is not counted.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{Ir, NodeId, NodeRef, QuantityRole, Terminal};

/// One signal-flow node that does not have exactly one potential driver.
pub(super) struct SignalFlowFault {
    /// The path of the node.
    pub node: String,
    /// The paths of the instances that contribute a potential to `(node, gnd)`, in path order:
    /// none, or two or more.
    pub drivers: Vec<String>,
}

/// The §6 rule 2 count over one elaborated design.
pub(super) struct SignalFlowCheck<'ir> {
    /// The design under check.
    ir: &'ir Ir,
}

impl<'ir> SignalFlowCheck<'ir> {
    /// Every signal-flow node of `ir` with zero or several potential drivers, in node order.
    pub(super) fn run(ir: &'ir Ir) -> Vec<SignalFlowFault> {
        let check = SignalFlowCheck { ir };
        let (signal_flow, drivers) = (check.signal_flow_nodes(), check.potential_drivers());
        signal_flow
            .into_iter()
            .filter_map(|node| {
                let drivers: Vec<String> = drivers.get(&node).map(|paths| paths.iter().cloned().collect()).unwrap_or_default();
                (drivers.len() != 1).then(|| SignalFlowFault { node: check.ir.node(node).path().to_string(), drivers })
            })
            .collect()
    }

    /// The nodes every nature that meets there lacks `flow` for, in node order: a node bound to
    /// no instance is not met by any nature and is not one of them.
    fn signal_flow_nodes(&self) -> Vec<NodeId> {
        let mut has_flow: BTreeMap<NodeId, bool> = BTreeMap::new();
        for instance in self.ir.instances() {
            let body = instance.body();
            for (slot, node) in body.nodes().iter().enumerate() {
                if let Some(NodeRef::Node(bound)) = instance.binding(slot) {
                    *has_flow.entry(bound).or_insert(false) |= self.ir.nature(node.nature).flow().is_some();
                }
            }
        }
        has_flow.into_iter().filter(|(_, flow)| !flow).map(|(node, _)| node).collect()
    }

    /// For each node, the paths of the instances holding a potential contribution to `(n, gnd)`,
    /// under a condition or not: a contribution under a runtime `if` is a driver when any branch
    /// drives (design D11).
    fn potential_drivers(&self) -> BTreeMap<NodeId, BTreeSet<String>> {
        let mut drivers: BTreeMap<NodeId, BTreeSet<String>> = BTreeMap::new();
        for instance in self.ir.instances() {
            let body = instance.body();
            for contribution in body.contributions().iter().filter(|c| c.quantity == QuantityRole::Potential) {
                let Some(branch) = body.branches().get(contribution.branch as usize) else { continue };
                let slot = match (branch.plus, branch.minus) {
                    (Terminal::Node(slot), Terminal::Ground) | (Terminal::Ground, Terminal::Node(slot)) => slot,
                    _ => continue,
                };
                if let Some(NodeRef::Node(node)) = instance.binding(slot as usize) {
                    drivers.entry(node).or_default().insert(instance.path().to_string());
                }
            }
        }
        drivers
    }
}
