//! The `body`, `function` and `instance` sections of the printed tree.

use std::fmt::Write;

use crate::model::{BodyData, BranchKind, CarrierRole, ChildValue, EventSource, Expr, ExprData, FunctionData, InstanceData, NodeRef, Projection, Terminal};

use super::{Piece, Printer, Scope};

impl<'a> Printer<'a> {
    /// One body: its header, then one section per kind of record, in the order the text has always had.
    pub(super) fn body(&mut self, body: &BodyData) {
        let at = self.at(&body.origin);
        let instance = body.instance.as_ref().map(|instance| format!("  instance of {}", instance.item)).unwrap_or_default();
        self.line(0, &format!("body {}{at}{instance}", Printer::name(&body.name)));
        let scope = Scope { locals: &body.locals, params: &body.params, nodes: &body.nodes, carriers: &body.carriers, streams: &body.streams, session_parameters: &body.session_parameters };
        self.params(scope, body);
        self.signals(body);
        self.branches(scope, body);
        self.sites(scope, body);
        self.structure(scope, body);
        self.locals(1, &body.locals);
        self.children(scope, body);
        self.drives(scope, body);
        self.events(scope, body);
        self.behavior(scope, body);
    }

    /// The `param` lines: type, default, `where` check and `structural` mark.
    fn params(&mut self, scope: Scope<'_>, body: &BodyData) {
        for param in &body.params {
            let mut text = format!("param {}: {}", Printer::name(&param.name), param.ty);
            if let Some(default) = &param.default {
                let _ = write!(text, " = {default}");
            }
            if let Some(check) = param.check {
                let _ = write!(text, " where {}", self.render(scope, Piece::Expr(check)));
            }
            if param.structural {
                text.push_str(" structural");
            }
            text.push_str(&self.at(&param.origin));
            self.line(1, &text);
        }
    }

    /// The `node`, `carrier` and `stream` lines.
    fn signals(&mut self, body: &BodyData) {
        for node in &body.nodes {
            let nature = self.nature_name(node.nature.index());
            let role = match node.role {
                crate::model::NodeRole::Port => "port",
                crate::model::NodeRole::Signal => "signal",
                crate::model::NodeRole::Reached => "reached",
            };
            let init = node.init.map_or(String::new(), |r| format!(" init {}", Printer::real(r)));
            let at = self.at(&node.origin);
            self.line(1, &format!("node {}: {nature} {role}{init}{at}", Printer::name(&node.name)));
        }
        for carrier in &body.carriers {
            let role = match &carrier.role {
                CarrierRole::In => "in".to_string(),
                CarrierRole::Out => "out".to_string(),
                CarrierRole::Signal => "signal".to_string(),
                CarrierRole::Reached => "reached".to_string(),
                CarrierRole::Resolved { driven, .. } => format!("resolved {driven}"),
                CarrierRole::Internal => "internal".to_string(),
            };
            let init = carrier.init.map_or(String::new(), |c| format!(" init {c}"));
            let at = self.at(&carrier.origin);
            self.line(1, &format!("carrier {}: {} {role}{init}{at}", Printer::name(&carrier.name), carrier.ty));
        }
        for stream in &body.streams {
            let fields: Vec<String> = stream.fields.iter().map(|f| format!("{}: {}", Printer::name(&f.name), f.ty)).collect();
            self.line(1, &format!("stream {} ({})", Printer::name(&stream.name), fields.join(", ")));
        }
    }

    /// The `branch` lines.
    fn branches(&mut self, scope: Scope<'_>, body: &BodyData) {
        for (i, branch) in body.branches.iter().enumerate() {
            let kind = match branch.kind {
                BranchKind::Flow => "flow",
                BranchKind::Potential => "potential",
                BranchKind::Switch => "switch",
            };
            let plus = Printer::terminal(scope, branch.plus);
            let minus = Printer::terminal(scope, branch.minus);
            self.line(1, &format!("branch b{i} ({plus}, {minus}) {kind}"));
        }
    }

    /// The sites the code of the body refers to by index: `limit`, `stim`, `derivative`, `delay`, `ddx`, `trace`, `session`, `noise` and `hoisted`.
    fn sites(&mut self, scope: Scope<'_>, body: &BodyData) {
        for (i, site) in body.limits.iter().enumerate() {
            let enabled = site.enabled.map_or(String::new(), |e| format!(" enabled {}", self.render(scope, Piece::Expr(e))));
            let at = self.at(&site.origin);
            self.line(1, &format!("limit L{i} {:?}{enabled}{at}", site.name));
        }
        for (i, site) in body.stims.iter().enumerate() {
            let at = self.at(&site.origin);
            self.line(1, &format!("stim S{i} {:?}{at}", site.name));
        }
        for (i, site) in body.derivatives.iter().enumerate() {
            let mut text = format!("derivative D{i} {:?} state {}", site.name, self.render(scope, Piece::Expr(site.state)));
            if let Some(abstol) = site.abstol {
                let _ = write!(text, " abstol {}", self.render(scope, Piece::Expr(abstol)));
            }
            text.push_str(&self.at(&site.origin));
            self.line(1, &text);
        }
        for (i, site) in body.delays.iter().enumerate() {
            let maximum = site.maximum.map_or(String::new(), |m| format!(", {}", self.render(scope, Piece::Expr(m))));
            let mut text = format!("delay{i}({}, {}{maximum}) {:?}", self.render(scope, Piece::Expr(site.input)), self.render(scope, Piece::Expr(site.time)), site.name);
            text.push_str(&self.at(&site.origin));
            self.line(1, &text);
        }
        for (i, partial) in body.partial_derivatives.iter().enumerate() {
            let at = self.at(&partial.origin);
            self.line(1, &format!("ddx{i} {:?}{at}", partial.name));
        }
        for (i, site) in body.traces.iter().enumerate() {
            let stream = body.streams.get(site.stream as usize).map_or(format!("<bad stream {}>", site.stream), |s| format!("{:?}", s.name));
            let at = self.at(&site.origin);
            self.line(1, &format!("trace{i} {stream}{at}"));
        }
        for (i, parameter) in body.session_parameters.iter().enumerate() {
            let at = self.at(&parameter.origin);
            self.line(1, &format!("session S{i} {:?}{at}", parameter.name));
        }
        for (i, source) in body.noise_sources.iter().enumerate() {
            let group = source.group.as_ref().map_or("none".to_string(), |g| format!("{g:?}"));
            let mut text = format!("noise{i}({}, {group}) {:?}", self.render(scope, Piece::Expr(source.density)), source.name);
            text.push_str(&self.at(&source.origin));
            self.line(1, &text);
        }
        for hoisted in &body.hoisted {
            let at = self.at(&hoisted.origin);
            self.line(1, &format!("hoisted {}: {}{at}", Printer::name(&hoisted.name), Printer::name(&hoisted.module)));
        }
    }

    /// The structure records: `merge`, `reach` and `inflow`.
    fn structure(&mut self, scope: Scope<'_>, body: &BodyData) {
        for merge in &body.merges {
            self.line(1, &format!("merge ({}, {})", Printer::terminal(scope, merge.plus), Printer::terminal(scope, merge.minus)));
        }
        for reach in &body.reaches {
            let child = body.children.get(reach.child as usize);
            let callee = child.and_then(|c| self.ir.bodies.get(c.body.index()));
            let child_name = child.map_or("<bad child>".to_string(), |c| Printer::name(&c.name));
            let text = match reach.item {
                crate::model::ReachedItem::Node { node, child_node } => {
                    let own = scope.nodes.get(node as usize).map_or(format!("<bad node {node}>"), |n| Printer::name(&n.name));
                    let theirs = callee.and_then(|b| b.nodes.get(child_node as usize)).map_or(format!("<bad node {child_node}>"), |n| Printer::name(&n.name));
                    format!("reach node {own} = {child_name}.{theirs}")
                }
                crate::model::ReachedItem::Carrier { carrier, child_carrier } => {
                    let own = Printer::carrier(scope, carrier);
                    let theirs = callee.and_then(|b| b.carriers.get(child_carrier as usize)).map_or(format!("<bad carrier {child_carrier}>"), |c| Printer::name(&c.name));
                    format!("reach carrier {own} = {child_name}.{theirs}")
                }
            };
            self.line(1, &text);
        }
        for inflow in &body.inflows {
            let port = scope.nodes.get(inflow.port as usize).map_or(format!("<bad node {}>", inflow.port), |n| Printer::name(&n.name));
            self.line(1, &format!("inflow {port} measured by b{}", inflow.branch));
        }
    }

    /// The `child` lines: the callee, its nodes, params and carriers.
    fn children(&mut self, scope: Scope<'_>, body: &BodyData) {
        for (ci, child) in body.children.iter().enumerate() {
            let callee = self.ir.bodies.get(child.body.index());
            let name = callee.map_or("<bad body>".to_string(), |b| Printer::name(&b.name));
            // A child's own node reads as the callee names it: `own:<name>` (Q1).
            let own = |k: u32| callee.and_then(|b| b.nodes.get(k as usize)).map_or(format!("own:<bad node {k}>"), |n| format!("own:{}", Printer::name(&n.name)));
            let nodes: Vec<String> = child.nodes.iter().map(|t| match t {
                Terminal::Own(k) => own(*k),
                other => Printer::terminal(scope, *other),
            }).collect();
            let pname = |i: u32| callee.and_then(|b| b.params.get(i as usize)).map_or("<bad param>".to_string(), |p| Printer::name(&p.name));
            // A value set beside the child (`ChildValue`) shows in place of the constant of the
            // same param; one with no constant follows the constants.
            let entries: &[(u32, ChildValue)] = body.child_params.get(ci).map_or(&[], Vec::as_slice);
            let mut params: Vec<String> = Vec::new();
            for (i, v) in &child.params {
                match entries.iter().find(|(j, _)| j == i) {
                    Some((_, value)) => params.push(format!("{} = {}", pname(*i), self.child_value(scope, value))),
                    None => params.push(format!("{} = {v}", pname(*i))),
                }
            }
            for (i, value) in entries {
                if !child.params.iter().any(|(j, _)| j == i) {
                    params.push(format!("{} = {}", pname(*i), self.child_value(scope, value)));
                }
            }
            // The carriers the placement connects, by the parent's names: `_` for an open port.
            let bound = body.child_carriers.get(ci).map_or(&[][..], Vec::as_slice);
            let carriers = if bound.is_empty() {
                String::new()
            } else {
                let names: Vec<String> = bound.iter().map(|c| c.map_or("_".to_string(), |c| Printer::carrier(scope, c))).collect();
                format!("  carriers ({})", names.join(", "))
            };
            let at = self.at(&child.origin);
            self.line(
                1,
                &format!("child {}: {name}  nodes ({})  params ({}){carriers}{at}", Printer::name(&child.name), nodes.join(", "), params.join(", ")),
            );
        }
    }

    /// The `drive` lines.
    fn drives(&mut self, scope: Scope<'_>, body: &BodyData) {
        for drive in &body.drives {
            let mut text = format!("drive {}", Printer::carrier(scope, drive.carrier));
            if let Some(element) = drive.element {
                text.push_str(&format!("[{element}]"));
            }
            for step in &drive.path {
                match step {
                    Projection::Field(k) => text.push_str(&format!(".{k}")),
                    Projection::Index(index) => text.push_str(&format!("[{}]", self.render(scope, Piece::Expr(*index)))),
                }
            }
            text.push_str(" = ");
            text.push_str(&self.render(scope, Piece::Expr(drive.value)));
            if let Some(after) = drive.after {
                text.push_str(" after ");
                text.push_str(&self.render(scope, Piece::Expr(after)));
            }
            text.push_str(&self.at(&drive.origin));
            self.line(1, &text);
        }
    }

    /// The `event` lines.
    fn events(&mut self, scope: Scope<'_>, body: &BodyData) {
        for (k, event) in body.events.iter().enumerate() {
            let mut text = format!("event E{k} ");
            match &event.source {
                EventSource::Changed(c) => text.push_str(&format!("changed {}", Printer::carrier(scope, *c))),
                // `settle` (§13 rule 17) names no carrier, param or expression.
                EventSource::Settle => text.push_str("settle"),
                EventSource::ElementChanged { carrier, element } => {
                    text.push_str(&format!("changed {}[{element}]", Printer::carrier(scope, *carrier)))
                }
                EventSource::ParamChanged(p) => {
                    let name = scope.params.get(*p as usize).map_or(format!("<bad param {p}>"), |param| Printer::name(&param.name));
                    text.push_str(&format!("param_changed {name}"));
                }
                EventSource::Cross { e, ttol, etol } => {
                    text.push_str("cross ");
                    text.push_str(&self.render(scope, Piece::Expr(*e)));
                    for (name, tol) in [("ttol", ttol), ("etol", etol)] {
                        if let Some(tol) = tol {
                            text.push_str(&format!(" {name} {}", self.render(scope, Piece::Expr(*tol))));
                        }
                    }
                }
                EventSource::At(e) => {
                    text.push_str("at ");
                    text.push_str(&self.render(scope, Piece::Expr(*e)));
                }
            }
            if let Some(guard) = event.guard {
                text.push_str(" guard ");
                text.push_str(&self.render(scope, Piece::Expr(guard)));
            }
            text.push_str(&self.at(&event.origin));
            self.line(1, &text);
        }
    }

    /// The `analog` block and the reactions.
    fn behavior(&mut self, scope: Scope<'_>, body: &BodyData) {
        if let Some(analog) = &body.analog {
            self.line(1, "analog");
            for &stmt in analog {
                let text = self.render(scope, Piece::Stmt(stmt));
                self.line(2, &text);
            }
        }
        for (k, reaction) in body.reactions.iter().enumerate() {
            let on: Vec<String> = reaction.on.iter().map(|e| format!("E{e}")).collect();
            let at = self.at(&reaction.origin);
            self.line(1, &format!("reaction R{k} on ({}){at}", on.join(", ")));
            self.locals(2, &reaction.locals);
            let scope = Scope { locals: &reaction.locals, ..scope };
            for &stmt in &reaction.stmts {
                let text = self.render(scope, Piece::Stmt(stmt));
                self.line(2, &text);
            }
        }
    }

    /// A param value of a placement: a constant as written, an expression in the parent's scope.
    pub(super) fn child_value(&mut self, scope: Scope<'_>, value: &ChildValue) -> String {
        match value {
            ChildValue::Const(c) => c.to_string(),
            ChildValue::Expr(e) => self.render(scope, Piece::Expr(*e)),
        }
    }

    /// One function: its signature, its locals and its body, one statement per line.
    pub(super) fn function(&mut self, function: &FunctionData) {
        let params: Vec<String> = function.params.iter().map(|p| format!("{}: {}", Printer::name(&p.name), p.ty)).collect();
        let at = self.at(&function.origin);
        let instance = function.instance.as_ref().map(|instance| format!("  // instance of {}", instance.item)).unwrap_or_default();
        self.line(0, &format!("fn {}({}) -> {}{at}{instance}", Printer::name(&function.name), params.join(", "), function.ret));
        self.locals(1, &function.locals);
        let scope = Scope { locals: &function.locals, params: &[], nodes: &[], carriers: &[], streams: &[], session_parameters: &[] };
        let body = function.body;
        // A block body prints one statement per line. In the full text its id is printed and on
        // the stack meanwhile. In the structural form each line is a statement of its own,
        // guarded by its own walk, and a block whose type its value does not imply prints whole,
        // with its type.
        let lines = match self.ir.exprs.get(body.index()) {
            Some(ExprData { kind: Expr::Block { stmts, value }, ty, .. }) => {
                let fresh = if self.structural { *ty == Printer::block_type(self.ir, value) } else { !self.printed.contains(&body) };
                fresh.then_some((stmts, value))
            }
            _ => None,
        };
        let Some((stmts, value)) = lines else {
            let text = self.render(scope, Piece::Expr(body));
            self.line(1, &text);
            return;
        };
        if !self.structural {
            self.printed.insert(body);
            self.on_stack.insert(body);
        }
        for &stmt in stmts {
            let text = self.render(scope, Piece::Stmt(stmt));
            self.line(1, &text);
        }
        if let Some(value) = value {
            let text = self.render(scope, Piece::Expr(*value));
            self.line(1, &text);
        }
        if !self.structural {
            self.on_stack.remove(&body);
        }
    }

    /// One instance line: its body, nodes, params, nets and whether it is the top.
    pub(super) fn instance(&mut self, index: usize, instance: &InstanceData) {
        let body = self.ir.bodies.get(instance.body.index());
        let body_name = body.map_or("<bad body>".to_string(), |b| Printer::name(&b.name));
        let nodes: Vec<String> = instance
            .nodes
            .iter()
            .map(|n| match n {
                NodeRef::Ground => "gnd".to_string(),
                NodeRef::Node(id) => self.ir.nodes.get(id.index()).map_or(format!("<bad node {}>", id.0), |n| Printer::name(&n.path)),
            })
            .collect();
        let params: Vec<String> = instance
            .params
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let pname = body.and_then(|b| b.params.get(i)).map_or("<bad param>".to_string(), |p| Printer::name(&p.name));
                format!("{pname} = {v}")
            })
            .collect();
        let top = if self.ir.top.index() == index { "  top" } else { "" };
        let at = self.at(&instance.origin);
        let nets = if instance.nets.is_empty() {
            String::new()
        } else {
            let nets: Vec<String> = instance
                .nets
                .iter()
                .map(|id| self.ir.nets.get(id.index()).map_or(format!("<bad net {}>", id.0), |n| Printer::name(&n.path)))
                .collect();
            format!("  nets ({})", nets.join(", "))
        };
        self.line(
            0,
            &format!(
                "instance {}: {body_name}  nodes ({})  params ({}){nets}{top}{at}",
                Printer::name(&instance.path),
                nodes.join(", "),
                params.join(", ")
            ),
        );
    }
}
