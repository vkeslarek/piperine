//! Placement: the instance tree and the node table. An instance is one placement of a module,
//! bound to the parent's nodes and to param values; the tree is walked in pre-order, children
//! in source order. Every signal of every instance is one node of the design, named by path;
//! a port is not a node, it is bound to the parent's node or to ground (§21 rules 1 and 3).

use piperine_ir::{Constant, InstanceId, InstanceSpec, NodeRef, NodeSpec, Origin, Terminal};

use super::body::ModulePlan;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, ExprId, FileId};
use piperine_ir::NodeRole;

/// One placement about to become an instance.
struct Placing {
    module: DeclId,
    /// `Top.r1`: the path of the instance.
    path: String,
    name: String,
    parent: Option<InstanceId>,
    /// What each port of the module is bound to, in port order.
    ports: Vec<NodeRef>,
    /// The values the placement writes: (index in the module's params, value, the expression).
    given: Vec<(u32, Constant, ExprId)>,
    /// Where diagnostics of the placement sit.
    file: FileId,
    span: Span,
    origin: Origin,
}

impl Elaboration<'_, '_> {
    /// The top instance and, below it, the whole tree. `None` after a diagnostic.
    pub(super) fn place_top(&mut self, top: DeclId) -> Option<InstanceId> {
        let data = self.design.decl_data(top).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let span = data.name.as_ref().map_or(data.span, |n| n.span);
        let placing = Placing { module: top, path: name.clone(), name, parent: None, ports: Vec::new(), given: Vec::new(), file: data.file, span, origin: self.locator.decl(top) };
        self.place(placing, &mut Vec::new())
    }

    /// Place `placing`, then its children. `stack` holds the modules being placed above it.
    fn place(&mut self, placing: Placing, stack: &mut Vec<DeclId>) -> Option<InstanceId> {
        if stack.contains(&placing.module) {
            let module = self.design.decl_data(placing.module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let message = format!("module `{module}` places itself (through `{}`)", placing.path);
            self.report(placing.file, Diagnostic::new("E0341", message, placing.span, "recursive placement"));
            return None;
        }
        let plan = self.plans.get(&placing.module)?.clone();
        let params = self.bind_params(&placing, &plan)?;
        let declared_by = self.ir.next_instance_id();
        let mut ports = placing.ports.iter().copied();
        let mut nodes = Vec::new();
        for node in &plan.nodes {
            match node.role {
                NodeRole::Port => nodes.push(ports.next().unwrap_or(NodeRef::Ground)),
                NodeRole::Signal => {
                    let id = self.ir.node(NodeSpec { path: format!("{}.{}", placing.path, node.name), nature: node.nature, declared_by, init: node.init, origin: node.origin.clone() });
                    nodes.push(NodeRef::Node(id));
                }
            }
        }
        let instance = self.ir.instance(InstanceSpec { path: placing.path.clone(), name: placing.name, body: plan.body, parent: placing.parent, nodes: nodes.clone(), params, origin: placing.origin });
        stack.push(placing.module);
        for child in &plan.children {
            self.place_child(child, instance, &placing.path, &nodes, stack);
        }
        stack.pop();
        Some(instance)
    }

    /// One child of the instance `parent`: its ports bound to the parent's nodes.
    fn place_child(&mut self, child: &super::children::ChildPlan, parent: InstanceId, parent_path: &str, parent_nodes: &[NodeRef], stack: &mut Vec<DeclId>) {
        let node = self.design.expr_node(child.call).clone();
        let file = self.design.decl_data(node.owner).file;
        let ports = child
            .terminals
            .iter()
            .map(|t| match t {
                Terminal::Node(i) => parent_nodes.get(*i as usize).copied().unwrap_or(NodeRef::Ground),
                Terminal::Ground => NodeRef::Ground,
            })
            .collect();
        let placing = Placing {
            module: child.module,
            path: format!("{parent_path}.{}", child.name),
            name: child.name.clone(),
            parent: Some(parent),
            ports,
            given: child.given.clone(),
            file,
            span: node.span,
            origin: self.locator.origin(file, node.span),
        };
        self.place(placing, stack);
    }

    /// The value of each param of the instance: the one given, else the default. A required
    /// param left out is `E0319`; a false `where` is `E0317`, naming the param's path.
    fn bind_params(&mut self, placing: &Placing, plan: &ModulePlan) -> Option<Vec<Constant>> {
        let mut values = Vec::new();
        let mut bindings = Bindings::new();
        let mut ok = true;
        for (index, param) in plan.params.iter().enumerate() {
            let given = placing.given.iter().find(|(i, _, _)| *i as usize == index);
            match given.map(|(_, value, _)| *value).or(param.default) {
                Some(value) => {
                    bindings.bind(param.decl, value);
                    values.push(value);
                }
                None => {
                    let message = format!("`{}` leaves the required param `{}` unbound", placing.path, param.name);
                    self.report(placing.file, Diagnostic::new("E0319", message, placing.span, "required param not bound"));
                    ok = false;
                }
            }
        }
        if !ok {
            return None;
        }
        for (index, param) in plan.params.iter().enumerate() {
            let Some(cond) = param.where_ else { continue };
            let at = placing.given.iter().find(|(i, _, _)| *i as usize == index).map(|(_, _, expr)| self.design.expr_node(*expr).span).unwrap_or(placing.span);
            match self.consts.value(cond, &bindings) {
                Ok(Constant::Bool(true)) => {}
                Ok(Constant::Bool(false)) => {
                    let message = format!("the `where` of the param `{}.{}` is false for the value {}", placing.path, param.name, values[index]);
                    self.report(placing.file, Diagnostic::new("E0317", message, at, "`where` is false"));
                    ok = false;
                }
                Ok(Constant::Real(_) | Constant::Int(_)) => {
                    self.unsupported(placing.file, at, "a `where` that is not a `bool`");
                    ok = false;
                }
                Err(fault) => {
                    self.const_fault(&fault, "a `where` condition");
                    ok = false;
                }
            }
        }
        if let DeclKind::Module(m) = &self.design.decl_data(placing.module).kind
            && let Some(cond) = m.where_
        {
            match self.consts.value(cond, &bindings) {
                Ok(Constant::Bool(false)) => {
                    let message = format!("the `where` of module `{}` is false for the instance `{}`", self.design.decl_data(placing.module).name.as_ref().map_or("", |n| n.text.as_str()), placing.path);
                    self.report(placing.file, Diagnostic::new("E0317", message, placing.span, "`where` is false"));
                    ok = false;
                }
                Ok(_) => {}
                Err(fault) => {
                    self.const_fault(&fault, "a module `where` condition");
                    ok = false;
                }
            }
        }
        ok.then_some(values)
    }
}
