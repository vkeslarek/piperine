//! `inst.name` through a child of a child (§8.2.8, §8.4.8): `bank.c0.sn`, where `c0` is a `pub let` of `bank`'s module and its
//! placement goes through a template-typed param (`cell: C`, `C: Fn(..)`), so the module that owns `sn` is only known once the
//! template is folded. A body reads the items of its own children, never of a grandchild's, so the child's body carries a reached
//! node for the item (`NodeRole::Reached`, bound to the grandchild's own when it is placed) and the body that reads it reaches that node.
//!
//! * A **demand** is one `child.item` read through a child (`bank.c0.sn` demands `c0.sn` of the module of `bank`): they are
//!   found once for the design, in source order, and each body of the module that is the child lowers a reached node for every
//!   demand its variant can meet, right after its own nodes (`lower_nested_demands`), so the node's index is known to the reader.
//! * The reader resolves the item against the concrete module (`E0343` for a missing or non-`pub` one), makes its own reached node
//!   of the child's node, and binds it to the child's (`nested_terminal`).

use std::collections::BTreeMap;

use piperine_ir::{BodyNode, NodeRole, Reach, ReachedItem, Terminal};

use super::body::NodePlan;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

/// One item `item` of the child `child` (a `let` binding of a module's body) that another body reads as `owner.child.item`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NestedDemand {
    /// The `let` binding of the placement in the module's body.
    pub(super) child: DeclId,
    /// The name of the item the read names.
    pub(super) item: String,
}

/// The demands of the design, by the module whose body holds the child.
#[derive(Debug, Default)]
pub(super) struct NestedDemands {
    found: Option<BTreeMap<DeclId, Vec<NestedDemand>>>,
}

impl Elaboration<'_, '_> {
    /// The demands the design makes of `module`, in source order: found the first time any module asks.
    fn demands_of(&mut self, module: DeclId) -> Vec<NestedDemand> {
        if self.nested_demands.found.is_none() {
            let found = self.find_nested_demands();
            self.nested_demands.found = Some(found);
        }
        self.nested_demands.found.as_ref().and_then(|all| all.get(&module)).cloned().unwrap_or_default()
    }

    /// Every read `a.b.x` of the design where `b` is a `let` of the module `a` names: a demand of that module.
    fn find_nested_demands(&self) -> BTreeMap<DeclId, Vec<NestedDemand>> {
        let mut found: BTreeMap<DeclId, Vec<NestedDemand>> = BTreeMap::new();
        for index in 0..self.design.expr_count() {
            let expr = ExprId(index as u32);
            if self.typed(expr).is_none() {
                continue;
            }
            let ExprKind::Field { expr: receiver, name, .. } = &self.design.expr_node(expr).kind else { continue };
            let Some((module, child)) = self.child_of_module(*receiver) else { continue };
            let demand = NestedDemand { child, item: name.text.clone() };
            let list = found.entry(module).or_default();
            if !list.contains(&demand) {
                list.push(demand);
            }
        }
        found
    }

    /// The `let` binding and the module whose body holds it, when `receiver` (through parentheses and indexes) is `owner.child` with
    /// `child` the name of a placement `let`.
    fn child_of_module(&self, receiver: ExprId) -> Option<(DeclId, DeclId)> {
        let mut at = receiver;
        loop {
            match &self.design.expr_node(at).kind {
                ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => at = *inner,
                _ => break,
            }
        }
        let ExprKind::Field { res: Resolution::Decl(child), .. } = &self.design.expr_node(at).kind else { return None };
        let value = self.body_let_value(*child)?;
        let ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. } = &self.design.expr_node(self.peel_parens(value)).kind else { return None };
        let mut parent = self.design.decl_data(*child).parent;
        while let Some(decl) = parent {
            if matches!(self.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some((decl, *child));
            }
            parent = self.design.decl_data(decl).parent;
        }
        None
    }

    /// The item named `name` of `module` (a port, a signal, a param or a `let` name, entering generation).
    fn item_named(&self, module: DeclId, name: &str) -> Option<DeclId> {
        let mut pending = vec![module];
        while let Some(at) = pending.pop() {
            for child in &self.design.decl_data(at).children {
                let data = self.design.decl_data(*child);
                let named = data.name.as_ref().is_some_and(|n| n.text == name);
                match &data.kind {
                    DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) if named => return Some(*child),
                    DeclKind::Let(_) => {
                        if let Some(binding) = data.children.iter().find(|b| self.design.decl_data(**b).name.as_ref().is_some_and(|n| n.text == name)) {
                            return Some(*binding);
                        }
                    }
                    DeclKind::Generation(_) => pending.push(*child),
                    _ => {}
                }
            }
        }
        None
    }

    /// Whether the code of a parent may name `item` of a child: a port always, any other item when it is `pub` (§8.2.8).
    fn is_visible_item(&self, item: DeclId) -> bool {
        let data = self.design.decl_data(item);
        match &data.kind {
            DeclKind::Port(_) => true,
            DeclKind::Binding(_) => data.parent.is_some_and(|p| self.design.decl_data(p).public),
            _ => data.public,
        }
    }

    /// The node item of the concrete module of `demand`'s child that the body of `frame` reads: the child's placement
    /// resolved under the key params of the frame, its module's item of that name when it is a visible port or signal that is
    /// a node. `None` for a demand this variant cannot meet.
    fn met_demand(&self, frame: usize, demand: &NestedDemand) -> Option<DeclId> {
        let value = self.body_let_value(demand.child)?;
        let module = self.placed_module(self.peel_parens(value), &self.frame_bindings(frame))?;
        let item = self.item_named(module, &demand.item)?;
        let is_node = matches!(self.design.decl_data(item).kind, DeclKind::Port(_) | DeclKind::Signal(_)) && self.types.carrier(item).is_none();
        (is_node && self.is_visible_item(item)).then_some(item)
    }

    /// Gives the body of `module`, being lowered, the reached node of every demand its variant meets, right after its own nodes.
    pub(super) fn lower_nested_demands(&mut self, scope: &Scope, module: DeclId) {
        let demands = self.demands_of(module);
        let mut slot = self.structure.nodes.len() as u32;
        for demand in demands {
            let Some(item) = self.met_demand(0, &demand) else { continue };
            let Some(value) = self.body_let_value(demand.child) else { continue };
            let call = self.peel_parens(value);
            let key = self.placement_key(scope, call);
            let Some(frame) = self.placement_frame(scope, demand.child, key, self.design.expr_node(call).span) else { continue };
            let made = self.frame_node(frame, item);
            if made != Some(Terminal::Node(slot)) {
                self.unsupported(scope.file, self.design.expr_node(call).span, "a node of a child read by its parent, in a child that places it more than once");
            }
            slot += 1;
        }
    }

    /// The node `bank.c0.sn` names, as a reached node of the body: the node `bank`'s body reaches into its child `c0`, itself
    /// reached from `bank`. `None` when `expr` is not a read of an item of a child of a child; `Some(None)` after a diagnostic.
    pub(super) fn nested_terminal(&mut self, scope: &Scope, expr: ExprId) -> Option<Option<Terminal>> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Field { expr: receiver, name, .. } = &node.kind else { return None };
        let (module, child) = self.child_of_module(*receiver)?;
        let mut owner = *receiver;
        loop {
            match &self.design.expr_node(owner).kind {
                ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => owner = *inner,
                _ => break,
            }
        }
        let ExprKind::Field { expr: bank, .. } = self.design.expr_node(owner).kind.clone() else { return None };
        Some(self.reach_grandchild(scope, bank, module, child, &name.text, node.span))
    }

    /// The reached node of the body for `item` of the child `child` of the module `module`, which the child `bank` of the body places.
    fn reach_grandchild(&mut self, scope: &Scope, bank: ExprId, module: DeclId, child: DeclId, item: &str, span: crate::msl::Span) -> Option<Terminal> {
        let (binding, key) = self.receiver_placement(scope, bank)?;
        let frame = self.placement_frame(scope, binding, key, span)?;
        if let Some(terminal) = self.nested_nodes.get(&(frame, child, item.to_string())) {
            return Some(*terminal);
        }
        let demand = NestedDemand { child, item: item.to_string() };
        let Some(concrete) = self.concrete_module_of(frame, &demand) else {
            self.unsupported(scope.file, span, "an item of a child whose module is not known");
            return None;
        };
        let module_name = self.design.decl_data(concrete).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(decl) = self.item_named(concrete, item) else {
            self.report(scope.file, Diagnostic::new("E0343", format!("no item `{item}` in module `{module_name}`"), span, "no such item"));
            return None;
        };
        if !self.is_visible_item(decl) {
            self.report(scope.file, Diagnostic::new("E0343", format!("`{item}` is not `pub` in module `{module_name}`"), span, "not `pub`"));
            return None;
        }
        if self.met_demand(frame, &demand) != Some(decl) {
            self.unsupported(scope.file, span, "an item of a child's child that is not one of its nodes");
            return None;
        }
        let terminal = self.add_nested_node(frame, module, child, decl, item)?;
        self.nested_nodes.insert((frame, child, item.to_string()), terminal);
        Some(terminal)
    }

    /// The module the placement bound to `demand.child` places in the variant of `frame`.
    fn concrete_module_of(&self, frame: usize, demand: &NestedDemand) -> Option<DeclId> {
        let value = self.body_let_value(demand.child)?;
        self.placed_module(self.peel_parens(value), &self.frame_bindings(frame))
    }

    /// The node of the body that stands for the demanded node of the child of `frame`: its slot in the child's body is the
    /// first slot after the child's own nodes, plus the demands of the module that its variant meets before this one.
    fn add_nested_node(&mut self, frame: usize, module: DeclId, child: DeclId, decl: DeclId, item: &str) -> Option<Terminal> {
        let bank = self.frames[frame].child?;
        let own = self.own_node_total(frame)?;
        let demands = self.demands_of(module);
        let position = demands.iter().position(|d| d.child == child && d.item == item)?;
        let before = demands[..position].iter().filter(|d| self.met_demand(frame, d).is_some()).count() as u32;
        let ty = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return None,
        };
        let nature = self.nature_decl_of(ty).and_then(|n| self.nature(n))?;
        let name = format!("{}{}.{item}", self.frames[frame].prefix, self.design.decl_data(child).name.as_ref().map_or("", |n| n.text.as_str()));
        let origin = self.locator.decl(decl);
        let spec = BodyNode { name: name.clone(), nature, role: NodeRole::Reached, init: None, origin: origin.clone() };
        let plan = NodePlan { name, role: NodeRole::Reached, nature, init: None, origin };
        self.structure.nodes.push((spec, plan));
        let node = self.structure.nodes.len() as u32 - 1;
        self.structure.reaches.push(Reach { child: bank, item: ReachedItem::Node { node, child_node: own + before } });
        Some(Terminal::Node(node))
    }
}
