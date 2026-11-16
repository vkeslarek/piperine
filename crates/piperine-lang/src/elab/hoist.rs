//! Hoisting a module with a result (part III §8.3.12, Appendix C.1.1; DIG-10).
//!
//! A placement of a module that declares `-> T` gets no body and no instance: its carriers,
//! drives, events and reactions join the body of the module that places it, under the
//! placement's name (`gate.x` for `let gate = transition(..)`, else `<module>$<k>`, `k` counting
//! that module's placements in source order), and each read of it lowers the callee's result
//! expression where the read stands. The POM keeps the placement as written: hoisting is a view
//! of the IR, never a rewrite of the authored tree.
//!
//! Each placement is a [`Frame`]: the callee module, the frame of its caller, its params bound
//! to constants, and its value inputs bound to the caller's expressions. A name read in a frame
//! resolves there: a carrier to its hoisted carrier, a param to its constant, a value input to the
//! caller's expression lowered in the caller's frame (§8.3.8, §8.3.13), a module-body `let` to
//! its value lowered in place. A discrete `in` value input (`in x: Bit<f64>`) is a carrier of the
//! frame driven by the caller's expression, an anonymous definition. `cross` and `at` are core:
//! a placement of one is an event of the enclosing body, not a frame.

use piperine_ir::{Constant, Drive, ExprId as LoweredExpr};

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

    /// Hoist every placement of a module with a result that the module of the frame `scope.frame`
    /// writes, in source order.
    pub(super) fn hoist_placements(&mut self, scope: &Scope) {
        let module = self.frames[scope.frame].module;
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return };
        for call in data.instances.clone() {
            if self.result_callee(call).is_some() {
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
        let outer = std::mem::replace(&mut scope.frame, parent);
        let lowered = lower(self, scope);
        scope.frame = outer;
        lowered
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier
    /// pattern, the matching component of a tuple pattern over a tuple expression.
    pub(super) fn body_let_value(&self, binding: DeclId) -> Option<ExprId> {
        let data = self.design.decl_data(binding);
        let DeclKind::Binding(b) = &data.kind else { return None };
        if b.origin != crate::pom::BindingOrigin::BodyLet {
            return None;
        }
        let DeclKind::Let(l) = &self.design.decl_data(data.parent?).kind else { return None };
        match &self.design.pat_node(l.pat).kind {
            PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding => Some(l.value),
            PatKind::Tuple(items) => {
                let at = items.iter().position(|p| matches!(&self.design.pat_node(*p).kind, PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding))?;
                match &self.design.expr_node(self.peel_parens(l.value)).kind {
                    ExprKind::Tuple(values) if values.len() == items.len() => Some(values[at]),
                    _ => None,
                }
            }
            _ => None,
        }
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
    fn hoisted_name(&self, parent: usize, call: ExprId, module: DeclId) -> String {
        let owner = self.design.expr_node(call).owner;
        if let DeclKind::Let(l) = &self.design.decl_data(owner).kind
            && self.peel_parens(l.value) == call
            && let PatKind::Ident { name, sub: None, .. } = &self.design.pat_node(l.pat).kind
        {
            return name.text.clone();
        }
        let caller = self.frames[parent].module;
        let earlier = match &self.design.decl_data(caller).kind {
            DeclKind::Module(m) => m.instances.iter().take_while(|c| **c != call).filter(|c| self.result_callee(**c) == Some(module)).count(),
            _ => 0,
        };
        let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str());
        format!("{name}${earlier}")
    }

    /// Open the frame of `call`, placed in the frame `scope.frame`, and hoist what the callee
    /// holds into the body: its carriers, the drives of its discrete inputs and its definitions,
    /// its own placements, its reactions. `None` after a diagnostic.
    fn hoist(&mut self, scope: &Scope, call: ExprId) -> Option<usize> {
        let module = self.result_callee(call)?;
        let parent = scope.frame;
        let node = self.design.expr_node(call).clone();
        let file = self.design.decl_data(node.owner).file;
        let mut chain = Some(parent);
        while let Some(k) = chain {
            if self.frames[k].module == module {
                let name = self.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.report(file, Diagnostic::new("E0341", format!("module `{name}` places itself through its result"), node.span, "recursive placement"));
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
        frame.prefix = prefix;
        frame.params = bound;
        for (k, port) in ports.iter().enumerate() {
            if self.types.carrier(*port).is_none() {
                frame.inputs.insert(*port, args.get(k).map(|a| a.value));
            }
        }
        self.frames.push(frame);
        let index = self.frames.len() - 1;
        self.frames[parent].placements.insert(call, index);
        self.lower_carriers(index, module);
        // A discrete `in` value input: the hoisted carrier, driven by the caller's expression.
        for (port, arg) in ports.iter().zip(args) {
            let Some(carrier) = self.frames[index].carriers.get(port).copied() else { continue };
            let origin = self.expr_origin(arg.value);
            if let Some((value, _)) = self.lower_drive_value(scope, arg.value, None) {
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

    /// Whether `module` holds only what a hoisted module can hold: no node, no `analog` block,
    /// no placement of a module without a result, no `out` carrier. `P0001` naming the first
    /// form that is not, at the placement.
    fn hoistable(&mut self, module: DeclId, file: crate::pom::FileId, span: crate::msl::Span) -> bool {
        let data = self.design.decl_data(module).clone();
        let DeclKind::Module(m) = &data.kind else { return false };
        let refuse = |this: &mut Self, what: &str| {
            this.unsupported(file, span, &format!("a module with a result that holds {what},"));
            false
        };
        for child in &data.children {
            match &self.design.decl_data(*child).kind {
                DeclKind::Port(p) if self.types.carrier(*child).is_some() && p.dir == Some(crate::pom::Dir::Out) => return refuse(self, "an `out` carrier"),
                DeclKind::Port(p) if self.types.carrier(*child).is_none() && self.design.nature_of(p.ty).is_some() => return refuse(self, "a node port"),
                DeclKind::Signal(_) if self.types.carrier(*child).is_none() => return refuse(self, "a node"),
                _ => {}
            }
        }
        if m.behaviors.iter().any(|b| matches!(&self.design.decl_data(*b).kind, DeclKind::Behavior(bd) if bd.kind == BehaviorKind::Analog)) {
            return refuse(self, "an `analog` block");
        }
        if m.instances.iter().any(|c| matches!(self.types.callee(*c), Some(Callee::Module(_))) && self.result_callee(*c).is_none()) {
            return refuse(self, "a placement of a module without a result");
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
            bindings.bind(*param, value);
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
