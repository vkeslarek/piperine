//! Placement: the instance tree, the node table and the net table. An instance is one placement
//! of a module, bound to the parent's nodes and nets and to param values; the tree is walked in
//! pre-order, children in source order. Every signal of every instance is one node of the design,
//! named by path; a port is not a node, it is bound to the parent's node or to ground (§21 rules 1
//! and 3), except a node port of the top, which is a node of the design for the host to drive
//! (§8.1.14, `host_sources.rs`). The discrete twin: every signal carrier of every instance is one net, named by path,
//! a carrier port is bound to the parent's net, and a discrete port of the top is a net of the
//! top (DIG-09 AC 4 and 5).

use piperine_ir::{CarrierRole, Constant, InstanceId, Reach, ReachedItem, InstanceSpec, NetId, NetSpec, NodeRef, NodeSpec, Origin, Terminal};

use super::body::{ModulePlan, ParamPlan};
use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::instances::Instance;
use super::structural::BodyKey;
use super::host_sources::{HostPort, HostSources};
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, ExprId, FileId};
use piperine_ir::NodeRole;

/// One placement about to become an instance.
struct Placing {
    module: DeclId,
    /// The arguments of the instance of a generic module this placement makes.
    instance: Option<Instance>,
    /// `Top.r1`: the path of the instance.
    path: String,
    name: String,
    parent: Option<InstanceId>,
    /// What each port of the module is bound to, in port order.
    ports: Vec<NodeRef>,
    /// The net each carrier port of the module is bound to, in port order; empty for the top.
    carrier_ports: Vec<Option<NetId>>,
    /// The values the placement writes: (index in the module's params, value, the expression).
    given: Vec<(u32, Constant, ExprId)>,
    /// Where diagnostics of the placement sit.
    file: FileId,
    span: Span,
    origin: Origin,
}

/// An instance placed: its own nodes and nets, which a parent that reaches into it binds to.
struct Placed {
    instance: InstanceId,
    nodes: Vec<NodeRef>,
    nets: Vec<NetId>,
    /// The node the top's placement made for each of its node ports (§8.1.14); empty below the top.
    host_ports: Vec<HostPort>,
}

/// The instance whose children are being placed: what they bind to.
struct Parent<'a> {
    instance: InstanceId,
    path: &'a str,
    nodes: &'a [NodeRef],
    nets: &'a [NetId],
    /// The values of its params, which its children's param values may read.
    bindings: &'a Bindings,
}

impl Elaboration<'_, '_> {
    /// The top instance and, below it, the whole tree. `None` after a diagnostic.
    pub(super) fn place_top(&mut self, top: DeclId) -> Option<InstanceId> {
        let data = self.design.decl_data(top).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let span = data.name.as_ref().map_or(data.span, |n| n.span);
        let placing = Placing { module: top, instance: None, path: name.clone(), name, parent: None, ports: Vec::new(), carrier_ports: Vec::new(), given: Vec::new(), file: data.file, span, origin: self.locator.decl(top) };
        let path = placing.path.clone();
        let placed = self.place(placing, &mut Vec::new())?;
        HostSources::new(&mut self.ir).place(placed.instance, &path, &placed.host_ports);
        Some(placed.instance)
    }

    /// Place `placing`, then its children. `stack` holds the modules being placed above it; a chain of generic module
    /// instances may be a thousand deep, so each level runs on a stack segment of its own when the stack runs low.
    fn place(&mut self, placing: Placing, stack: &mut Vec<BodyKey>) -> Option<Placed> {
        crate::stack::Stack::wide(|| self.place_instance(placing, stack))
    }

    /// [`Elaboration::place`] on the stack it runs on.
    fn place_instance(&mut self, placing: Placing, stack: &mut Vec<BodyKey>) -> Option<Placed> {
        let param_plans = self.plans_of(placing.module, placing.instance.as_ref())?.clone();
        let (params, bindings) = self.bind_params(&placing, &param_plans)?;
        let (params, bindings) = self.apply_overrides(placing.module, &placing.path, &param_plans, params, bindings, (placing.file, placing.span));
        // a module that places itself with the params it was placed with never ends; with others it is a recursion by
        // generation that the params bring to an end (§8.2.7), bounded by the depth of D7
        let key = self.body_key(placing.module, placing.instance.as_ref(), &bindings)?;
        if stack.contains(&key) {
            let module = self.design.decl_data(placing.module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let message = format!("module `{module}` places itself (through `{}`)", placing.path);
            self.report(placing.file, Diagnostic::new("E0341", message, placing.span, "recursive placement"));
            return None;
        }
        let plan = self.body_plan(placing.module, placing.instance.as_ref(), &bindings)?;
        let declared_by = self.ir.next_instance_id();
        let mut ports = placing.ports.iter().copied();
        let mut nodes = Vec::new();
        let mut host_ports = Vec::new();
        for node in &plan.nodes {
            match node.role {
                // The top has no parent to bind a port: its node is the design's own, driven by
                // the host (§8.1.14). An open port of a child is ground.
                NodeRole::Port => match ports.next() {
                    Some(bound) => nodes.push(bound),
                    None if placing.parent.is_none() => {
                        let id = self.ir.node(NodeSpec { path: format!("{}.{}", placing.path, node.name), nature: node.nature, declared_by, init: node.init, origin: node.origin.clone() });
                        host_ports.push(HostPort { name: node.name.clone(), node: id, nature: node.nature, origin: node.origin.clone() });
                        nodes.push(NodeRef::Node(id));
                    }
                    None => nodes.push(NodeRef::Ground),
                },
                NodeRole::Signal => {
                    let id = self.ir.node(NodeSpec { path: format!("{}.{}", placing.path, node.name), nature: node.nature, declared_by, init: node.init, origin: node.origin.clone() });
                    nodes.push(NodeRef::Node(id));
                }
                // Bound to the child's node once the child is placed (`IrBuilder::bind_reached_node`).
                NodeRole::Reached => nodes.push(NodeRef::Ground),
            }
        }
        self.merge_instance_nodes(&plan, &nodes);
        let nets = self.nets(&placing, &plan.carriers, declared_by);
        let instance = self.ir.instance(InstanceSpec { path: placing.path.clone(), name: placing.name, body: plan.body, parent: placing.parent, nodes: nodes.clone(), params, origin: placing.origin });
        if !nets.is_empty() {
            self.ir.bind_nets(instance, nets.clone());
        }
        stack.push(key);
        for (index, child) in plan.children.iter().enumerate() {
            let parent = Parent { instance, path: &placing.path, nodes: &nodes, nets: &nets, bindings: &bindings };
            if let Some(placed) = self.place_child(child, &parent, stack) {
                self.bind_reaches(&plan.reaches, index as u32, instance, &placed);
            }
        }
        stack.pop();
        Some(Placed { instance, nodes, nets, host_ports })
    }

    /// Binds each node and carrier `instance` reads of its child number `child` (`inst.name`, a
    /// `Reach` of the body) to the child instance's own, now that the child is placed.
    fn bind_reaches(&mut self, reaches: &[Reach], child: u32, instance: InstanceId, placed: &Placed) {
        for reach in reaches.iter().filter(|r| r.child == child) {
            match reach.item {
                ReachedItem::Node { node, child_node } => {
                    if let Some(own) = placed.nodes.get(child_node as usize) {
                        self.ir.bind_reached_node(instance, node, *own);
                    }
                }
                ReachedItem::Carrier { carrier, child_carrier } => {
                    if let Some(own) = placed.nets.get(child_carrier as usize) {
                        self.ir.bind_reached_net(instance, carrier, *own);
                    }
                }
            }
        }
    }

    /// Makes the two ends of each branch the body merges one node in the instance whose nodes are
    /// `nodes` (`IrBuilder::merge_nodes`): ground absorbs the node it meets.
    fn merge_instance_nodes(&mut self, plan: &ModulePlan, nodes: &[NodeRef]) {
        let end = |terminal: &Terminal| match terminal {
            Terminal::Node(i) => nodes.get(*i as usize).copied().unwrap_or(NodeRef::Ground),
            _ => NodeRef::Ground,
        };
        for (plus, minus) in &plan.merges {
            match (end(plus), end(minus)) {
                (kept, NodeRef::Node(merged)) => self.ir.merge_nodes(kept, merged),
                (NodeRef::Node(merged), kept @ NodeRef::Ground) => self.ir.merge_nodes(kept, merged),
                _ => {}
            }
        }
    }

    /// The net of each carrier of the instance being placed, in the body's carrier order: a carrier
    /// port the parent binds takes the parent's net; any other carrier (a signal, a port of the
    /// top) declares its own, named by its path, with the carrier's initial value as the net's
    /// (§5.2.3: the declaring module gives it).
    fn nets(&mut self, placing: &Placing, carriers: &[piperine_ir::Carrier], declared_by: InstanceId) -> Vec<NetId> {
        let mut bound = placing.carrier_ports.iter().copied();
        let mut nets = Vec::new();
        for carrier in carriers {
            let given = match carrier.role {
                CarrierRole::In | CarrierRole::Out => bound.next().flatten(),
                CarrierRole::Signal | CarrierRole::Reached => None,
            };
            // A reached carrier has no net of its own: this one is a placeholder `bind_reaches`
            // replaces with the child's, so its path cannot be the child's (`Top.c.state`).
            let at = if carrier.role == CarrierRole::Reached { format!("{}.reach.{}", placing.path, carrier.name) } else { format!("{}.{}", placing.path, carrier.name) };
            let net = match given {
                Some(net) => net,
                None => self.ir.net(NetSpec { path: at, ty: carrier.ty.clone(), declared_by, init: carrier.init, origin: carrier.origin.clone() }),
            };
            nets.push(net);
        }
        nets
    }

    /// One child of the instance `parent`: its ports bound to the parent's nodes and nets, each
    /// param value that reads the parent's params evaluated over the parent's bound values
    /// `bindings` (DIG-09 AC 11).
    fn place_child(&mut self, child: &super::children::ChildPlan, parent: &Parent, stack: &mut Vec<BodyKey>) -> Option<Placed> {
        let node = self.design.expr_node(child.call).clone();
        let file = self.design.decl_data(node.owner).file;
        let ports = child
            .terminals
            .iter()
            .map(|t| match t {
                Terminal::Node(i) => parent.nodes.get(*i as usize).copied().unwrap_or(NodeRef::Ground),
                Terminal::Ground => NodeRef::Ground,
                Terminal::Own(_) => panic!("Ir::check: an own terminal belongs in a child's node slot only"),
            })
            .collect();
        let carrier_ports = child.carriers.iter().map(|c| c.and_then(|i| parent.nets.get(i as usize).copied())).collect();
        let mut given = Vec::new();
        for (index, value, expr) in &child.given {
            let value = match value {
                Some(value) => *value,
                None => match self.fold_param_value(*expr, parent.bindings) {
                    Ok(value) => value,
                    Err(fault) => {
                        self.const_fault(&fault, "a param value");
                        return None;
                    }
                },
            };
            given.push((*index, value, *expr));
        }
        let placing = Placing {
            module: child.module,
            instance: child.instance.clone(),
            path: format!("{}.{}", parent.path, child.name),
            name: child.name.clone(),
            parent: Some(parent.instance),
            ports,
            carrier_ports,
            given,
            file,
            span: node.span,
            origin: self.locator.origin(file, node.span),
        };
        self.place(placing, stack)
    }

    /// The value of each param of the instance, the one given, else the default, and the
    /// bindings its children's param values read. A required param left out is `E0319`; a false
    /// `where` is `E0317`, naming the param's path.
    fn bind_params(&mut self, placing: &Placing, plans: &[ParamPlan]) -> Option<(Vec<Constant>, Bindings)> {
        let mut values = Vec::new();
        let mut bindings = Bindings::new();
        let mut ok = true;
        for (index, param) in plans.iter().enumerate() {
            let given = placing.given.iter().find(|(i, _, _)| *i as usize == index);
            match given.map(|(_, value, _)| *value).or(param.default) {
                Some(value) => {
                    self.bind_param(&mut bindings, param.decl, value);
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
        for (index, param) in plans.iter().enumerate() {
            let Some(cond) = param.where_ else { continue };
            let at = placing.given.iter().find(|(i, _, _)| *i as usize == index).map(|(_, _, expr)| self.design.expr_node(*expr).span).unwrap_or(placing.span);
            match self.consts.value(cond, &bindings) {
                Ok(Constant::Bool(true)) => {}
                Ok(Constant::Bool(false)) => {
                    let message = format!("the `where` of the param `{}.{}` is false for the value {}", placing.path, param.name, values[index]);
                    self.report(placing.file, Diagnostic::new("E0317", message, at, "`where` is false"));
                    ok = false;
                }
                Ok(Constant::Real(_) | Constant::Int(_) | Constant::Aggregate(_)) => {
                    self.unsupported(placing.file, at, "a `where` that is not a `bool`");
                    ok = false;
                }
                Err(fault) => {
                    self.const_fault(&fault, "a `where` condition");
                    ok = false;
                }
            }
        }
        // the `where` of an instance of a generic module is discharged when its body is asked for (`generic_bodies.rs`)
        if placing.instance.is_none()
            && let DeclKind::Module(m) = &self.design.decl_data(placing.module).kind
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
        ok.then_some((values, bindings))
    }
}
