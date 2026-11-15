//! Placement: the instance tree and the node table. An instance is one placement of a module,
//! bound to the parent's nodes and to param values; the tree is walked in pre-order.

use piperine_ir::{InstanceId, InstanceSpec, NodeRef, NodeSpec};

use super::elaborator::Elaboration;
use crate::msl::Diagnostic;
use crate::pom::DeclId;

impl Elaboration<'_, '_> {
    /// The top instance: its params take their defaults, each signal is a node of the design.
    pub(super) fn place_top(&mut self, top: DeclId) -> Option<InstanceId> {
        let data = self.design.decl_data(top).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let origin = self.locator.decl(top);
        let plan = self.plans.get(&top)?;
        let body = plan.body;
        let mut params = Vec::new();
        let mut missing = Vec::new();
        for param in &plan.params {
            match param.default {
                Some(value) => params.push(value),
                None => missing.push(param.name.clone()),
            }
        }
        let signals: Vec<_> = plan.nodes.iter().map(|n| (n.name.clone(), n.nature, n.init, n.origin.clone(), n.role)).collect();
        if let Some(param) = missing.first() {
            let span = data.name.as_ref().map_or(data.span, |n| n.span);
            self.report(data.file, Diagnostic::new("E0319", format!("`{name}` leaves the required param `{param}` unbound"), span, "required param not bound"));
            return None;
        }
        let declared_by = self.ir.next_instance_id();
        let mut nodes = Vec::new();
        for (node_name, nature, init, node_origin, _role) in signals {
            let id = self.ir.node(NodeSpec { path: format!("{name}.{node_name}"), nature, declared_by, init, origin: node_origin });
            nodes.push(NodeRef::Node(id));
        }
        Some(self.ir.instance(InstanceSpec { path: name.clone(), name, body, parent: None, nodes, params, origin }))
    }
}
