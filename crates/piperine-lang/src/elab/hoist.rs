//! Hoisting a module with a result or a value input (part III §8.3.12, Appendix C.1.1; DIG-10,
//! AS-08, AS-09).
//!
//! A placement of a module that declares `-> T` or takes a value input gets no body and no
//! instance: its carriers, drives, events, reactions, nodes and `analog` blocks join the body of
//! the module that places it, under the placement's name (`gate.x` for `let gate = transition(..)`, else `<module>$<k>`, `k` counting
//! that module's placements in source order), and each read of it lowers the callee's result
//! expression where the read stands. The POM keeps the placement as written: hoisting is a view
//! of the IR, never a rewrite of the authored tree.
//!
//! Each placement is a [`Frame`]: the callee module, the frame of its caller, its params bound
//! to constants, and its value inputs bound to the caller's expressions. A name read in a frame
//! resolves there: a carrier to its hoisted carrier, a param to its constant, a value input to the
//! caller's expression lowered in the caller's frame (§8.3.8, §8.3.13), a node port to the
//! caller's terminal for its argument, a node `signal` to a node the body gained, a module-body
//! `let` to its value lowered in place. The `analog` blocks of every frame are lowered after the
//! body's own, in frame order (§8.3.16: the callee's rows read what its inputs probe in the
//! caller, and the caller reads the callee's unknowns). A discrete `in` value input (`in x: Bit<f64>`) is a carrier of the
//! frame driven by the caller's expression, an anonymous definition. `cross` and `at` are core:
//! a placement of one is an event of the enclosing body, not a frame.

use piperine_ir::{Constant, Drive, ExprId as LoweredExpr, Hoisted, NodeRole, StmtId as LoweredStmt};

use super::carriers::Frame;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::{BehaviorKind, DeclId, DeclKind, ExprId, ExprKind, FieldInit, PatKind, Resolution, StructClass};
use crate::types::Callee;

impl Elaboration<'_, '_> {
    /// The module with a result that `call` places, if it places one.
    pub(super) fn result_callee(&self, call: ExprId) -> Option<DeclId> {
        match self.types.callee(call) {
            Some(Callee::Module(m)) if matches!(&self.design.decl_data(m).kind, DeclKind::Module(d) if d.ret.is_some()) => Some(m),
            _ => None,
        }
    }

    /// The module `call` places when that module is hoisted: one with a result or with a value input.
    pub(super) fn hoisted_callee(&self, call: ExprId) -> Option<DeclId> {
        match self.types.callee(call) {
            Some(Callee::Module(m)) if self.is_hoisted(m) => Some(m),
            _ => None,
        }
    }

    /// Whether a placement of `module` is hoisted into the body that places it (Appendix C.1.1): it
    /// declares a result or takes a value input. Every other module keeps its own body.
    pub(super) fn is_hoisted(&self, module: DeclId) -> bool {
        let data = self.design.decl_data(module);
        matches!(&data.kind, DeclKind::Module(m) if m.ret.is_some()) || data.children.iter().any(|c| self.is_value_input(*c))
    }

    /// Whether `decl` is a port of a value type: a value input.
    pub(super) fn is_value_input(&self, decl: DeclId) -> bool {
        super::structural::ParamReads::new(self.design, self.types).is_value_input(decl)
    }

    /// The placements, outermost first, that open the frame `frame`: the calls that lead from the
    /// body's own module to it. Empty for the body's own module.
    pub(super) fn placement_chain(&self, frame: usize) -> Vec<ExprId> {
        let mut chain = Vec::new();
        let mut at = frame;
        while let Some(call) = self.frames[at].call {
            chain.push(call);
            match self.frames[at].parent {
                Some(parent) => at = parent,
                None => break,
            }
        }
        chain.reverse();
        chain
    }

    /// Hoist every placement of a hoisted module that the module of the frame `scope.frame`
    /// writes, in source order.
    pub(super) fn hoist_placements(&mut self, scope: &Scope) {
        let module = self.frames[scope.frame].module;
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return };
        // A placement in a loop is hoisted by the unrolling of the loop, once per iteration.
        let looped = self.looped_placements(module);
        for call in data.instances.clone() {
            if self.hoisted_callee(call).is_some() && !looped.contains(&call) {
                self.frame_of(scope, call);
            }
        }
    }

    /// The frame of the placement `call`, written in the frame `scope.frame`; hoisted the first
    /// time it is asked for. `None` after a diagnostic.
    pub(super) fn frame_of(&mut self, scope: &Scope, call: ExprId) -> Option<usize> {
        if let Some(frame) = self.frames[scope.frame].placements.get(&call) {
            return Some(*frame);
        }
        // A placement that failed has said why once; its reads stay silent.
        if self.frames[scope.frame].failed.contains(&call) {
            return None;
        }
        let frame = self.hoist(scope, call);
        if frame.is_none() {
            self.frames[scope.frame].failed.insert(call);
        }
        frame
    }

    /// `call`'s result, lowered where it is read: the callee's result expression in its frame.
    pub(super) fn lower_result(&mut self, scope: &mut Scope, call: ExprId) -> Option<LoweredExpr> {
        let frame = self.frame_of(scope, call)?;
        let module = self.frames[frame].module;
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        let result = data.result?;
        let outer = std::mem::replace(&mut scope.frame, frame);
        let lowered = self.lower_expr(scope, result);
        scope.frame = outer;
        lowered
    }

    /// A read of the value input `decl` of the frame `scope.frame`: the caller's expression,
    /// lowered in the caller's frame. `None` when the frame has no such input (the read is not one).
    pub(super) fn input_of(&self, scope: &Scope, decl: DeclId) -> Option<Option<ExprId>> {
        self.frames.get(scope.frame).and_then(|f| f.inputs.get(&decl)).copied()
    }

    /// Lower `expr`, the caller's expression of an input of the frame `scope.frame`, in the
    /// caller's frame, with `lower`.
    pub(super) fn in_caller<T>(&mut self, scope: &mut Scope, lower: impl FnOnce(&mut Self, &mut Scope) -> Option<T>) -> Option<T> {
        let parent = self.frames[scope.frame].parent?;
        // the caller's expression reads the loop variables it was written under (`unroll.rs`)
        let seen = self.frames[scope.frame].unrolled.clone();
        let outer = std::mem::replace(&mut scope.frame, parent);
        let outer_unrolled = std::mem::replace(&mut scope.unrolled, seen);
        let lowered = lower(self, scope);
        scope.frame = outer;
        scope.unrolled = outer_unrolled;
        lowered
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier
    /// pattern, the matching component of a tuple pattern over a tuple expression.
    pub(super) fn body_let_value(&self, binding: DeclId) -> Option<ExprId> {
        super::consts::ConstEval::body_let_value(self.design, binding)
    }

    /// A read of a module-body `let`: the result of the placement it binds, else its value
    /// lowered where the read stands (Appendix C.1.1: inlined at each read).
    pub(super) fn lower_body_let(&mut self, scope: &mut Scope, value: ExprId) -> Option<LoweredExpr> {
        let value = self.peel_parens(value);
        if matches!(self.design.expr_node(value).kind, ExprKind::Call { .. }) && self.result_callee(value).is_some() {
            return self.lower_result(scope, value);
        }
        self.lower_expr(scope, value)
    }

    /// The name of the placement `call` of `module` in the frame `parent`: the `let` it
    /// initializes, else `<module>$<k>`.
    fn hoisted_name(&mut self, parent: usize, call: ExprId, module: DeclId) -> String {
        let owner = self.design.expr_node(call).owner;
        if let DeclKind::Let(l) = &self.design.decl_data(owner).kind
            && self.peel_parens(l.value) == call
            && let PatKind::Ident { name, sub: None, .. } = &self.design.pat_node(l.pat).kind
        {
            return name.text.clone();
        }
        let caller = self.frames[parent].module;
        let looped = self.looped_placements(caller);
        let same = |c: &ExprId| self.result_callee(*c) == Some(module);
        let outside: Vec<ExprId> = match &self.design.decl_data(caller).kind {
            DeclKind::Module(m) => m.instances.iter().copied().filter(|c| same(c) && !looped.contains(c)).collect(),
            _ => Vec::new(),
        };
        let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str());
        // A placement in an unrolled loop is numbered after the ones outside any loop, in unrolled
        // order (`unroll.rs`, D16); the others count the placements before them in source order.
        let earlier = if looped.contains(&call) {
            let next = self.unrolled_names.entry((parent, module)).or_insert(0);
            *next += 1;
            outside.len() + *next as usize - 1
        } else {
            outside.iter().take_while(|c| **c != call).count()
        };
        format!("{name}${earlier}")
    }

    /// Open the frame of `call`, placed in the frame `scope.frame`, and hoist what the callee
    /// holds into the body: its carriers, the drives of its discrete inputs and its definitions,
    /// its own placements, its reactions. `None` after a diagnostic.
    fn hoist(&mut self, scope: &Scope, call: ExprId) -> Option<usize> {
        let module = self.hoisted_callee(call)?;
        let parent = scope.frame;
        let node = self.design.expr_node(call).clone();
        let file = self.design.decl_data(node.owner).file;
        let mut chain = Some(parent);
        while let Some(k) = chain {
            if self.frames[k].module == module {
                let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.report(file, Diagnostic::new("E0341", format!("module `{name}` places itself through a hoisted placement"), node.span, "recursive placement"));
                return None;
            }
            chain = self.frames[k].parent;
        }
        let ExprKind::Call { callee, args, params, .. } = &node.kind else { return None };
        if !self.hoistable(module, file, node.span) {
            return None;
        }
        let fields: Vec<FieldInit> = match &self.design.expr_node(*callee).kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module => fields.clone(),
            _ => params.clone().unwrap_or_default(),
        };
        let bound = self.hoisted_params(module, &fields, file, node.span)?;
        let name = self.hoisted_name(parent, call, module);
        let prefix = format!("{}{name}.", self.frames[parent].prefix);
        let ports: Vec<DeclId> = self.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_))).collect();
        let mut frame = Frame::root(module);
        frame.parent = Some(parent);
        frame.call = Some(call);
        frame.prefix = prefix;
        frame.params = bound;
        frame.unrolled = scope.unrolled.clone();
        let mut node_ports = Vec::new();
        for (k, port) in ports.iter().enumerate() {
            if self.is_value_input(*port) {
                frame.inputs.insert(*port, args.get(k).map(|a| a.value));
            } else if self.types.carrier(*port).is_none() {
                node_ports.push((*port, args.get(k).map(|a| a.value)));
            }
        }
        // A node port is the caller's terminal for its argument, read in the caller's frame.
        for (port, arg) in node_ports {
            let terminal = self.lower_terminal(scope, arg?)?;
            frame.nodes.insert(port, terminal);
        }
        self.frames.push(frame);
        let index = self.frames.len() - 1;
        self.frames[parent].placements.insert(call, index);
        let gained = self.hoist_nodes(index, module);
        self.record_hoisted(index, module, &name, call, gained);
        self.lower_carriers(index, module);
        // A discrete `in` value input: the hoisted carrier, driven by the caller's expression.
        for (port, arg) in ports.iter().zip(args) {
            let Some(carrier) = self.frames[index].carriers.get(port).copied() else { continue };
            let origin = self.expr_origin(arg.value);
            let wanted = self.structure.carriers.get(carrier as usize).map(|c| c.ty.clone());
            if let Some((value, _)) = self.lower_drive_value(scope, arg.value, None, wanted.as_ref()) {
                self.structure.drives.push(Drive { carrier, value, after: None, origin });
            }
        }
        let mut inner = Self::sub_scope(scope);
        inner.frame = index;
        self.lower_definitions(&inner, module);
        self.hoist_placements(&inner);
        self.lower_digital(&inner, module);
        Some(index)
    }

    /// The node `signal`s of `module` as nodes of the body, named under the frame `frame`: its
    /// terminals are the frame's own. How many nodes the body gained.
    fn hoist_nodes(&mut self, frame: usize, module: DeclId) -> usize {
        let signals: Vec<DeclId> = self.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Signal(_)) && self.types.carrier(*c).is_none()).collect();
        let prefix = self.frames[frame].prefix.clone();
        let mut gained = 0;
        for signal in signals {
            if let Some(terminal) = self.add_node(signal, NodeRole::Signal, &prefix, frame) {
                self.frames[frame].nodes.insert(signal, terminal);
                gained += 1;
            }
        }
        gained
    }

    /// Records the frame `frame` of `module`, placed by `call` under `name`, in `BodyData::hoisted`
    /// when it adds a node or an `analog` statement to the body (the interpreter refuses it by
    /// name, and a reader of the IR sees what was placed where).
    fn record_hoisted(&mut self, frame: usize, module: DeclId, name: &str, call: ExprId, gained_nodes: usize) {
        let has_analog = matches!(&self.design.decl_data(module).kind, DeclKind::Module(m) if m.behaviors.iter().any(|b| matches!(&self.design.decl_data(*b).kind, DeclKind::Behavior(bd) if bd.kind == BehaviorKind::Analog)));
        if gained_nodes == 0 && !has_analog {
            return;
        }
        let node = self.design.expr_node(call);
        let origin = self.locator.origin(self.design.decl_data(node.owner).file, node.span);
        let module_name = self.design.decl_data(self.frames[frame].module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        self.structure.hoisted.push(Hoisted { name: name.to_string(), module: module_name, origin });
    }

    /// The `analog` blocks of every hoisted frame, lowered in frame order after the body's own
    /// (§8.3.16); `None` when no frame has a statement.
    pub(super) fn lower_hoisted_analog(&mut self, scope: &mut Scope) -> Option<Vec<LoweredStmt>> {
        let mut stmts: Option<Vec<LoweredStmt>> = None;
        // A frame the lowering of an earlier frame hoists lazily joins the walk.
        let mut frame = 1;
        while frame < self.frames.len() {
            // A reach frame (`reach.rs`) names a placed child, whose `analog` block is its own body's.
            if self.frames[frame].child.is_some() {
                frame += 1;
                continue;
            }
            let module = self.frames[frame].module;
            let outer = std::mem::replace(&mut scope.frame, frame);
            let lowered = self.lower_analog(scope, module);
            scope.frame = outer;
            if let Some(lowered) = lowered {
                stmts.get_or_insert_with(Vec::new).extend(lowered);
            }
            frame += 1;
        }
        stmts
    }

    /// Whether `module` holds only what a hoisted module can hold: no placement of a module that is
    /// not hoisted (it would need a body inside a frame), no `out` carrier. `P0001` naming the first
    /// form that is not, at the placement.
    fn hoistable(&mut self, module: DeclId, file: crate::pom::FileId, span: crate::msl::Span) -> bool {
        let data = self.design.decl_data(module).clone();
        let DeclKind::Module(m) = &data.kind else { return false };
        let refuse = |this: &mut Self, what: &str| {
            this.unsupported(file, span, &format!("a hoisted module that holds {what},"));
            false
        };
        for child in &data.children {
            if matches!(&self.design.decl_data(*child).kind, DeclKind::Port(p) if self.types.carrier(*child).is_some() && p.dir == Some(crate::pom::Dir::Out)) {
                return refuse(self, "an `out` carrier");
            }
        }
        if m.instances.iter().any(|c| matches!(self.types.callee(*c), Some(Callee::Module(_))) && self.hoisted_callee(*c).is_none()) {
            return refuse(self, "a placement of a module that is not hoisted");
        }
        true
    }

    /// The params of a hoisted module, each bound to a constant: the value the placement's param
    /// block gives, else the default (edge case 5). A required param left out is `E0319`, a false
    /// `where` `E0317`. `None` after a diagnostic.
    fn hoisted_params(&mut self, module: DeclId, fields: &[FieldInit], file: crate::pom::FileId, span: crate::msl::Span) -> Option<std::collections::BTreeMap<DeclId, Constant>> {
        let params: Vec<DeclId> = self.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        let mut bindings = Bindings::new();
        let mut out = std::collections::BTreeMap::new();
        let mut ok = true;
        for param in &params {
            let data = self.design.decl_data(*param).clone();
            let DeclKind::Param(p) = &data.kind else { continue };
            let given = fields.iter().find(|f| f.res == Resolution::Decl(*param)).map(|f| f.value);
            let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let value = match given.or(p.default) {
                Some(expr) => match self.consts.value(expr, &Bindings::new()) {
                    Ok(value) => value,
                    Err(fault) => {
                        self.const_fault(&fault, "a param value of a module with a result");
                        ok = false;
                        continue;
                    }
                },
                None => {
                    self.report(file, Diagnostic::new("E0319", format!("the placement leaves the required param `{name}` unbound"), span, "required param not bound"));
                    ok = false;
                    continue;
                }
            };
            self.bind_param(&mut bindings, *param, value);
            out.insert(*param, value);
        }
        for param in &params {
            let data = self.design.decl_data(*param).clone();
            let DeclKind::Param(p) = &data.kind else { continue };
            let Some(cond) = p.where_ else { continue };
            if let Ok(Constant::Bool(false)) = self.consts.value(cond, &bindings) {
                let name = data.name.as_ref().map_or("", |n| n.text.as_str());
                self.report(file, Diagnostic::new("E0317", format!("the `where` of the param `{name}` is false for this placement"), span, "`where` is false"));
                ok = false;
            }
        }
        ok.then_some(out)
    }
}
