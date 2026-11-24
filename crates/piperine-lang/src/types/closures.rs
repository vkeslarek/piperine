//! Closures as values (§1.5.14, §3 rule 3): bound by `let`, called, returned in a block, passed to
//! a row of Appendix H, to `limit` or to a parameter of function type. A closure's parameters take
//! their types from annotations, from the type its position expects, or from the first call in the
//! same body (D17); one never called with a parameter left open is `E0181`. The checker keeps each
//! closure's captures and function class beside its type: a call where the class is not allowed is
//! `E0122`, and a closure that captures something is no function pointer.

use std::collections::BTreeSet;

use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::class::FunctionClass;
use super::Ty;

/// The core primitives that read the environment of the solve (§3 table, §14.4): `temperature()` is constant during an
/// analysis, so a function that reads only it is pure.
const SOLVE_READING: &[&str] = &["gmin", "source_scale", "now", "freq", "analysis"];

/// What the checker knows of one closure beyond its type.
#[derive(Debug, Clone)]
pub(super) struct ClosureInfo {
    /// The locals, params and ports of the enclosing body that the closure reads: it captures them
    /// by value where it is written (§1.5.14).
    pub(super) captures: BTreeSet<DeclId>,
    /// The function class of its body.
    pub(super) class: FunctionClass,
}

impl Checker<'_> {
    /// The closure expression `expr` stands for, through parentheses and the value of a block.
    pub(super) fn closure_expr(&self, expr: ExprId) -> Option<ExprId> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Closure { .. } => Some(expr),
            ExprKind::Paren(inner) => self.closure_expr(*inner),
            ExprKind::Block(block) => {
                let last = self.design.stmt_node(*block.stmts.last()?);
                match &last.kind {
                    crate::pom::StmtKind::Expr { expr, semi: false } => self.closure_expr(*expr),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Whether the closure `expr` stands for has a parameter with no written type.
    pub(super) fn has_open_parameter(&self, expr: ExprId) -> bool {
        matches!(&self.design.expr_node(expr).kind, ExprKind::Closure { params, .. } if params.iter().any(|p| p.ty.is_none()))
    }

    /// A closure written where its position expects no type: every parameter must be written. Its
    /// parameters take the annotations; one left open is `E0181`, since only a `let` that is
    /// followed by a call can find it (`defer_closure`).
    pub(super) fn closure_value(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Closure { params, .. } = &node.kind else { return None };
        if self.has_open_parameter(expr) {
            let message = "the type of a closure parameter cannot be inferred here: write it, or call the closure after binding it with `let` (§1.4.4)".to_string();
            self.coded(file, "E0181", node.span, message, "parameter type unknown");
            return None;
        }
        let declared: Vec<Ty> = params.iter().filter_map(|p| p.ty.and_then(|t| self.lower_type(t))).collect();
        if declared.len() != params.len() {
            return None;
        }
        self.closure_typed(expr, &declared, None)?;
        self.exprs[expr.index()].clone()
    }

    /// `let f = |x| ..;`: the closure waits for its first call to give `x` a type.
    pub(super) fn defer_closure(&mut self, binding: DeclId, closure: ExprId) {
        self.pending_closures.insert(binding, closure);
        self.closure_locals.insert(binding, closure);
    }

    /// `E0181` for each closure bound by `let` whose parameters were never given a type by a call.
    pub(super) fn report_uncalled_closures(&mut self) {
        for closure in std::mem::take(&mut self.pending_closures).into_values() {
            let file = self.file_of_expr(closure);
            let span = self.design.expr_node(closure).span;
            let message = "a closure parameter has no written type and the closure is never called: its type cannot be inferred (§1.4.4)".to_string();
            self.coded(file, "E0181", span, message, "parameter type unknown");
        }
    }

    /// The closure bound to the local `binding`, once its first call has typed it.
    pub(super) fn settle_pending(&mut self, binding: DeclId, args: &[Arg]) {
        let Some(closure) = self.pending_closures.remove(&binding) else { return };
        let declared: Option<Vec<Ty>> = args.iter().map(|a| self.infer(a.value)).collect();
        let Some(declared) = declared else {
            self.poisoned.insert(binding);
            return;
        };
        if let Some(found) = self.closure_typed(closure, &declared, None) {
            self.locals.insert(binding, Ty::Fn(declared, Box::new(found)));
        } else {
            self.poisoned.insert(binding);
        }
    }

    /// Records the captures and the class of the closure `expr`, typed just now.
    pub(super) fn record_closure(&mut self, expr: ExprId) {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Closure { body, .. } = &node.kind else { return };
        let mut inside = Vec::new();
        crate::pom::preorder(self.design, *body, &mut inside);
        let mut captures = BTreeSet::new();
        for e in &inside {
            let ExprKind::Path(path) = &self.design.expr_node(*e).kind else { continue };
            let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { continue };
            let data = self.design.decl_data(decl);
            let local = matches!(data.kind, DeclKind::Binding(_) | DeclKind::FnParam(_) | DeclKind::Param(_) | DeclKind::Port(_) | DeclKind::Signal(_));
            let declared_inside = data.span.start >= node.span.start && data.span.end <= node.span.end;
            if local && !declared_inside {
                captures.insert(decl);
            }
        }
        let class = self.class_of_expressions(&inside, &mut BTreeSet::new());
        self.closures.insert(expr, ClosureInfo { captures, class });
    }

    /// The class of a body made of `exprs` (preorder): `emit!`, the environment functions, and the
    /// classes of the functions and the closures it calls. `visited` ends a recursion: the class
    /// of a recursive function is the union over the functions reachable from it, which is the
    /// fixed point of its body's classes (§3, rule 12).
    fn class_of_expressions(&mut self, exprs: &[ExprId], visited: &mut BTreeSet<DeclId>) -> FunctionClass {
        let mut class = FunctionClass::default();
        for e in exprs {
            match &self.design.expr_node(*e).kind {
                ExprKind::Emit { .. } => class.emitting = true,
                // a probe reads the solve: through a handle a function may write one (§3 rule 7, GN-12)
                ExprKind::Call { class: crate::pom::CallClass::Probe, .. } => class.solve_reading = true,
                ExprKind::Call { callee, .. } => match self.callee_resolution(*callee) {
                    Some(Resolution::Builtin(id)) if SOLVE_READING.contains(&id.info().name) => class.solve_reading = true,
                    Some(Resolution::Decl(decl)) => match self.design.decl_data(decl).kind {
                        DeclKind::Fn(_) => class = class.union(self.function_class(decl, visited)),
                        DeclKind::Binding(_) | DeclKind::FnParam(_) => {
                            if let Some(info) = self.closure_locals.get(&decl).and_then(|c| self.closures.get(c)) {
                                class = class.union(info.class);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                },
                _ => {}
            }
        }
        class
    }

    /// The class of the function `decl`, read off its source so that one not checked yet answers
    /// too; the union of its body's class and that of every function it reaches.
    pub(super) fn function_class(&mut self, decl: DeclId, visited: &mut BTreeSet<DeclId>) -> FunctionClass {
        if let Some(known) = self.classes.get(&decl) {
            return *known;
        }
        if !visited.insert(decl) {
            return FunctionClass::default();
        }
        let DeclKind::Fn(f) = &self.design.decl_data(decl).kind else { return FunctionClass::default() };
        let Some(body) = f.body.clone() else { return FunctionClass::default() };
        let mut inside = Vec::new();
        for e in crate::pom::block_exprs(self.design, &body) {
            crate::pom::preorder(self.design, e, &mut inside);
        }
        let class = self.class_of_expressions(&inside, visited);
        // memoized only for the function asked about first: inside a cycle the others are partial
        if visited.len() == 1 {
            self.classes.insert(decl, class);
        }
        class
    }

    /// A call of the named function `function` where its class is not allowed (§3 table, PI-12 AC 3): a
    /// solve-reading function in a const context, an emitting one in a const context or a module body, is
    /// `E0122` naming the function and the class. `emit!` written directly stays `E0601`.
    pub(super) fn refuse_outside_class(&mut self, expr: ExprId, function: DeclId) {
        let Some(class) = self.function_class(function, &mut BTreeSet::new()).refused_in(self.context) else { return };
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let name = self.design.decl_data(function).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("the function `{name}` is {class} and is called where that class is not allowed (§3)");
        self.coded(file, "E0122", span, message, "a call out of its class");
    }

    /// A call of the local `decl` whose type is `fn(..) -> R`, a closure or a function pointer:
    /// the arguments against its parameters, and, for a closure, `E0122` where its class is not
    /// allowed. There is no named function to record as the callee.
    pub(super) fn local_call(&mut self, expr: ExprId, decl: DeclId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        self.settle_pending(decl, args);
        // a value of a type parameter bound by `Fn(A) -> R` is called through the bound (§3.8.6)
        let local = match self.locals.get(&decl).cloned() {
            Some(Ty::Param(param)) => self.callable_of(param).or(Some(Ty::Param(param))),
            other => other,
        };
        let Some(Ty::Fn(params, result)) = local else {
            match local {
                // the name of an instance without a result is only the head of `inst.name` (§8.2.14)
                Some(Ty::Module(module)) if self.binds_an_instance(decl) => {
                    self.instance_as_value(file, span, decl, module.decl);
                }
                // the callee's position requires a function (or a template, which places): any other type is a mismatch (§1.5)
                Some(found) => {
                    let name = self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                    let message = format!("`{name}` is called, but its type `{}` is no function and no template (§1.5)", found.name(self.design));
                    self.coded(file, "E0180", span, message, "a call of a value that is not a function");
                }
                None if !self.poisoned.contains(&decl) => self.unsupported(file, span, "a call of a value whose type is unknown"),
                None => {}
            }
            for arg in args {
                self.infer(arg.value);
            }
            return None;
        };
        if let Some(info) = self.closure_locals.get(&decl).and_then(|c| self.closures.get(c)).cloned()
            && let Some(class) = info.class.refused_in(self.context)
        {
            let name = self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let message = format!("the closure `{name}` is {class} and is called where that class is not allowed (§3)");
            self.coded(file, "E0122", span, message, "a call out of its class");
        }
        self.apply(expr, &params, args)?;
        Some(*result)
    }

    /// Whether `expr` is written as a function value: a closure, the name of a function, or a local
    /// bound to a closure (§1.2.10).
    pub(super) fn is_function_value(&self, expr: ExprId) -> bool {
        if self.closure_expr(expr).is_some() {
            return true;
        }
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.is_function_value(*inner),
            ExprKind::Path(_) => match self.callee_resolution(expr) {
                Some(Resolution::Decl(decl)) => matches!(self.design.decl_data(decl).kind, DeclKind::Fn(_)) || self.closure_locals.contains_key(&decl),
                _ => false,
            },
            _ => false,
        }
    }

    /// Whether `expr` is a closure that captures something: no function pointer (§1.2.10).
    pub(super) fn is_capturing_closure(&self, expr: ExprId) -> bool {
        let closure = match &self.design.expr_node(expr).kind {
            ExprKind::Path(_) => match self.callee_resolution(expr) {
                Some(Resolution::Decl(decl)) => self.closure_locals.get(&decl).copied(),
                _ => None,
            },
            _ => self.closure_expr(expr),
        };
        closure.and_then(|c| self.closures.get(&c)).is_some_and(|info| !info.captures.is_empty())
    }

    /// `P0001` where a closure that captures is written where a function pointer is required: a
    /// parameter of a user `fn`, or an annotated `let`. Only a closure that captures nothing is
    /// a function pointer.
    pub(super) fn refuse_capturing_pointer(&mut self, expr: ExprId) {
        if self.is_capturing_closure(expr) {
            let file = self.file_of_expr(expr);
            let span = self.design.expr_node(expr).span;
            self.unsupported(file, span, "a closure that captures where a function pointer `fn(..) -> R` is expected (only a closure that captures nothing coerces to one)");
        }
    }
}
