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
        self.place_children(&plan, instance, (&placing.path, placing.file), &bindings, (&mut nodes, &nets), stack);
        stack.pop();
        Some(Placed { instance, nodes, nets, host_ports })
    }

    /// Places the children of `plan` under `instance`, in source order except that a child connected to a node of another child
    /// (`Th(c0.sn)`, a reached node) waits until that child is placed and the node is bound. A node this body reaches is a node of
    /// its own to the siblings placed after and to the parent that reads it in turn (`bank.c0.sn`), so `nodes` takes it at once.
    /// Children that each wait for the other's node are refused by name (the checker refuses a name read before its `let` first,
    /// so no source reaches it today).
    fn place_children(&mut self, plan: &ModulePlan, instance: InstanceId, (path, file): (&str, FileId), bindings: &Bindings, (nodes, nets): (&mut Vec<NodeRef>, &[NetId]), stack: &mut Vec<BodyKey>) {
        let reached: Vec<u32> = plan.nodes.iter().enumerate().filter(|(_, node)| node.role == NodeRole::Reached).map(|(slot, _)| slot as u32).collect();
        let mut unbound: std::collections::BTreeSet<u32> = reached.into_iter().collect();
        let mut waiting: Vec<usize> = (0..plan.children.len()).collect();
        while !waiting.is_empty() {
            let Some(at) = waiting.iter().position(|index| !plan.children[*index].terminals.iter().any(|t| matches!(t, Terminal::Node(slot) if unbound.contains(slot)))) else {
                let child = &plan.children[waiting[0]];
                let span = self.design.expr_node(child.call).span;
                self.unsupported(file, span, "children each connected to a node of the other");
                return;
            };
            let index = waiting.remove(at);
            let parent = Parent { instance, path, nodes: nodes.as_slice(), nets, bindings };
            if let Some(placed) = self.place_child(&plan.children[index], &parent, stack) {
                for (slot, own) in self.bind_reaches(&plan.reaches, index as u32, instance, &placed) {
                    nodes[slot as usize] = own;
                    unbound.remove(&slot);
                }
            }
        }
    }

    /// Binds each node and carrier `instance` reads of its child number `child` (`inst.name`, a
    /// `Reach` of the body) to the child instance's own, now that the child is placed; the nodes bound, by slot.
    fn bind_reaches(&mut self, reaches: &[Reach], child: u32, instance: InstanceId, placed: &Placed) -> Vec<(u32, NodeRef)> {
        let mut bound = Vec::new();
        for reach in reaches.iter().filter(|r| r.child == child) {
            match reach.item {
                ReachedItem::Node { node, child_node } => {
                    if let Some(own) = placed.nodes.get(child_node as usize) {
                        self.ir.bind_reached_node(instance, node, *own);
                        bound.push((node, *own));
                    }
                }
                ReachedItem::Carrier { carrier, child_carrier } => {
                    if let Some(own) = placed.nets.get(child_carrier as usize) {
                        self.ir.bind_reached_net(instance, carrier, *own);
                    }
                }
            }
        }
        bound
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
                CarrierRole::Signal | CarrierRole::Reached | CarrierRole::Internal => None,
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

    /// The value of each param of the instance, the one the host bound before the run to a key param, else the one given, else
    /// the default (one that reads earlier params folded over their values here, §8.1.16), and the bindings its children's param
    /// values read. A required param left out is `E0319`; a false `where` is `E0317`, naming the param's path.
    fn bind_params(&mut self, placing: &Placing, plans: &[ParamPlan]) -> Option<(Vec<Constant>, Bindings)> {
        let mut values = Vec::new();
        let mut bindings = Bindings::new();
        let mut ok = true;
        for (index, param) in plans.iter().enumerate() {
            let given = placing.given.iter().find(|(i, _, _)| *i as usize == index).map(|(_, value, _)| *value);
            let chosen = match self.host_override(placing.module, &placing.path, plans, index, (placing.file, placing.span)).or(given) {
                Some(value) => Some(value),
                None => match self.default_of_instance(placing, param, &bindings) {
                    Ok(value) => value,
                    Err(()) => {
                        ok = false;
                        continue;
                    }
                },
            };
            match chosen {
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
                Ok(Constant::Real(_) | Constant::Int(_) | Constant::Aggregate(_) | Constant::FloatBits(_) | Constant::Wide(_)) => {
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

    /// The default of `param` at `placing`, folded over the values its earlier params take there (`bound`) under the arguments of
    /// the instance of a generic module it places. `Err` after the diagnostic of a default that does not fold.
    fn default_of_instance(&mut self, placing: &Placing, param: &ParamPlan, bound: &Bindings) -> Result<Option<Constant>, ()> {
        let outer = placing.instance.as_ref().map(|instance| self.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments)));
        let folded = self.default_at(param, bound);
        if let Some(outer) = outer {
            self.consts.set_substitution(outer);
        }
        folded.map_err(|fault| self.const_fault(&fault, "a param default"))
    }
}
