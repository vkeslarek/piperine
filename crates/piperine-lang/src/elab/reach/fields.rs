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

use crate::refusal::forms::ValueForm;
use crate::elab::body::NodePlan;
use crate::elab::consts::Bindings;
use crate::elab::carriers::Frame;
use crate::elab::hoist::PlacementKey;
use crate::elab::element_commits::PlaceTarget;
use crate::elab::elaborator::Elaboration;
use crate::elab::lower::Scope;
use super::placement_params::{PlacementParams, PlacementSite};
use super::demands::ReachedKind;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// The index each placement that is not hoisted takes in the body's children (`Child` order,
    /// the order `lower_children` follows), and a clean table of reach frames, for the body of
    /// `module` about to be lowered.
    pub(in crate::elab) fn index_children(&mut self, module: DeclId) {
        self.state.structure.child_indices.clear();
        self.state.structure.reach_frames.clear();
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return };
        let mut next = 0;
        for call in data.instances.clone() {
            if self.placed_module(call, &self.state.structure.body_keys).is_some() && self.hoisted_callee(call).is_none() {
                self.state.structure.child_indices.insert(call, next);
                // a placement in generation is one child per iteration
                next += self.state.structure.generated.of(self.state.design.expr_node(call).owner).map_or(1, |iterations| iterations.len() as u32);
            }
        }
    }

    /// The node a field read `c.mid` names, as a terminal of the body.
    pub(in crate::elab) fn reached_terminal(&mut self, scope: &Scope, expr: ExprId) -> Option<Terminal> {
        let (frame, item) = self.reached_item(scope, expr)?;
        let terminal = self.frame_node(frame, item);
        if terminal.is_none() {
            let span = self.state.design.expr_node(expr).span;
            // a module that declares a node in generation was refused by its count (`declares_in_generation`)
            self.refuse_unless_refused(frame, scope.values.file, span, ValueForm::NameOfChildThatIsNotOne);
        }
        terminal
    }

    /// The carrier a field read `c.state` names, as a carrier of the body.
    pub(in crate::elab) fn reached_carrier(&mut self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let (frame, item) = self.reached_item(scope, expr)?;
        self.frame_carrier(frame, item)
    }

    /// A field read `c.x` as a value: the carrier, the param or the `let` of the child it names.
    pub(in crate::elab) fn lower_field(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        let (frame, item) = self.reached_item(scope, expr)?;
        let substitution = self.state.structure.frames[frame].substitution.clone();
        self.read_has_its_type(scope, expr, self.state.structure.frames[frame].module, item, &substitution)?;
        match self.state.design.decl_data(item).kind.clone() {
            DeclKind::Param(_) => match self.state.structure.frames[frame].params.get(&item).copied() {
                Some(value) => Some(self.constant_expr(value, ty, origin)),
                // a param whose given value does not fold is read in place (`reach/placement_params.rs`)
                None if self.state.structure.frames[frame].deferred_params.contains_key(&item) => {
                    let entry = self.enter_frame(scope, frame);
                    let lowered = self.lower_deferred_param(scope, item);
                    self.leave_frame(scope, entry);
                    lowered
                }
                None => {
                    self.refuse(scope.values.file, span, ValueForm::ParamOfChildThatNoPlacementValue);
                    None
                }
            },
            DeclKind::Port(_) | DeclKind::Signal(_) => match self.frame_carrier(frame, item) {
                Some(index) => Some(self.carrier_read(index, origin)),
                None => {
                    self.refuse(scope.values.file, span, ValueForm::NodeOfChildReadAsValue);
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

    /// A read through a template (`c0.cs`, §8.4.8) was given its type by its context (`Types::template_item`): the concrete item
    /// `item` of `module`, its type under `substitution`, has it, or the read is `E0180` naming both, an error of this
    /// instantiation (§3 rule 9).
    pub(in crate::elab) fn read_has_its_type(&mut self, scope: &Scope, expr: ExprId, module: DeclId, item: DeclId, substitution: &crate::types::Substitution) -> Option<()> {
        let Some(wanted) = self.state.types.template_item(expr).cloned() else { return Some(()) };
        let declared = match &self.state.design.decl_data(item).kind {
            DeclKind::Param(param) => self.typed_written(param.ty),
            DeclKind::Port(_) | DeclKind::Signal(_) => match self.state.types.carrier(item) {
                Some(crate::types::Ty::Carrier { value, .. }) => Some((**value).clone()),
                _ => None,
            },
            _ => self.typed_local(item),
        };
        let found = declared.map(|ty| substitution.apply(&ty));
        if found.as_ref() == Some(&wanted) {
            return Some(());
        }
        let name = self.state.design.decl_data(item).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let module_name = self.state.design.decl_data(module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let found_name = found.map_or_else(|| "a node".to_string(), |ty| format!("`{}`", ty.name(self.state.design)));
        let message = format!("mismatched types: the item `{name}` of module `{module_name}` is {found_name}, and the read wants `{}` (§8.4.8)", wanted.name(self.state.design));
        let span = self.state.design.expr_node(expr).span;
        self.report(scope.values.file, crate::msl::Diagnostic::new("E0180", message, span, "the concrete item has another type"));
        None
    }

    /// The frame of the child a field read `c.x` names and the item `x` of it. `None` after a
    /// diagnostic.
    pub(in crate::elab) fn reached_item(&mut self, scope: &Scope, expr: ExprId) -> Option<(usize, DeclId)> {
        // a field of a bundle port or signal is the carrier or node its leaf stands for, in this module's own frame (§7 rule 12)
        if let Some(place) = self.bundle_leaf(scope, expr) {
            // a field of an element of an array of bundles is one element of its leaf: the element forms read it (`elements.rs`)
            if !place.elements.is_empty() {
                self.refuse(scope.values.file, self.state.design.expr_node(expr).span, ValueForm::FieldOfElementOfArrayOfBundles);
                return None;
            }
            return Some((scope.values.frame, place.decl));
        }
        let node = self.state.design.expr_node(expr).clone();
        let ExprKind::Field { expr: receiver, name, res } = &node.kind else {
            self.refuse(scope.values.file, node.span, ValueForm::FieldAccessThatIsNotItemOf);
            return None;
        };
        let frame = self.child_frame(scope, *receiver)?;
        let item = match res {
            Resolution::Decl(item) => *item,
            // a child placed through a callable param (`let c0 = cell(a)`, `cell: C`): the module is the folded template's (§8.4.8)
            _ => self.visible_item_of(scope, self.state.structure.frames[frame].module, &name.text, node.span)?,
        };
        Some((frame, item))
    }

    /// The frame that holds the names of the child `receiver` is: a name bound by a body `let` to a placement, or an item of a
    /// child that is one, at any depth (`p.mid.c0`, §8.2.8). A hoisted placement has its frame already; any other gets a reach frame,
    /// made the first time it is read.
    fn child_frame(&mut self, scope: &Scope, receiver: ExprId) -> Option<usize> {
        let span = self.state.design.expr_node(receiver).span;
        let mut at = self.peel_parens(receiver);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr, index } = self.state.design.expr_node(at).kind.clone() {
            indexes.push(index);
            at = self.peel_parens(expr);
        }
        indexes.reverse();
        let ExprKind::Field { expr: inner, name, res } = self.state.design.expr_node(at).kind.clone() else {
            let (binding, key) = self.receiver_placement(scope, receiver)?;
            if key.iteration.is_empty() {
                return self.placement_frame(scope, binding, key, span);
            }
            // a child of the body placed in generation is made as a child of a child is (`frame_of_iteration`): its placement's
            // values read the loop variables of the iteration it names
            let owner = self.state.design.expr_node(key.call).owner;
            let position = self.iteration_position(owner, &key.iteration)?;
            let iteration = self.state.structure.generated.of(owner)?.get(position)?.clone();
            return self.frame_of_iteration(scope.values.frame, binding, Some(&iteration), span);
        };
        let parent = self.child_frame(scope, inner)?;
        let binding = match res {
            Resolution::Decl(binding) => binding,
            _ => self.visible_item_of(scope, self.state.structure.frames[parent].module, &name.text, span)?,
        };
        self.nested_child_frame(scope, parent, binding, &indexes, span)
    }

    /// The reach frame of the placement the `let` binding `binding` of the module of the reach frame `parent` is, the iteration of its
    /// generation named by `indexes` (one for each `for` around it). `None` after a diagnostic.
    fn nested_child_frame(&mut self, scope: &Scope, parent: usize, binding: DeclId, indexes: &[ExprId], span: crate::msl::Span) -> Option<usize> {
        let Some(call) = self.body_let_value(binding).map(|value| self.peel_parens(value)) else {
            self.refuse(scope.values.file, span, ValueForm::FieldOfSomethingThatIsNotName);
            return None;
        };
        let owner = self.state.design.expr_node(call).owner;
        let loops = self.generation_loops(owner).len();
        if indexes.len() != loops {
            self.refuse(scope.values.file, span, ValueForm::ItemOfChildPlacedInGenerationRead);
            return None;
        }
        let ordinals = self.fold_ordinals(scope, indexes)?;
        if loops == 0 {
            return self.frame_of_iteration(parent, binding, None, span);
        }
        let iterations = self.iterations_under(parent, owner);
        let held: Vec<Vec<u64>> = iterations.iter().map(|iteration| self.iteration_ordinals(owner, iteration)).collect();
        let Some(at) = held.iter().position(|each| *each == ordinals) else {
            self.refer_outside_generation(scope.values.file, span, binding, Some(Self::outermost_missing_ordinal(&held, &ordinals)));
            return None;
        };
        self.frame_of_iteration(parent, binding, Some(&iterations[at]), span)
    }

    /// The reach frame of the placement `key` bound by the `let` binding `binding`, made the first time: a child of the body when the
    /// frame `scope` reads is the body's own (frame 0), else a child of the child that frame is, below it.
    pub(in crate::elab) fn placement_frame(&mut self, scope: &Scope, binding: DeclId, key: PlacementKey, span: crate::msl::Span) -> Option<usize> {
        if let Some(frame) = self.state.structure.frames[scope.values.frame].placements.get(&key) {
            return Some(*frame);
        }
        if let Some(frame) = self.state.structure.reach_frames.get(&(scope.values.frame, key.clone())) {
            return Some(*frame);
        }
        // a placement of a reach frame is folded under that frame's generic arguments, not the body's (`GBlock<C>` placing `GBank<C>`)
        let outer = (scope.values.frame != 0).then(|| self.state.consts.set_substitution(self.state.structure.frames[scope.values.frame].substitution.clone()));
        let made = self.make_placement_frame(scope, binding, key, span);
        if let Some(outer) = outer {
            self.state.consts.set_substitution(outer);
        }
        made
    }

    /// The reach frame [`placement_frame`](Self::placement_frame) makes for the placement `key`, folded under the substitution of the
    /// frame `scope` reads.
    fn make_placement_frame(&mut self, scope: &Scope, binding: DeclId, key: PlacementKey, span: crate::msl::Span) -> Option<usize> {
        let call = key.call;
        // a frame below a reach frame reads the params the placement bound to it; a hoisted frame has no body of its own to reach into.
        // A hoisted module that holds a placement was refused when it was hoisted (`hoist.rs`), so a read through it adds no second
        // diagnostic for the same cause
        let (keys, child) = if scope.values.frame == 0 {
            (scope.values.keys.clone(), self.child_index(&key))
        } else if self.state.structure.frames[scope.values.frame].is_reached() {
            (self.frame_bindings(scope.values.frame), None)
        } else {
            self.refuse_unless_refused(scope.values.frame, scope.values.file, span, ValueForm::FieldOfChildOfChild);
            return None;
        };
        let Some(module) = self.placed_module(call, &keys) else {
            self.refuse_unless_refused(scope.values.frame, scope.values.file, span, ValueForm::FieldOfChildOfChild);
            return None;
        };
        if scope.values.frame == 0 && child.is_none() {
            // a placement of the body with no child of its own is a hoisted one: refused when it was hoisted, if it holds a placement
            if !self.state.structure.refused_modules.contains(&module) {
                self.refuse_unless_refused(scope.values.frame, scope.values.file, span, ValueForm::FieldOfChildOfChild);
            }
            return None;
        }
        let name = format!("{}{}", self.state.design.decl_data(binding).name.as_ref().map_or_else(String::new, |n| n.text.clone()), key.subscripts());
        let nested = (scope.values.frame != 0).then(|| self.reading_scope(scope, &keys));
        let within = nested.as_ref().unwrap_or(scope);
        let values = self.placement_key_values(within, call, module, self.state.design.expr_node(call).span)?;
        let mut frame = Frame::root(module);
        frame.parent = Some(scope.values.frame);
        frame.prefix = format!("{}{name}.", self.state.structure.frames[scope.values.frame].prefix);
        frame.child = child;
        if child.is_none() {
            frame.placement = Some((binding, key.iteration.clone()));
        }
        // an instance of a generic module reads its names under its own arguments
        if self.is_generic_module(module) {
            let instance = self.placement_instance(within, call, module)?;
            frame.substitution = self.instance_substitution(instance.item, &instance.arguments);
        }
        // every param of the module is the value the placement (or its template) gives, else its default: what a `let` of the
        // child read in the parent's context reads (§8.2.25); one that does not fold is read in place, and what is wrong with the
        // placement is the placement's own diagnostic, not the frame's. A key param is the value that selects the child's body variant
        let folded = self.reach_frame_params(within, call, module, &frame.substitution)?;
        frame.params = folded.values;
        frame.deferred_params = folded.deferred;
        // a value deferred to the caller reads the loop variables and the iteration the placement sits in
        frame.unrolled = within.values.unrolled.clone();
        frame.ordinals = within.digital.ordinals.clone();
        for key in self.state.structure.keys.of(module) {
            if let Some(value) = values.get(*key) {
                frame.params.insert(*key, value);
            }
        }
        self.state.structure.frames.push(frame);
        let index = self.state.structure.frames.len() - 1;
        self.state.structure.reach_frames.insert((scope.values.frame, key), index);
        Some(index)
    }

    /// The index among the body's children of the child the placement `key` makes: the first of its iterations, in generation.
    fn child_index(&self, key: &PlacementKey) -> Option<u32> {
        let first = self.state.structure.child_indices.get(&key.call).copied()?;
        // a placement in generation is one child per iteration, in iteration order (`lower_children`)
        let owner = self.state.design.expr_node(key.call).owner;
        match self.state.structure.generated.of(owner) {
            Some(_) => Some(first + self.iteration_position(owner, &key.iteration)? as u32),
            None => Some(first),
        }
    }

    /// A scope that reads the names of the frame `scope` reads, with its params `keys`: where the arguments of a placement below that
    /// frame are folded.
    fn reading_scope(&self, scope: &Scope, keys: &Bindings) -> Scope {
        let mut within = Scope::new(scope.values.file);
        within.values.frame = scope.values.frame;
        within.values.keys = keys.clone();
        within.values.unrolled = scope.values.unrolled.clone();
        within.digital.ordinals = scope.digital.ordinals.clone();
        within
    }

    /// The params of `module` as the placement `call`, read from `scope`, binds them (`reach/placement_params.rs`), reporting nothing: a
    /// given value folds over the caller's constants (every param of the reading frame that folds, its loop variables) and generic
    /// arguments, a default over the params before it under the instance's own (`substitution`). `None` when the placement is
    /// written through a template that does not fold.
    fn reach_frame_params(&mut self, scope: &Scope, call: ExprId, module: DeclId, substitution: &crate::types::Substitution) -> Option<PlacementParams> {
        let ExprKind::Call { class, .. } = self.state.design.expr_node(call).kind.clone() else { return None };
        let Ok(through) = self.template_through(scope, call, &class) else { return None };
        let fields = self.placement_fields(call, module);
        let caller = scope.values.keys.overlaid(&scope.values.unrolled);
        let outer = self.state.consts.set_substitution(substitution.clone());
        let site = PlacementSite { fields: &fields, template: through.as_ref(), caller: &caller, caller_substitution: &outer };
        let folded = self.fold_placement_params(module, &site);
        self.state.consts.set_substitution(outer);
        Some(folded)
    }

    /// The nodes of the array of nodes `decl` of the reach frame `frame`, made the first time: one node of the body for each
    /// element, named by the child's path, each bound to the child's own once it is placed. `false` when `decl` is no array
    /// of nodes of a child (a diagnostic was reported when its length does not fold).
    pub(in crate::elab) fn reach_node_array(&mut self, frame: usize, decl: DeclId) -> bool {
        if self.state.structure.frames[frame].node_shapes.contains_key(&decl) {
            return true;
        }
        let Some(child) = self.state.structure.frames[frame].child else { return false };
        let outer = self.state.consts.set_substitution(self.state.structure.frames[frame].substitution.clone());
        let found = self.state.consts.typed_node_array(decl).map(|array| (self.declared_shape(&array, frame, decl, "nodes"), self.own_node_offset(frame, decl), array));
        self.state.consts.set_substitution(outer);
        let Some((Some(shape), Some(offset), array)) = found else { return false };
        let crate::types::Ty::Node(nature) = array.innermost().clone() else { return false };
        let Some(nature) = self.nature(nature) else { return false };
        let name = self.state.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let first = self.state.digital.structure.nodes.len() as u32;
        for flat in 0..shape.count() {
            let element = format!("{}{name}{}", self.state.structure.frames[frame].prefix, shape.subscripts(flat));
            let origin = self.state.locator.decl(decl);
            let spec = BodyNode { name: element.clone(), nature, role: NodeRole::Reached, init: None, origin: origin.clone() };
            let plan = NodePlan { name: element, role: NodeRole::Reached, nature, init: None, origin };
            self.state.digital.structure.nodes.push((spec, plan));
            self.state.digital.structure.reaches.push(Reach { child, item: ReachedItem::Node { node: first + flat as u32, child_node: offset + flat as u32 } });
        }
        self.state.structure.frames[frame].nodes.insert(decl, Terminal::Node(first));
        self.state.structure.frames[frame].node_shapes.insert(decl, shape);
        true
    }

    /// The index the first node of `decl`, a node or an array of nodes of the module of the reach frame `frame`, takes among the
    /// nodes of the child's body: the nodes its node ports and node signals have before it, an array counting each element.
    fn own_node_offset(&mut self, frame: usize, decl: DeclId) -> Option<u32> {
        let module = self.state.structure.frames[frame].module;
        let children = self.state.design.decl_data(module).children.clone();
        let is = |this: &Self, c: &DeclId, port: bool| match this.state.design.decl_data(*c).kind {
            DeclKind::Port(_) => port && this.state.types.carrier(*c).is_none_or(|_| this.state.types.node_array(*c).is_some()),
            DeclKind::Signal(_) => !port && this.state.types.carrier(*c).is_none_or(|_| this.state.types.node_array(*c).is_some()),
            _ => false,
        };
        let ordered: Vec<DeclId> = children.iter().filter(|c| is(self, c, true)).chain(children.iter().filter(|c| is(self, c, false))).copied().collect();
        let mut offset = 0u32;
        for earlier in ordered {
            if earlier == decl {
                return Some(offset);
            }
            offset += match self.state.consts.typed_node_array(earlier) {
                Some(array) => self.shape_in(&array, frame)?.count() as u32,
                None => 1,
            };
        }
        None
    }

    /// Whether the module of the reach frame `frame` declares a node or a carrier in generation, which only the module's own
    /// lowering can count: reported, since no parent can name a slot after it.
    fn declares_in_generation(&mut self, frame: usize) -> bool {
        let module = self.state.structure.frames[frame].module;
        let data = self.state.design.decl_data(module).clone();
        let found = data.children.iter().find(|child| matches!(self.state.design.decl_data(**child).kind, DeclKind::Generation(_)) && self.declares_node(**child));
        if let Some(generation) = found {
            self.state.structure.refused_modules.insert(module);
            self.refuse(data.file, self.state.design.decl_data(*generation).span, ValueForm::NodeDeclaredInGenerationOfModuleParent);
        }
        found.is_some()
    }

    /// How many nodes the body of the module of the reach frame `frame` has of its own, before any reached node: its node ports
    /// and node signals, an array counting each element. `None` after a diagnostic when the module declares a node in generation,
    /// whose count only the module's own lowering knows.
    pub(in crate::elab) fn own_node_total(&mut self, frame: usize) -> Option<u32> {
        if self.declares_in_generation(frame) {
            return None;
        }
        let module = self.state.structure.frames[frame].module;
        let mut total = 0u32;
        for child in self.state.design.decl_data(module).children.clone() {
            match &self.state.design.decl_data(child).kind {
                DeclKind::Port(_) | DeclKind::Signal(_) if self.state.types.carrier(child).is_none() => total += 1,
                DeclKind::Port(_) | DeclKind::Signal(_) if self.state.types.node_array(child).is_some() => {
                    let array = self.state.consts.typed_node_array(child)?;
                    total += self.shape_in(&array, frame)?.count() as u32;
                }
                _ => {}
            }
        }
        Some(total)
    }

    /// How many carriers the body of the module of the reach frame `frame` has of its own, before any reached carrier: its discrete
    /// ports and signals, a bundle counting its fields, an array of carriers one, an array of none no carrier (`lower_carriers`).
    /// `None` after a diagnostic when the module declares one in generation.
    pub(in crate::elab) fn own_carrier_total(&mut self, frame: usize) -> Option<u32> {
        if self.declares_in_generation(frame) {
            return None;
        }
        let module = self.state.structure.frames[frame].module;
        let children = self.state.design.module_items(&self.state.design.decl_data(module).children.clone());
        let mut total = 0u32;
        for child in children {
            let carrier = matches!(self.state.design.decl_data(child).kind, DeclKind::Port(_) | DeclKind::Signal(_)) && self.state.types.carrier(child).is_some() && self.state.types.node_array(child).is_none();
            if carrier && !self.is_empty_array(frame, child) {
                total += 1;
            }
        }
        Some(total)
    }

    /// Whether the generation `decl` declares a port or a signal, at any depth.
    fn declares_node(&self, decl: DeclId) -> bool {
        self.state.design.decl_data(decl).children.iter().any(|child| match &self.state.design.decl_data(*child).kind {
            DeclKind::Port(_) | DeclKind::Signal(_) => true,
            DeclKind::Generation(_) => self.declares_node(*child),
            _ => false,
        })
    }

    /// The `let` binding a receiver `c` or `c[k]` names and the placement it reads, with its iteration: inside the loops of
    /// a placement written in generation the name is the iteration's instance, and each index outside them names one by
    /// its ordinal (§8.2.23). `None` after a diagnostic: a receiver that is no such name, an index that does not fold
    /// (`E0188`), an instance the generation never made (`E0342`), or a part of a nested array of instances.
    pub(in crate::elab) fn receiver_placement(&mut self, scope: &Scope, receiver: ExprId) -> Option<(DeclId, PlacementKey)> {
        let span = self.state.design.expr_node(receiver).span;
        let mut at = self.peel_parens(receiver);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr, index } = self.state.design.expr_node(at).kind.clone() {
            indexes.push(index);
            at = self.peel_parens(expr);
        }
        indexes.reverse();
        let Some((binding, call)) = self.placement_bound_to(at) else {
            self.refuse(scope.values.file, span, ValueForm::FieldOfSomethingThatIsNotName);
            return None;
        };
        let mut key = self.placement_key(scope, call);
        let bindings = self.frame_bindings(scope.values.frame).overlaid(&scope.values.unrolled);
        for index in indexes {
            match self.state.consts.value(index, &bindings) {
                Ok(piperine_ir::Constant::Int(ordinal)) if ordinal >= 0 => key.iteration.push(ordinal as u64),
                _ => {
                    self.not_folded(scope, self.state.design.expr_node(index).span, "the index of a generated instance");
                    return None;
                }
            }
        }
        let owner = self.state.design.expr_node(call).owner;
        let loops = self.generation_loops(owner).len();
        if key.iteration.len() != loops {
            self.refuse(scope.values.file, span, ValueForm::PartOfArrayOfGeneratedInstancesAs);
            return None;
        }
        if loops > 0 && self.iteration_position(owner, &key.iteration).is_none() {
            let held: Vec<Vec<u64>> = self.state.structure.generated.of(owner).map_or_else(Vec::new, |iterations| iterations.iter().map(|iteration| self.iteration_ordinals(owner, iteration)).collect());
            self.refer_outside_generation(scope.values.file, span, binding, Some(Self::outermost_missing_ordinal(&held, &key.iteration)));
            return None;
        }
        Some((binding, key))
    }

    /// The `let` binding `receiver` names and the placement call that is its whole value.
    fn placement_bound_to(&self, receiver: ExprId) -> Option<(DeclId, ExprId)> {
        let ExprKind::Path(path) = &self.state.design.expr_node(self.peel_parens(receiver)).kind else { return None };
        let Some(Resolution::Decl(binding)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
        let value = self.body_let_value(binding)?;
        let call = self.peel_parens(value);
        self.placed_module(call, &self.state.structure.body_keys).map(|_| (binding, call))
    }

    /// The terminal of the body that the node `decl` of the frame `frame` is: the frame's own, or,
    /// in a reach frame, the node of the body that stands for the child's (made the first time).
    pub(in crate::elab) fn frame_node(&mut self, frame: usize, decl: DeclId) -> Option<Terminal> {
        if let Some(terminal) = self.state.structure.frames.get(frame)?.nodes.get(&decl) {
            return Some(*terminal);
        }
        let data = self.state.design.decl_data(decl).clone();
        let ty = match &data.kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return None,
        };
        if self.state.types.carrier(decl).is_some() {
            return None;
        }
        let nature = self.nature_decl_of(ty).and_then(|n| self.nature(n))?;
        let name = format!("{}{}", self.state.structure.frames[frame].prefix, data.name.as_ref().map_or("", |n| n.text.as_str()));
        let origin = self.state.locator.decl(decl);
        let (child, child_node) = match self.state.structure.frames[frame].child {
            Some(child) => (child, self.own_node_slot(self.state.structure.frames[frame].module, decl)?),
            None => self.nested_slot(frame, decl, ReachedKind::Node)?,
        };
        let spec = BodyNode { name: name.clone(), nature, role: NodeRole::Reached, init: None, origin: origin.clone() };
        let plan = NodePlan { name, role: NodeRole::Reached, nature, init: None, origin };
        self.state.digital.structure.nodes.push((spec, plan));
        let node = self.state.digital.structure.nodes.len() as u32 - 1;
        self.state.digital.structure.reaches.push(Reach { child, item: ReachedItem::Node { node, child_node } });
        let terminal = Terminal::Node(node);
        self.state.structure.frames[frame].nodes.insert(decl, terminal);
        Some(terminal)
    }

    /// The carrier of the body that the carrier `decl` of the frame `frame` is: the frame's own,
    /// or, in a reach frame, the carrier of the body that stands for the child's (made the first
    /// time).
    pub(in crate::elab) fn frame_carrier(&mut self, frame: usize, decl: DeclId) -> Option<u32> {
        if let Some(index) = self.state.structure.frames.get(frame)?.carriers.get(&decl) {
            return Some(*index);
        }
        self.state.types.carrier(decl)?;
        let ty = self.carried_type(decl)?;
        let name = format!("{}{}", self.state.structure.frames[frame].prefix, self.state.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()));
        let (child, child_carrier) = match self.state.structure.frames[frame].child {
            Some(child) => (child, self.own_carrier_slot(self.state.structure.frames[frame].module, decl)?),
            None => self.nested_slot(frame, decl, ReachedKind::Carrier)?,
        };
        let carrier = Carrier { name, ty, role: CarrierRole::Reached, init: None, origin: self.state.locator.decl(decl), count: None };
        let index = self.add_carrier(carrier);
        self.state.digital.structure.reaches.push(Reach { child, item: ReachedItem::Carrier { carrier: index, child_carrier } });
        self.state.structure.frames[frame].carriers.insert(decl, index);
        Some(index)
    }

    /// The carrier of the body a drive `c.line = v` or a commit `c.line <- v` of the child's `pub` resolved signal `decl` writes:
    /// the reached carrier of `frame`, made a resolved carrier of the body (`CarrierRole::Resolved`) so the body holds a drive of
    /// the child's net, bound to it once the child is placed (`place.rs`). `None` after a diagnostic.
    pub(in crate::elab) fn drivable_reached_carrier(&mut self, frame: usize, decl: DeclId) -> Option<u32> {
        let index = self.frame_carrier(frame, decl)?;
        if matches!(self.state.digital.structure.carriers[index as usize].role, CarrierRole::Resolved { .. }) {
            return Some(index);
        }
        let data = self.state.design.decl_data(decl).clone();
        let Some(Ty::Carrier { nature, .. }) = self.state.consts.typed_carrier(decl) else {
            self.refuse(data.file, data.span, ValueForm::DriveOfChildCarrierThatIsNot);
            return None;
        };
        let (chosen, _) = self.chosen_resolve_impl(decl, nature)?;
        let Some(driven) = self.stored_type(&chosen.driven) else {
            self.refuse(data.file, data.span, ValueForm::ResolvedCarrierWhoseDriveTypeHasNo);
            return None;
        };
        self.state.digital.structure.carriers[index as usize].role = CarrierRole::Resolved { driven, port: false };
        self.state.digital.structure.resolved_carriers.push((index, decl, nature));
        Some(index)
    }

    /// The place `place` when it is `inst.name`, a resolved carrier of a placed child, lowered as a target of a drive: `None`
    /// when it is any other place, `Some(None)` after a diagnostic.
    pub(in crate::elab) fn reached_place_target(&mut self, scope: &Scope, place: ExprId) -> Option<Option<PlaceTarget>> {
        let node = self.state.design.expr_node(self.peel_parens(place)).clone();
        let ExprKind::Field { res: Resolution::Decl(item), .. } = &node.kind else { return None };
        if !matches!(self.state.design.decl_data(*item).kind, DeclKind::Port(_) | DeclKind::Signal(_)) || self.state.types.carrier(*item).is_none() {
            return None;
        }
        let target = self.reached_item(scope, self.peel_parens(place)).and_then(|(frame, item)| {
            let carrier = self.drivable_reached_carrier(frame, item)?;
            let CarrierRole::Resolved { driven, .. } = self.state.digital.structure.carriers[carrier as usize].role.clone() else { return None };
            Some(PlaceTarget { carrier, element: None, constant: None, path: Vec::new(), ty: driven })
        });
        Some(target)
    }

    /// The index the node `decl` of `module` takes among the nodes of the module's body: its node
    /// ports, then its node signals, in declaration order (`lower_nodes`).
    fn own_node_slot(&self, module: DeclId, decl: DeclId) -> Option<u32> {
        let children = &self.state.design.decl_data(module).children;
        let is = |c: &DeclId, port: bool| match self.state.design.decl_data(*c).kind {
            DeclKind::Port(_) => port && self.state.types.carrier(*c).is_none(),
            DeclKind::Signal(_) => !port && self.state.types.carrier(*c).is_none(),
            _ => false,
        };
        children.iter().filter(|c| is(c, true)).chain(children.iter().filter(|c| is(c, false))).position(|c| *c == decl).map(|p| p as u32)
    }

    /// The index the carrier `decl` of `module` takes among the carriers of the module's body: its
    /// carrier ports, then its carrier signals (`lower_carriers`).
    fn own_carrier_slot(&self, module: DeclId, decl: DeclId) -> Option<u32> {
        let children = &self.state.design.decl_data(module).children;
        let is = |c: &DeclId, port: bool| match self.state.design.decl_data(*c).kind {
            DeclKind::Port(_) => port && self.state.types.carrier(*c).is_some(),
            DeclKind::Signal(_) => !port && self.state.types.carrier(*c).is_some(),
            _ => false,
        };
        children.iter().filter(|c| is(c, true)).chain(children.iter().filter(|c| is(c, false))).position(|c| *c == decl).map(|p| p as u32)
    }

    /// The params of other modules that a field read `c.e` of a `pub let` makes key params: the
    /// ones the `let`'s value reads, which the reach frame must hold as constants.
    pub(in crate::elab) fn params_read_through_reaches(&self) -> Vec<(DeclId, DeclId)> {
        let mut found = Vec::new();
        for index in 0..self.state.design.expr_count() {
            let expr = ExprId(index as u32);
            if self.typed(expr).is_none() {
                continue;
            }
            let ExprKind::Field { res: Resolution::Decl(item), .. } = &self.state.design.expr_node(expr).kind else { continue };
            let Some(module) = self.state.design.decl_data(*item).parent.and_then(|p| self.module_around(p)) else { continue };
            match &self.state.design.decl_data(*item).kind {
                DeclKind::Param(_) => found.push((module, *item)),
                DeclKind::Binding(_) => {
                    if let Some(params) = self.body_let_value(*item).and_then(|value| crate::elab::structural::ParamReads::new(self.state.design, self.state.types).of(value)) {
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
            if matches!(self.state.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some(decl);
            }
            decl = self.state.design.decl_data(decl).parent?;
        }
    }
}
