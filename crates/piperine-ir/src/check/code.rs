//! The code pass of the type check: every root of code in the IR (a param's `where`, a placement's
//! param value, the `analog` block, a drive, an event, a guarded `limit`, a primitive's value input, a
//! reaction, a function body) is walked once, with the owner that bounds it, and the functions the
//! `analog` blocks reach are checked for recursion.

use std::collections::BTreeSet;

use super::walk::{CodeWalk, Owner, WalkPosition, WalkStep};
use super::{ClassCheck, TypeCheck};
use crate::ids::{ExprId, FunctionId};
use crate::model::{BodyData, CarrierRole, ChildValue, EventSource, Projection, ValueType};

impl TypeCheck<'_> {
    /// Walks the code of every body, then of every function, then checks the recursion `analog`
    /// reaches.
    pub(super) fn code(&mut self) {
        let ir = self.reader.ir;
        // The functions the `analog` blocks call, directly.
        let mut analog_calls: BTreeSet<FunctionId> = BTreeSet::new();
        for body in &ir.bodies {
            self.body_code(body, &mut analog_calls);
        }
        let mut graph: Vec<BTreeSet<FunctionId>> = Vec::new();
        for function in &ir.functions {
            let mut walk = CodeWalk::new(Owner::of_function(function), function.params.len());
            self.require_type(&walk, function.body, &function.ret);
            self.walk_from(&mut walk, WalkStep::Expression(function.body, WalkPosition::top()));
            graph.push(walk.calls);
        }
        self.recursion(&graph, &analog_calls);
    }

    /// The code of one body, position by position; the functions its `analog` block calls join
    /// `analog_calls`.
    fn body_code(&mut self, body: &BodyData, analog_calls: &mut BTreeSet<FunctionId>) {
        let ir = self.reader.ir;
        for param in &body.params {
            let Some(check) = param.check else { continue };
            let mut walk = CodeWalk::new(Owner::of_body(ir, body, false, format!("body {} param {}", body.name, param.name)), 0);
            self.require_type(&walk, check, &ValueType::Bool);
            self.walk_from(&mut walk, WalkStep::Expression(check, WalkPosition::top()));
        }
        self.child_param_values(body);
        if let Some(analog) = &body.analog {
            let mut walk = CodeWalk::new(Owner::of_body(ir, body, true, format!("body {}", body.name)), 0);
            for &stmt in analog {
                self.walk_from(&mut walk, WalkStep::Statement(stmt, WalkPosition::top(), true));
            }
            self.site_uses(&walk);
            analog_calls.extend(walk.calls);
        }
        self.drives(body);
        self.events(body);
        self.primitive_inputs(body);
        for (k, reaction) in body.reactions.iter().enumerate() {
            let mut owner = Owner::of_body(ir, body, false, format!("body {} reaction {k}", body.name));
            owner.locals = reaction.locals.iter().map(|l| l.ty.clone()).collect();
            owner.mutable = reaction.locals.iter().map(|l| l.mutable).collect();
            owner.reaction = true;
            let mut walk = CodeWalk::new(owner, 0);
            for &stmt in &reaction.stmts {
                self.walk_from(&mut walk, WalkStep::Statement(stmt, WalkPosition::top(), false));
            }
        }
    }

    /// The param values a body gives its children: each has the param's type and reads only the
    /// parent's params.
    fn child_param_values(&mut self, body: &BodyData) {
        let ir = self.reader.ir;
        for (ci, list) in body.child_params.iter().enumerate() {
            let Some(child) = body.children.get(ci) else { continue };
            let callee = ir.bodies.get(child.body.index());
            for (p, value) in list {
                let ChildValue::Expr(e) = value else { continue };
                let Some(param) = callee.and_then(|b| b.params.get(*p as usize)) else { continue };
                let item = format!("body {} child {} param {}", body.name, child.name, param.name);
                let mut walk = CodeWalk::new(Owner::of_body(ir, body, false, item.clone()), 0);
                self.require_type(&walk, *e, &param.ty);
                self.walk_from(&mut walk, WalkStep::Expression(*e, WalkPosition::top()));
                ClassCheck { reader: self.reader, findings: &mut *self.findings }.parent_reads(&item, *e);
            }
        }
    }

    /// The drives of a body: the value has the type the target writes, the delay is an `f64`.
    fn drives(&mut self, body: &BodyData) {
        let ir = self.reader.ir;
        for drive in &body.drives {
            let name = body.carriers.get(drive.carrier as usize).map_or("<bad carrier>", |c| c.name.as_str());
            let mut walk = CodeWalk::new(Owner::of_body(ir, body, false, format!("body {} drive {name}", body.name)), 0);
            if let Some(c) = body.carriers.get(drive.carrier as usize) {
                let held = match &c.role {
                    CarrierRole::Resolved { driven, .. } => driven,
                    _ => &c.ty,
                };
                if let Some(written) = self.drive_type(&walk, &drive.origin, drive.carrier, held, drive.element, &drive.path) {
                    self.carried(&walk, drive.value, &written, "the value of a drive");
                }
            }
            for step in &drive.path {
                if let Projection::Index(index) = step {
                    self.walk_from(&mut walk, WalkStep::Expression(*index, WalkPosition::top()));
                }
            }
            self.walk_from(&mut walk, WalkStep::Expression(drive.value, WalkPosition::top()));
            if let Some(after) = drive.after {
                self.carried(&walk, after, &ValueType::F64, "the delay of a drive");
                self.walk_from(&mut walk, WalkStep::Expression(after, WalkPosition::top()));
            }
        }
    }

    /// The events of a body: the expressions of its source, and its guard, a `bool`.
    fn events(&mut self, body: &BodyData) {
        let ir = self.reader.ir;
        for (k, event) in body.events.iter().enumerate() {
            let mut guarded = Owner::of_body(ir, body, false, format!("body {} event {k}", body.name));
            guarded.event = Some(k as u32);
            let mut walk = CodeWalk::new(guarded, 0);
            match event.source {
                EventSource::Cross { e, ttol, etol } => {
                    for id in [e].into_iter().chain(ttol).chain(etol) {
                        self.walk_from(&mut walk, WalkStep::Expression(id, WalkPosition::top()));
                    }
                }
                EventSource::At(e) => self.walk_from(&mut walk, WalkStep::Expression(e, WalkPosition::top())),
                EventSource::ElementChanged { carrier, element } => {
                    self.element_event(&format!("body {} event {k}", body.name), &event.origin, body, carrier, element)
                }
                EventSource::Changed(_) | EventSource::ParamChanged(_) | EventSource::Settle => {}
            }
            if let Some(guard) = event.guard {
                self.carried(&walk, guard, &ValueType::Bool, "the guard of an event");
                self.walk_from(&mut walk, WalkStep::Expression(guard, WalkPosition::top()));
            }
        }
    }

    /// What the primitives of a body evaluate on their own: a `limit`'s `enabled` (a `bool`), a
    /// derivative's state and `abstol`, a delay's input, time and maximum, a noise density (`f64`s).
    fn primitive_inputs(&mut self, body: &BodyData) {
        let ir = self.reader.ir;
        for (k, site) in body.limits.iter().enumerate() {
            let Some(enabled) = site.enabled else { continue };
            let mut walk = CodeWalk::new(Owner::of_body(ir, body, false, format!("body {} limit {k}", body.name)), 0);
            self.require_type(&walk, enabled, &ValueType::Bool);
            self.walk_from(&mut walk, WalkStep::Expression(enabled, WalkPosition::top()));
        }
        for (k, site) in body.derivatives.iter().enumerate() {
            let roots: Vec<ExprId> = [site.state].into_iter().chain(site.abstol).collect();
            self.value_input(body, format!("body {} derivative {k}", body.name), &roots);
        }
        for (k, site) in body.delays.iter().enumerate() {
            let roots: Vec<ExprId> = [site.input, site.time].into_iter().chain(site.maximum).collect();
            self.value_input(body, format!("body {} delay {k}", body.name), &roots);
        }
        for (k, source) in body.noise_sources.iter().enumerate() {
            self.value_input(body, format!("body {} noise source {k}", body.name), &[source.density]);
        }
    }

    /// The `f64` values of one primitive of `body`, named `item`, walked as one owner's code that
    /// may read an `ac_stim` site.
    fn value_input(&mut self, body: &BodyData, item: String, roots: &[ExprId]) {
        let owner = Owner { value_input: true, ..Owner::of_body(self.reader.ir, body, false, item) };
        let mut walk = CodeWalk::new(owner, 0);
        for &root in roots {
            self.require_type(&walk, root, &ValueType::F64);
            self.walk_from(&mut walk, WalkStep::Expression(root, WalkPosition::top()));
        }
    }

    /// Every function that reaches itself through calls and is reached from an `analog` body
    /// (`analog_calls`, the functions that code calls, directly). Recursion elsewhere is allowed
    /// (§3 rule 12): the elaborator unrolls the recursion `analog` reaches, since a solve needs
    /// a bounded contribution.
    fn recursion(&mut self, graph: &[BTreeSet<FunctionId>], analog_calls: &BTreeSet<FunctionId>) {
        let reaches = |from: Vec<usize>| -> BTreeSet<usize> {
            let mut seen: BTreeSet<usize> = BTreeSet::new();
            let mut stack = from;
            while let Some(f) = stack.pop() {
                if f < graph.len() && seen.insert(f) {
                    stack.extend(graph[f].iter().map(|g| g.index()));
                }
            }
            seen
        };
        let from_analog = reaches(analog_calls.iter().map(|f| f.index()).collect());
        for (i, function) in self.reader.ir.functions.iter().enumerate() {
            let recursive = reaches(graph[i].iter().map(|f| f.index()).collect()).contains(&i);
            if recursive && from_analog.contains(&i) {
                self.findings.report(
                    format!("function {}", function.name),
                    function.origin.clone(),
                    format!("function {} calls itself, directly or through other functions, and `analog` reaches it (§3 rule 12)", function.name),
                );
            }
        }
    }

    /// Each `limit` and `ac_stim` site of the walked `analog` block is used by one expression.
    fn site_uses(&mut self, walk: &CodeWalk) {
        for (kind, users) in [("limit", &walk.limit_users), ("ac_stim", &walk.stimulus_users)] {
            for (i, set) in users.iter().enumerate() {
                if set.len() > 1 {
                    self.findings.report(walk.owner.item.clone(), walk.owner.origin.clone(), format!("{kind} site {i} is used by {} expressions, once is allowed", set.len()));
                }
            }
        }
    }

    /// Runs the walk from `root` to its end.
    pub(super) fn walk_from(&mut self, walk: &mut CodeWalk, root: WalkStep) {
        let mut stack = vec![root];
        while let Some(step) = stack.pop() {
            match step {
                WalkStep::Exit(id) => {
                    walk.on_stack.remove(&id);
                }
                WalkStep::Bind(local) => {
                    if let Some(slot) = walk.bound.get_mut(local as usize) {
                        *slot = true;
                    }
                    walk.intern_scope();
                }
                WalkStep::Restore(bound) => {
                    walk.bound = bound;
                    walk.intern_scope();
                }
                WalkStep::Statement(id, position, top) => self.statement(walk, id, position, top),
                WalkStep::Expression(id, position) => self.expression(walk, id, position),
            }
            // `pending` holds the follow-up steps in evaluation order.
            stack.extend(walk.pending.drain(..).rev());
        }
    }
}
