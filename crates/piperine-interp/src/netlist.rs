//! `Interpreter::circuit`: the IR's instance table as `CircuitBuilder` calls.

use piperine_ir::{Constant, NodeRef};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, ParamValue, Port, Result};

use crate::Interpreter;

impl Interpreter {
    /// The design as a solver circuit: one solver node per design node, in the IR's node order
    /// (so node `i` is `NodeIdentifier::Anonymous(i)`), named by its path to the builder; one
    /// `CircuitBuilder::device` per instance whose body has a module, under the instance's path.
    /// A body node is bound by position to the instance's design node (`NodeRef::Ground` to the
    /// reference). Each param is passed by name, except one that holds its body's default: it
    /// is given no value and its pin keeps the default.
    pub fn circuit(&self) -> Result<CircuitInstance> {
        let ir = self.ir();
        let mut builder = CircuitBuilder::new(ir.top().path());
        let nodes: Vec<_> = ir.nodes().map(|n| builder.node(n.path())).collect();
        let ground = builder.ground();
        for instance in ir.instances() {
            let body = instance.body();
            let Some(module) = self.module(body.id()) else { continue };
            let ports: Vec<Port> = instance
                .bindings()
                .iter()
                .map(|r| match r {
                    NodeRef::Node(id) => Port::Node(nodes[id.index()].clone()),
                    NodeRef::Ground => Port::Node(ground.clone()),
                })
                .collect();
            let values: Vec<(&str, ParamValue)> = body
                .params()
                .iter()
                .zip(instance.params())
                .filter(|(param, given)| param.default.as_ref() != Some(*given))
                .map(|(param, given)| {
                    let value = match given {
                        Constant::Real(r) => ParamValue::Real(r.get()),
                        Constant::Bool(b) => ParamValue::Bool(*b),
                    };
                    (param.name.as_str(), value)
                })
                .collect();
            builder.device(module, instance.path(), &values, &ports);
        }
        builder.build()
    }
}
