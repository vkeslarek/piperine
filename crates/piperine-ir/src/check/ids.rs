//! The reference check: every id names an entry of its arena, instances agree with their bodies and
//! their parents, the structure records of a body (merges, reaches, inflows) are in range and kept by
//! every instance of the body, and no two instances, nodes or nets share a path.

use std::collections::{BTreeMap, BTreeSet};

use super::{IdCheck, IrReader};
use crate::ids::{ExprId, StmtId};
use crate::model::{BodyData, BranchKind, CarrierRole, Child, ChildValue, EmitField, Expr, NodeRef, NodeRole, Origin, ReachedItem, Stmt, Terminal};

impl IdCheck<'_> {
    /// Runs the reference passes in their order.
    pub(super) fn run(&mut self) {
        self.references();
        self.instances();
        self.resolutions();
        self.structure();
        self.paths();
    }

    /// Reports `id` when the expression arena does not hold it.
    fn expression_id(&mut self, item: &str, origin: &Origin, id: ExprId) {
        let count = self.reader.ir.exprs.len();
        if id.index() >= count {
            self.findings.report(item.to_string(), origin.clone(), format!("expr id {} out of range ({count} exprs)", id.0));
        }
    }

    /// Reports `id` when the statement arena does not hold it.
    fn statement_id(&mut self, item: &str, origin: &Origin, id: StmtId) {
        let count = self.reader.ir.stmts.len();
        if id.index() >= count {
            self.findings.report(item.to_string(), origin.clone(), format!("stmt id {} out of range ({count} stmts)", id.0));
        }
    }

    /// Reports `function` when the function arena does not hold it.
    fn function_id(&mut self, item: &str, origin: &Origin, function: crate::ids::FunctionId) {
        let count = self.reader.ir.functions.len();
        if function.index() >= count {
            self.findings.report(item.to_string(), origin.clone(), format!("function id {} out of range ({count} functions)", function.0));
        }
    }

    /// Every id of every arena is in range: the top instance, each body's own records, the generic
    /// instance names, the functions, the expression and statement arenas, and the design's
    /// instances, nets and nodes.
    fn references(&mut self) {
        let ir = self.reader.ir;
        if ir.top.index() >= ir.instances.len() {
            self.findings.report("top".into(), IrReader::unknown_origin(), format!("top instance id {} out of range ({} instances)", ir.top.0, ir.instances.len()));
        }
        for body in &ir.bodies {
            self.body_references(body);
        }
        for body in &ir.bodies {
            if let Some(instance) = &body.instance
                && !instance.names_body(&body.name)
            {
                self.findings.report(format!("body {}", body.name), body.origin.clone(), format!("a body of the generic module instance `{}` is named `{}`", instance.name(), body.name));
            }
            self.child_param_ids(body);
            self.child_carrier_ids(body);
        }
        for function in &ir.functions {
            let item = format!("function {}", function.name);
            self.expression_id(&item, &function.origin, function.body);
            if let Some(instance) = &function.instance
                && !instance.names(&function.name)
            {
                self.findings.report(item, function.origin.clone(), format!("a generic instance of `{}` is named `{}`, not `{}`", instance.item, instance.name(), function.name));
            }
        }
        for (i, data) in ir.exprs.iter().enumerate() {
            let item = format!("expr {i}");
            let origin = data.origin.clone();
            self.expression_children(&item, &origin, &data.kind);
        }
        for (i, stmt) in ir.stmts.iter().enumerate() {
            self.statement_operands(&format!("stmt {i}"), stmt);
        }
        self.design_references();
    }

    /// The ids one body holds: its params' checks, its nodes' natures, its branch terminals, its
    /// statements and sites, its arrays, drives, events, reactions and children.
    fn body_references(&mut self, body: &BodyData) {
        let item = format!("body {}", body.name);
        let origin = body.origin.clone();
        for param in &body.params {
            if let Some(check) = param.check {
                self.expression_id(&item, &param.origin, check);
            }
        }
        for node in &body.nodes {
            if node.nature.index() >= self.reader.ir.natures.len() {
                self.findings.report(item.clone(), node.origin.clone(), format!("node {}: nature id {} out of range", node.name, node.nature.0));
            }
        }
        for branch in &body.branches {
            for t in [branch.plus, branch.minus] {
                match t {
                    Terminal::Node(i) if i as usize >= body.nodes.len() => {
                        self.findings.report(item.clone(), origin.clone(), format!("branch terminal node {i} out of range ({} nodes)", body.nodes.len()));
                    }
                    Terminal::Own(i) => self.findings.report(item.clone(), origin.clone(), format!("branch terminal own{i}: an own terminal belongs in a child's node slot only")),
                    Terminal::Node(_) | Terminal::Ground => {}
                }
            }
        }
        for &stmt in body.analog.iter().flatten() {
            self.statement_id(&item, &origin, stmt);
        }
        self.site_references(&item, body);
        self.carrier_arrays(&item, body);
        self.drive_and_event_references(&item, body);
        for reaction in &body.reactions {
            for &stmt in &reaction.stmts {
                self.statement_id(&item, &reaction.origin, stmt);
            }
        }
        for child in &body.children {
            self.child_references(&item, body, child);
        }
    }

    /// The expressions of a body's derivative, delay, noise and `limit` sites.
    fn site_references(&mut self, item: &str, body: &BodyData) {
        for site in &body.derivatives {
            for id in [site.state].into_iter().chain(site.abstol) {
                self.expression_id(item, &site.origin, id);
            }
        }
        for site in &body.delays {
            for id in [site.input, site.time].into_iter().chain(site.maximum) {
                self.expression_id(item, &site.origin, id);
            }
        }
        for source in &body.noise_sources {
            self.expression_id(item, &source.origin, source.density);
        }
        for site in &body.limits {
            if let Some(enabled) = site.enabled {
                self.expression_id(item, &site.origin, enabled);
            }
        }
    }

    /// The carriers and expressions of a body's drives, and the expressions of its events.
    fn drive_and_event_references(&mut self, item: &str, body: &BodyData) {
        for drive in &body.drives {
            if drive.carrier as usize >= body.carriers.len() {
                self.findings.report(item.to_string(), drive.origin.clone(), format!("drive of carrier {}, out of range ({} carriers)", drive.carrier, body.carriers.len()));
            }
            self.expression_id(item, &drive.origin, drive.value);
            if let Some(after) = drive.after {
                self.expression_id(item, &drive.origin, after);
            }
        }
        for event in &body.events {
            if let crate::model::EventSource::Cross { e, ttol, etol } = event.source {
                for id in [e].into_iter().chain(ttol).chain(etol) {
                    self.expression_id(item, &event.origin, id);
                }
            }
            if let crate::model::EventSource::At(e) = event.source {
                self.expression_id(item, &event.origin, e);
            }
            if let Some(guard) = event.guard {
                self.expression_id(item, &event.origin, guard);
            }
        }
    }

    /// A child of `body`: its body is in the arena, it binds as many nodes as that body has, its
    /// param indices are in range, and each terminal it binds is a node of `body` or its own.
    fn child_references(&mut self, item: &str, body: &BodyData, child: &Child) {
        let ir = self.reader.ir;
        match ir.bodies.get(child.body.index()) {
            None => self.findings.report(item.to_string(), child.origin.clone(), format!("child {}: body id {} out of range", child.name, child.body.0)),
            Some(callee) => {
                if child.nodes.len() != callee.nodes.len() {
                    self.findings.report(item.to_string(), child.origin.clone(), format!("child {} binds {} nodes, body {} has {}", child.name, child.nodes.len(), callee.name, callee.nodes.len()));
                }
                for (i, _) in &child.params {
                    if *i as usize >= callee.params.len() {
                        self.findings.report(item.to_string(), child.origin.clone(), format!("child {}: param index {i} out of range", child.name));
                    }
                }
            }
        }
        for (slot, t) in child.nodes.iter().enumerate() {
            match t {
                Terminal::Node(i) if *i as usize >= body.nodes.len() => {
                    self.findings.report(item.to_string(), child.origin.clone(), format!("child {}: terminal node {i} out of range", child.name));
                }
                Terminal::Own(k) => self.own_slot(item, child, slot, *k, ir.bodies.get(child.body.index())),
                Terminal::Node(_) | Terminal::Ground => {}
            }
        }
    }

    /// The expressions a statement of the arena names.
    fn statement_operands(&mut self, item: &str, stmt: &Stmt) {
        let origin = self.reader.statement_origin(stmt);
        match stmt {
            Stmt::Let { init, .. } => self.expression_id(item, &origin, *init),
            Stmt::Expr(e) => self.expression_id(item, &origin, *e),
            Stmt::Assign { value, .. } => self.expression_id(item, &origin, *value),
            Stmt::BoundStep { value, when, .. } => {
                for id in [*value].into_iter().chain(*when) {
                    self.expression_id(item, &origin, id);
                }
            }
            Stmt::Contribute { addends, .. } => {
                for a in addends {
                    self.expression_id(item, &origin, a.value);
                }
            }
            Stmt::Commit { value, after, target, .. } => {
                self.expression_id(item, &origin, *value);
                if let Some(after) = after {
                    self.expression_id(item, &origin, *after);
                }
                for index in target.iter().flat_map(crate::model::CommitTarget::index_operands) {
                    self.expression_id(item, &origin, index);
                }
            }
            Stmt::Emit { fields, .. } | Stmt::Trace { fields, .. } => {
                for f in fields {
                    if let EmitField::Value { value, .. } = f {
                        self.expression_id(item, &origin, *value);
                    }
                }
            }
        }
    }

    /// The ids the design's instances, nets and nodes hold.
    fn design_references(&mut self) {
        let ir = self.reader.ir;
        for instance in &ir.instances {
            let item = format!("instance {}", instance.path);
            let origin = instance.origin.clone();
            if instance.body.index() >= ir.bodies.len() {
                self.findings.report(item.clone(), origin.clone(), format!("body id {} out of range", instance.body.0));
            }
            for id in instance.parent.iter().chain(&instance.children) {
                if id.index() >= ir.instances.len() {
                    self.findings.report(item.clone(), origin.clone(), format!("instance id {} out of range", id.0));
                }
            }
            for n in &instance.nodes {
                if let NodeRef::Node(id) = n {
                    if id.index() >= ir.nodes.len() {
                        self.findings.report(item.clone(), origin.clone(), format!("node id {} out of range", id.0));
                    }
                }
            }
            for net in &instance.nets {
                if net.index() >= ir.nets.len() {
                    self.findings.report(item.clone(), origin.clone(), format!("net id {} out of range", net.0));
                }
            }
        }
        for net in &ir.nets {
            if net.declared_by.index() >= ir.instances.len() {
                self.findings.report(format!("net {}", net.path), net.origin.clone(), format!("declaring instance id {} out of range", net.declared_by.0));
            }
        }
        for node in &ir.nodes {
            let item = format!("node {}", node.path);
            if node.nature.index() >= ir.natures.len() {
                self.findings.report(item.clone(), node.origin.clone(), format!("nature id {} out of range", node.nature.0));
            }
            if node.declared_by.index() >= ir.instances.len() {
                self.findings.report(item, node.origin.clone(), format!("declaring instance id {} out of range", node.declared_by.0));
            }
        }
    }

    /// A child's own terminal `Own(k)` at `slot`: it must name its own slot, and that node of the
    /// child's body must be a `signal` or a node it reaches into a grandchild (a port is bound to the
    /// parent, never owned).
    fn own_slot(&mut self, item: &str, child: &Child, slot: usize, k: u32, callee: Option<&BodyData>) {
        if k as usize != slot {
            self.findings.report(item.to_string(), child.origin.clone(), format!("child {}: slot {slot} holds own{k}, an own terminal names its own slot", child.name));
            return;
        }
        let Some(node) = callee.and_then(|b| b.nodes.get(slot)) else { return };
        if node.role == NodeRole::Port {
            self.findings.report(item.to_string(), child.origin.clone(), format!("child {}: slot {slot} holds own{k}, but node {} of the child's body is a port", child.name, node.name));
        }
    }

    /// The values set beside the children: each names a child and a param of its body, once, and
    /// a constant is a value of the param's type.
    fn child_param_ids(&mut self, body: &BodyData) {
        let ir = self.reader.ir;
        let item = format!("body {}", body.name);
        for (ci, list) in body.child_params.iter().enumerate() {
            let Some(child) = body.children.get(ci) else {
                if !list.is_empty() {
                    self.findings.report(item.clone(), body.origin.clone(), format!("param values of child {ci}, out of range ({} children)", body.children.len()));
                }
                continue;
            };
            let callee = ir.bodies.get(child.body.index());
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            for (p, value) in list {
                let Some(param) = callee.and_then(|b| b.params.get(*p as usize)) else {
                    if callee.is_some() {
                        self.findings.report(item.clone(), child.origin.clone(), format!("child {}: param index {p} out of range", child.name));
                    }
                    continue;
                };
                if !seen.insert(*p) {
                    self.findings.report(item.clone(), child.origin.clone(), format!("child {}: param {} is set twice", child.name, param.name));
                }
                match value {
                    ChildValue::Const(c) if !self.reader.holds(&param.ty, *c) => {
                        self.findings.report(item.clone(), child.origin.clone(), format!("child {}: the value {c} of param {} is not a value of its type {}", child.name, param.name, param.ty));
                    }
                    ChildValue::Const(_) => {}
                    ChildValue::Expr(e) => self.expression_id(&item, &child.origin, *e),
                }
            }
        }
    }

    /// The carriers each placement binds: one entry per carrier port of the child's body, each a
    /// carrier of the parent holding the port's value type, or open.
    fn child_carrier_ids(&mut self, body: &BodyData) {
        let ir = self.reader.ir;
        let item = format!("body {}", body.name);
        self.child_windows(&item, body);
        for (ci, bound) in body.child_carriers.iter().enumerate() {
            if bound.is_empty() {
                continue;
            }
            let Some(child) = body.children.get(ci) else {
                self.findings.report(item.clone(), body.origin.clone(), format!("carrier bindings of child {ci}, out of range ({} children)", body.children.len()));
                continue;
            };
            let Some(callee) = ir.bodies.get(child.body.index()) else { continue };
            let ports: Vec<_> = callee.carriers.iter().filter(|c| matches!(c.role, CarrierRole::In | CarrierRole::Out | CarrierRole::Reached | CarrierRole::Resolved { port: true, .. })).collect();
            if bound.len() != ports.len() {
                self.findings.report(item.clone(), child.origin.clone(), format!("child {} binds {} carriers, body {} has {} carrier ports", child.name, bound.len(), callee.name, ports.len()));
            }
            for (port, binding) in ports.iter().zip(bound) {
                let Some(c) = binding else { continue };
                match body.carriers.get(*c as usize) {
                    None => self.findings.report(item.clone(), child.origin.clone(), format!("child {}: carrier {c} out of range ({} carriers)", child.name, body.carriers.len())),
                    Some(parent) if parent.ty != port.ty => self.findings.report(
                        item.clone(),
                        child.origin.clone(),
                        format!("child {} binds carrier {}: {} to the port {}: {}", child.name, parent.name, parent.ty, port.name, port.ty),
                    ),
                    Some(_) => {}
                }
            }
        }
    }

    /// The ids an expression of the arena names: its operands, the statements of its blocks, and
    /// the functions it calls or names.
    fn expression_children(&mut self, item: &str, origin: &Origin, kind: &Expr) {
        let mut exprs: Vec<ExprId> = Vec::new();
        match kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) => {}
            Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::ParamPrevious(_) | Expr::Others(_) => {}
            Expr::Resolve { by_count, .. } => {
                for function in by_count {
                    self.function_id(item, origin, *function);
                }
            }
            Expr::Unwrap { is, value, .. } => exprs.extend([*is, *value]),
            Expr::Same { a, b } => exprs.extend([*a, *b]),
            Expr::LevelCast { expr, .. } => exprs.push(*expr),
            Expr::Cast { expr, .. } => exprs.push(*expr),
            Expr::Method { receiver, args, .. } => {
                exprs.push(*receiver);
                exprs.extend(args);
            }
            Expr::Unary { expr, .. } => exprs.push(*expr),
            Expr::Binary { lhs, rhs, .. } => exprs.extend([*lhs, *rhs]),
            Expr::If { cond, then, otherwise } => exprs.extend([*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => {
                for &s in stmts {
                    self.statement_id(item, origin, s);
                }
                exprs.extend(value);
            }
            Expr::Call { function, args } => {
                self.function_id(item, origin, *function);
                exprs.extend(args);
            }
            Expr::Math { args, .. } => exprs.extend(args),
            Expr::Closure { body, .. } => exprs.push(*body),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => {
                for &s in kind.statements() {
                    self.statement_id(item, origin, s);
                }
                exprs.extend(kind.operands());
            }
            Expr::FunctionRef(function) => self.function_id(item, origin, *function),
            Expr::Limit { x, f, seed, .. } => {
                exprs.extend([*x, *f]);
                exprs.extend(seed);
            }
            Expr::Stim { mag, phase, .. } => {
                exprs.push(*mag);
                exprs.extend(phase);
            }
            Expr::SessionParameter { .. } => {}
        }
        for e in exprs {
            self.expression_id(item, origin, e);
        }
    }

    // ---- instances and paths -------------------------------------------------------------

    /// Each instance binds as many nodes, params and nets as its body has, its nets hold its carriers'
    /// types, and it agrees with its parent and its children (stored in pre-order).
    fn instances(&mut self) {
        let ir = self.reader.ir;
        for (i, instance) in ir.instances.iter().enumerate() {
            let Some(body) = ir.bodies.get(instance.body.index()) else { continue };
            let item = format!("instance {}", instance.path);
            if instance.nodes.len() != body.nodes.len() {
                self.findings.report(item.clone(), instance.origin.clone(), format!("binds {} nodes, body {} has {}", instance.nodes.len(), body.name, body.nodes.len()));
            }
            if instance.params.len() != body.params.len() {
                self.findings.report(item.clone(), instance.origin.clone(), format!("binds {} params, body {} has {}", instance.params.len(), body.name, body.params.len()));
            }
            if instance.nets.len() != body.carriers.len() {
                self.findings.report(item.clone(), instance.origin.clone(), format!("binds {} nets, body {} has {} carriers", instance.nets.len(), body.name, body.carriers.len()));
            }
            for (carrier, net) in body.carriers.iter().zip(&instance.nets) {
                if let Some(net) = ir.nets.get(net.index()) {
                    if net.ty != carrier.ty {
                        self.findings.report(item.clone(), instance.origin.clone(), format!("binds carrier {}: {} to net {}: {}", carrier.name, carrier.ty, net.path, net.ty));
                    }
                }
            }
            self.instance_family(i, &item);
        }
    }

    /// Instance `i` (named `item`) is listed by its parent, stored after it, the top when it has no
    /// parent, and named as parent by each of its children.
    fn instance_family(&mut self, i: usize, item: &str) {
        let ir = self.reader.ir;
        let instance = &ir.instances[i];
        match instance.parent.and_then(|p| ir.instances.get(p.index())) {
            Some(parent) => {
                if !parent.children.iter().any(|c| c.index() == i) {
                    self.findings.report(item.to_string(), instance.origin.clone(), format!("parent {} does not list it as a child", parent.path));
                }
                if instance.parent.is_some_and(|p| p.index() >= i) {
                    self.findings.report(item.to_string(), instance.origin.clone(), "stored before its parent (not pre-order)".to_string());
                }
            }
            None => {
                if instance.parent.is_none() && ir.top.index() != i {
                    self.findings.report(item.to_string(), instance.origin.clone(), "has no parent and is not the top".to_string());
                }
            }
        }
        for child in &instance.children {
            if let Some(c) = ir.instances.get(child.index()) {
                if c.parent.map(|p| p.index()) != Some(i) {
                    self.findings.report(item.to_string(), instance.origin.clone(), format!("child {} does not name it as its parent", c.path));
                }
            }
        }
    }

    /// The records of structure a body keeps (analog-structure): its merges, its reaches and its
    /// inflows, each in range and kept by every instance of the body.
    fn structure(&mut self) {
        let ir = self.reader.ir;
        for body in &ir.bodies {
            let item = format!("body {}", body.name);
            self.merges_in_range(&item, body);
            self.reaches_in_range(&item, body);
            for inflow in &body.inflows {
                let port = body.nodes.get(inflow.port as usize);
                if port.is_none_or(|n| n.role != NodeRole::Port) {
                    self.findings.report(item.clone(), body.origin.clone(), format!("inflow of node {}: not a port of the body", inflow.port));
                }
                match body.branches.get(inflow.branch as usize) {
                    Some(branch) if branch.kind == BranchKind::Potential => {}
                    Some(_) => self.findings.report(item.clone(), body.origin.clone(), format!("inflow measured by branch {}, which is not a potential branch", inflow.branch)),
                    None => self.findings.report(item.clone(), body.origin.clone(), format!("inflow measured by branch {}, out of range ({} branches)", inflow.branch, body.branches.len())),
                }
            }
            for (i, node) in body.nodes.iter().enumerate() {
                let listed = body.reaches.iter().any(|r| matches!(r.item, ReachedItem::Node { node, .. } if node as usize == i));
                if node.role == NodeRole::Reached && !listed {
                    self.findings.report(item.clone(), node.origin.clone(), format!("reached node {} names no child node in `reaches`", node.name));
                }
            }
            for (i, carrier) in body.carriers.iter().enumerate() {
                let listed = body.reaches.iter().any(|r| matches!(r.item, ReachedItem::Carrier { carrier, .. } if carrier as usize == i));
                if carrier.role == CarrierRole::Reached && !listed {
                    self.findings.report(item.clone(), carrier.origin.clone(), format!("reached carrier {} names no child carrier in `reaches`", carrier.name));
                }
            }
        }
        self.instances_keep_structure();
    }

    /// Each merge of `body` names nodes of the body, and two different ones.
    fn merges_in_range(&mut self, item: &str, body: &BodyData) {
        for merge in &body.merges {
            for end in [merge.plus, merge.minus] {
                match end {
                    Terminal::Node(i) if i as usize >= body.nodes.len() => {
                        self.findings.report(item.to_string(), body.origin.clone(), format!("merge terminal node {i} out of range ({} nodes)", body.nodes.len()));
                    }
                    Terminal::Own(i) => self.findings.report(item.to_string(), body.origin.clone(), format!("merge terminal own{i}: an own terminal belongs in a child's node slot only")),
                    Terminal::Node(_) | Terminal::Ground => {}
                }
            }
            if merge.plus == merge.minus {
                self.findings.report(item.to_string(), body.origin.clone(), "a merge of a node with itself".to_string());
            }
        }
    }

    /// Each reach names a child of the body, a reached node or carrier of the body and the
    /// child's own.
    fn reaches_in_range(&mut self, item: &str, body: &BodyData) {
        let ir = self.reader.ir;
        for reach in &body.reaches {
            let Some(child) = body.children.get(reach.child as usize) else {
                self.findings.report(item.to_string(), body.origin.clone(), format!("reach into child {}, out of range ({} children)", reach.child, body.children.len()));
                continue;
            };
            let Some(callee) = ir.bodies.get(child.body.index()) else { continue };
            match reach.item {
                ReachedItem::Node { node, child_node } => {
                    if body.nodes.get(node as usize).is_none_or(|n| n.role != NodeRole::Reached) {
                        self.findings.report(item.to_string(), child.origin.clone(), format!("reach of node {node}: not a node of role `reached`"));
                    }
                    if child_node as usize >= callee.nodes.len() {
                        self.findings.report(item.to_string(), child.origin.clone(), format!("reach of node {child_node} of child {}, out of range ({} nodes)", child.name, callee.nodes.len()));
                    }
                }
                ReachedItem::Carrier { carrier, child_carrier } => {
                    if body.carriers.get(carrier as usize).is_none_or(|c| !matches!(c.role, CarrierRole::Reached | CarrierRole::Resolved { .. })) {
                        self.findings.report(item.to_string(), child.origin.clone(), format!("reach of carrier {carrier}: not a carrier of role `reached` or `resolved`"));
                    }
                    if child_carrier as usize >= callee.carriers.len() {
                        self.findings.report(item.to_string(), child.origin.clone(), format!("reach of carrier {child_carrier} of child {}, out of range ({} carriers)", child.name, callee.carriers.len()));
                    }
                }
            }
        }
    }

    /// What every instance of a body does with the body's structure records: a merged branch's two
    /// ends are one ref, and a reached node or carrier is the child instance's own.
    fn instances_keep_structure(&mut self) {
        let ir = self.reader.ir;
        for instance in &ir.instances {
            let Some(body) = ir.bodies.get(instance.body.index()) else { continue };
            let item = format!("instance {}", instance.path);
            let end = |t: Terminal| match t {
                Terminal::Node(i) => instance.nodes.get(i as usize).copied(),
                Terminal::Ground => Some(NodeRef::Ground),
                Terminal::Own(_) => None,
            };
            for merge in &body.merges {
                if let (Some(plus), Some(minus)) = (end(merge.plus), end(merge.minus)) {
                    if plus != minus {
                        self.findings.report(item.clone(), instance.origin.clone(), "binds the two ends of a merged branch to different nodes".to_string());
                    }
                }
            }
            for reach in &body.reaches {
                let Some(child) = instance.children.get(reach.child as usize).and_then(|c| ir.instances.get(c.index())) else { continue };
                match reach.item {
                    ReachedItem::Node { node, child_node } => {
                        let (own, theirs) = (instance.nodes.get(node as usize), child.nodes.get(child_node as usize));
                        if own.is_some() && theirs.is_some() && own != theirs {
                            self.findings.report(item.clone(), instance.origin.clone(), format!("reached node {node} is not bound to node {child_node} of child {}", child.path));
                        }
                    }
                    ReachedItem::Carrier { carrier, child_carrier } => {
                        let (own, theirs) = (instance.nets.get(carrier as usize), child.nets.get(child_carrier as usize));
                        if own.is_some() && theirs.is_some() && own != theirs {
                            self.findings.report(item.clone(), instance.origin.clone(), format!("reached carrier {carrier} is not bound to carrier {child_carrier} of child {}", child.path));
                        }
                    }
                }
            }
        }
    }

    /// No two instances, nodes or nets share a path.
    fn paths(&mut self) {
        let ir = self.reader.ir;
        let mut instances: BTreeMap<&str, (u32, &Origin)> = BTreeMap::new();
        for i in &ir.instances {
            instances.entry(i.path.as_str()).or_insert((0, &i.origin)).0 += 1;
        }
        let mut nodes: BTreeMap<&str, (u32, &Origin)> = BTreeMap::new();
        for n in &ir.nodes {
            nodes.entry(n.path.as_str()).or_insert((0, &n.origin)).0 += 1;
        }
        let mut nets: BTreeMap<&str, (u32, &Origin)> = BTreeMap::new();
        for n in &ir.nets {
            nets.entry(n.path.as_str()).or_insert((0, &n.origin)).0 += 1;
        }
        let mut found = Vec::new();
        for (kind, table) in [("instance", instances), ("node", nodes), ("net", nets)] {
            for (path, (count, origin)) in table {
                if count > 1 {
                    found.push((format!("{kind} {path}"), origin.clone(), format!("{count} {kind}s share the path {path}")));
                }
            }
        }
        for (item, origin, what) in found {
            self.findings.report(item, origin, what);
        }
    }
}
