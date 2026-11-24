//! `inst.name` (part III §8.2.8): what a body reads of a `pub` item of one of its children.
//!
//! A child that is placed (not hoisted) keeps its own body, so the parent names the child's
//! item through a *reach frame*: a [`Frame`] of the child's module whose names resolve to items
//! of the parent body that stand for the child's own (§8.2.25: a `pub let` is evaluated in the
//! reader's context, so its names are read through that frame).
//!
//! * A reached node is a node of the parent's body (`NodeRole::Reached`), a reached carrier a
//!   carrier of it (`CarrierRole::Reached`); each is listed in `BodyData::reaches` and bound by
//!   placement to the child instance's own once the child is placed (`place.rs`).
//! * A `pub` param reads the value the placement gives it: the param is a key param
//!   (`structural.rs`), so the body variant fixes it, and the frame holds that constant.
//! * A `pub let` is lowered in the reach frame.
//!
//! A hoisted child (a module with a value input) has no body of its own: its frame already holds
//! the body's own nodes and carriers, and is used as it is.

use piperine_ir::{BodyNode, Carrier, CarrierRole, ExprId as LoweredExpr, NodeRole, Origin, Reach, ReachedItem, Terminal, ValueType};

use super::body::NodePlan;
use super::carriers::Frame;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};
use crate::types::Callee;

impl Elaboration<'_, '_> {
    /// The index each placement that is not hoisted takes in the body's children (`Child` order,
    /// the order `lower_children` follows), and a clean table of reach frames, for the body of
    /// `module` about to be lowered.
    pub(super) fn index_children(&mut self, module: DeclId) {
        self.child_indices.clear();
        self.reach_frames.clear();
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return };
        let mut next = 0;
        for call in data.instances.clone() {
            if matches!(self.types.callee(call), Some(Callee::Module(_))) && self.hoisted_callee(call).is_none() {
                self.child_indices.insert(call, next);
                // a placement in generation is one child per iteration
                next += self.generated.of(self.design.expr_node(call).owner).map_or(1, |iterations| iterations.len() as u32);
            }
        }
    }

    /// The node a field read `c.mid` names, as a terminal of the body.
    pub(super) fn reached_terminal(&mut self, scope: &Scope, expr: ExprId) -> Option<Terminal> {
        let (frame, item) = self.reached_item(scope, expr)?;
        let terminal = self.frame_node(frame, item);
        if terminal.is_none() {
            let span = self.design.expr_node(expr).span;
            self.unsupported(scope.file, span, "a name of a child that is not one of its nodes, as a node");
        }
        terminal
    }

    /// The carrier a field read `c.state` names, as a carrier of the body.
    pub(super) fn reached_carrier(&mut self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let (frame, item) = self.reached_item(scope, expr)?;
        self.frame_carrier(frame, item)
    }

    /// A field read `c.x` as a value: the carrier, the param or the `let` of the child it names.
    pub(super) fn lower_field(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        let (frame, item) = self.reached_item(scope, expr)?;
        match self.design.decl_data(item).kind.clone() {
            DeclKind::Param(_) => match self.frames[frame].params.get(&item).copied() {
                Some(value) => Some(self.constant_expr(value, ty, origin)),
                None => {
                    self.unsupported(scope.file, span, "a param of a child that no placement value fixes");
                    None
                }
            },
            DeclKind::Port(_) | DeclKind::Signal(_) => match self.frame_carrier(frame, item) {
                Some(index) => Some(self.carrier_read(index, origin)),
                None => {
                    self.unsupported(scope.file, span, "a node of a child read as a value");
                    None
                }
            },
            _ => {
                let value = self.body_let_value(item)?;
                let entry = self.enter_frame(scope, frame);
                let lowered = self.lower_body_let(scope, value);
                self.leave_frame(scope, entry);
                lowered
            }
        }
    }

    /// The frame of the child a field read `c.x` names and the item `x` of it. `None` after a
    /// diagnostic.
    pub(super) fn reached_item(&mut self, scope: &Scope, expr: ExprId) -> Option<(usize, DeclId)> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Field { expr: receiver, res: Resolution::Decl(item), .. } = &node.kind else {
            self.unsupported(scope.file, node.span, "a field access that is not an item of a placed module");
            return None;
        };
        let frame = self.child_frame(scope, *receiver)?;
        Some((frame, *item))
    }

    /// The frame that holds the names of the child `receiver` is: a name bound by a body `let` to
    /// a placement. A hoisted placement has its frame already; any other gets a reach frame, made
    /// the first time it is read.
    fn child_frame(&mut self, scope: &Scope, receiver: ExprId) -> Option<usize> {
        let span = self.design.expr_node(receiver).span;
        let Some((binding, call)) = self.placement_bound_to(receiver) else {
            self.unsupported(scope.file, span, "a field of something that is not a name bound to a placement");
            return None;
        };
        if let Some(frame) = self.frames[scope.frame].placements.get(&call) {
            return Some(*frame);
        }
        if let Some(frame) = self.reach_frames.get(&(scope.frame, call)) {
            return Some(*frame);
        }
        let (Some(Callee::Module(module)), Some(child), 0) = (self.types.callee(call), self.child_indices.get(&call).copied(), scope.frame) else {
            self.unsupported(scope.file, span, "a field of a child of a child");
            return None;
        };
        let name = self.design.decl_data(binding).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let fields = self.placement_fields(call, module);
        let call_span = self.design.expr_node(call).span;
        let values = self.child_key_values(scope, module, &fields, scope.file, call_span)?;
        let mut frame = Frame::root(module);
        frame.parent = Some(scope.frame);
        frame.prefix = format!("{}{name}.", self.frames[scope.frame].prefix);
        frame.child = Some(child);
        for key in self.keys.of(module) {
            if let Some(value) = values.get(*key) {
                frame.params.insert(*key, value);
            }
        }
        // an instance of a generic module reads its names under its own arguments
        if self.is_generic_module(module) {
            let instance = self.module_instance(call, module)?;
            frame.substitution = self.instance_substitution(instance.item, &instance.arguments);
        }
        self.frames.push(frame);
        let index = self.frames.len() - 1;
        self.reach_frames.insert((scope.frame, call), index);
        Some(index)
    }

    /// The nodes of the array of nodes `decl` of the reach frame `frame`, made the first time: one node of the body for each
    /// element, named by the child's path, each bound to the child's own once it is placed. `false` when `decl` is no array
    /// of nodes of a child (a diagnostic was reported when its length does not fold).
    pub(super) fn reach_node_array(&mut self, frame: usize, decl: DeclId) -> bool {
        if self.frames[frame].node_shapes.contains_key(&decl) {
            return true;
        }
        let Some(child) = self.frames[frame].child else { return false };
        let outer = self.consts.set_substitution(self.frames[frame].substitution.clone());
        let found = self.consts.typed_node_array(decl).map(|array| (self.declared_shape(&array, frame, decl, "nodes"), self.own_node_offset(frame, decl), array));
        self.consts.set_substitution(outer);
        let Some((Some(shape), Some(offset), array)) = found else { return false };
        let crate::types::Ty::Node(nature) = array.innermost().clone() else { return false };
        let Some(nature) = self.nature(nature) else { return false };
        let name = self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let first = self.structure.nodes.len() as u32;
        for flat in 0..shape.count() {
            let element = format!("{}{name}{}", self.frames[frame].prefix, shape.subscripts(flat));
            let origin = self.locator.decl(decl);
            let spec = BodyNode { name: element.clone(), nature, role: NodeRole::Reached, init: None, origin: origin.clone() };
            let plan = NodePlan { name: element, role: NodeRole::Reached, nature, init: None, origin };
            self.structure.nodes.push((spec, plan));
            self.structure.reaches.push(Reach { child, item: ReachedItem::Node { node: first + flat as u32, child_node: offset + flat as u32 } });
        }
        self.frames[frame].nodes.insert(decl, Terminal::Node(first));
        self.frames[frame].node_shapes.insert(decl, shape);
        true
    }

    /// The index the first node of `decl`, a node or an array of nodes of the module of the reach frame `frame`, takes among the
    /// nodes of the child's body: the nodes its node ports and node signals have before it, an array counting each element.
    fn own_node_offset(&mut self, frame: usize, decl: DeclId) -> Option<u32> {
        let module = self.frames[frame].module;
        let children = self.design.decl_data(module).children.clone();
        let is = |this: &Self, c: &DeclId, port: bool| match this.design.decl_data(*c).kind {
            DeclKind::Port(_) => port && this.types.carrier(*c).is_none_or(|_| this.types.node_array(*c).is_some()),
            DeclKind::Signal(_) => !port && this.types.carrier(*c).is_none_or(|_| this.types.node_array(*c).is_some()),
            _ => false,
        };
        let ordered: Vec<DeclId> = children.iter().filter(|c| is(self, c, true)).chain(children.iter().filter(|c| is(self, c, false))).copied().collect();
        let mut offset = 0u32;
        for earlier in ordered {
            if earlier == decl {
                return Some(offset);
            }
            offset += match self.consts.typed_node_array(earlier) {
                Some(array) => self.shape_in(&array, frame)?.count() as u32,
                None => 1,
            };
        }
        None
    }

    /// The `let` binding `receiver` names and the placement call that is its whole value.
    fn placement_bound_to(&self, receiver: ExprId) -> Option<(DeclId, ExprId)> {
        let ExprKind::Path(path) = &self.design.expr_node(self.peel_parens(receiver)).kind else { return None };
        let Some(Resolution::Decl(binding)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
        let value = self.body_let_value(binding)?;
        let call = self.peel_parens(value);
        matches!(self.types.callee(call), Some(Callee::Module(_))).then_some((binding, call))
    }

    /// The terminal of the body that the node `decl` of the frame `frame` is: the frame's own, or,
    /// in a reach frame, the node of the body that stands for the child's (made the first time).
    pub(super) fn frame_node(&mut self, frame: usize, decl: DeclId) -> Option<Terminal> {
        if let Some(terminal) = self.frames.get(frame)?.nodes.get(&decl) {
            return Some(*terminal);
        }
        let child = self.frames[frame].child?;
        let data = self.design.decl_data(decl).clone();
        let ty = match &data.kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return None,
        };
        if self.types.carrier(decl).is_some() {
            return None;
        }
        let nature = self.nature_decl_of(ty).and_then(|n| self.nature(n))?;
        let name = format!("{}{}", self.frames[frame].prefix, data.name.as_ref().map_or("", |n| n.text.as_str()));
        let origin = self.locator.decl(decl);
        let spec = BodyNode { name: name.clone(), nature, role: NodeRole::Reached, init: None, origin: origin.clone() };
        let plan = NodePlan { name, role: NodeRole::Reached, nature, init: None, origin };
        self.structure.nodes.push((spec, plan));
        let node = self.structure.nodes.len() as u32 - 1;
        let child_node = self.own_node_slot(self.frames[frame].module, decl)?;
        self.structure.reaches.push(Reach { child, item: ReachedItem::Node { node, child_node } });
        let terminal = Terminal::Node(node);
        self.frames[frame].nodes.insert(decl, terminal);
        Some(terminal)
    }

    /// The carrier of the body that the carrier `decl` of the frame `frame` is: the frame's own,
    /// or, in a reach frame, the carrier of the body that stands for the child's (made the first
    /// time).
    pub(super) fn frame_carrier(&mut self, frame: usize, decl: DeclId) -> Option<u32> {
        if let Some(index) = self.frames.get(frame)?.carriers.get(&decl) {
            return Some(*index);
        }
        let child = self.frames[frame].child?;
        self.types.carrier(decl)?;
        let ty = self.carried_type(decl)?;
        let name = format!("{}{}", self.frames[frame].prefix, self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()));
        let carrier = Carrier { name, ty, role: CarrierRole::Reached, init: None, origin: self.locator.decl(decl), count: None };
        let index = self.add_carrier(carrier);
        let child_carrier = self.own_carrier_slot(self.frames[frame].module, decl)?;
        self.structure.reaches.push(Reach { child, item: ReachedItem::Carrier { carrier: index, child_carrier } });
        self.frames[frame].carriers.insert(decl, index);
        Some(index)
    }

    /// The index the node `decl` of `module` takes among the nodes of the module's body: its node
    /// ports, then its node signals, in declaration order (`lower_nodes`).
    fn own_node_slot(&self, module: DeclId, decl: DeclId) -> Option<u32> {
        let children = &self.design.decl_data(module).children;
        let is = |c: &DeclId, port: bool| match self.design.decl_data(*c).kind {
            DeclKind::Port(_) => port && self.types.carrier(*c).is_none(),
            DeclKind::Signal(_) => !port && self.types.carrier(*c).is_none(),
            _ => false,
        };
        children.iter().filter(|c| is(c, true)).chain(children.iter().filter(|c| is(c, false))).position(|c| *c == decl).map(|p| p as u32)
    }

    /// The index the carrier `decl` of `module` takes among the carriers of the module's body: its
    /// carrier ports, then its carrier signals (`lower_carriers`).
    fn own_carrier_slot(&self, module: DeclId, decl: DeclId) -> Option<u32> {
        let children = &self.design.decl_data(module).children;
        let is = |c: &DeclId, port: bool| match self.design.decl_data(*c).kind {
            DeclKind::Port(_) => port && self.types.carrier(*c).is_some(),
            DeclKind::Signal(_) => !port && self.types.carrier(*c).is_some(),
            _ => false,
        };
        children.iter().filter(|c| is(c, true)).chain(children.iter().filter(|c| is(c, false))).position(|c| *c == decl).map(|p| p as u32)
    }

    /// The params of other modules that a field read `c.e` of a `pub let` makes key params: the
    /// ones the `let`'s value reads, which the reach frame must hold as constants.
    pub(super) fn params_read_through_reaches(&self) -> Vec<(DeclId, DeclId)> {
        let mut found = Vec::new();
        for index in 0..self.design.expr_count() {
            let expr = ExprId(index as u32);
            if self.typed(expr).is_none() {
                continue;
            }
            let ExprKind::Field { res: Resolution::Decl(item), .. } = &self.design.expr_node(expr).kind else { continue };
            let Some(module) = self.design.decl_data(*item).parent.and_then(|p| self.module_around(p)) else { continue };
            match &self.design.decl_data(*item).kind {
                DeclKind::Param(_) => found.push((module, *item)),
                DeclKind::Binding(_) => {
                    if let Some(params) = self.body_let_value(*item).and_then(|value| super::structural::ParamReads::new(self.design, self.types).of(value)) {
                        found.extend(params.into_iter().map(|p| (module, p)));
                    }
                }
                _ => {}
            }
        }
        found
    }

    /// The module that `decl` is in, or is.
    fn module_around(&self, mut decl: DeclId) -> Option<DeclId> {
        loop {
            if matches!(self.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some(decl);
            }
            decl = self.design.decl_data(decl).parent?;
        }
    }
}
