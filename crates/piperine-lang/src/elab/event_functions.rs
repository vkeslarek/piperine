//! Functions that return `Event` (part IV §13 rule 7, UE-04, design D14): an IR function is a pure value and an event is
//! not (it has a pin, a guard and a place in a reaction's head), so a call of a `fn` whose result is `Event` is no IR
//! function. It is inlined where it stands: the event its body yields becomes the event of the head, of the body read,
//! of the guard or of the module result the call is written in, and a call inside that body nests the inlining.
//!
//! The parameters bind as `handles.rs` and `expansion.rs` bind them (a handle to the caller's carrier, a `str` to its
//! text, any other parameter to the argument lowered where the call stands) and are put back when the body is done, so
//! two calls of one function never see each other's arguments.

use super::elaborator::Elaboration;
use super::events::Disjunct;
use super::expansion::Environment;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, StmtKind};
use piperine_ir::ExprId as LoweredExpr;
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// Whether `function` is a `fn` whose declared result is an `Event` (§13 rule 7). The type of a call is not asked: a
    /// call read as a `bool` (§13 rules 2 and 9) is typed `bool`, and is the same call of the same function.
    pub(super) fn is_event_function(&self, function: DeclId) -> bool {
        matches!(&self.design.decl_data(function).kind, DeclKind::Fn(f) if f.ret.is_some_and(|ret| self.typed_written(ret) == Some(Ty::Event)))
    }

    /// The disjuncts of the call `call` of the event function `function`: those of the event its body yields, over the
    /// parameters the call binds (§13 rule 7). `None` after a diagnostic.
    pub(super) fn event_function_disjuncts(&mut self, scope: &mut Scope, call: ExprId, function: DeclId) -> Option<Vec<Disjunct>> {
        let span = self.design.expr_node(call).span;
        if self.reaches_recursion(function) {
            self.unsupported(scope.file, span, "a recursion through a function that returns an `Event`");
            return None;
        }
        // a function generic over a nature runs for the nature the call is instantiated at
        if self.design.decl_data(function).generics_span.is_some() {
            let instance = self.concrete_instance(call)?;
            let outer = self.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
            let disjuncts = self.event_function_body(scope, call, instance.item);
            self.consts.set_substitution(outer);
            return disjuncts;
        }
        self.event_function_body(scope, call, function)
    }

    /// The disjuncts of the event `function`'s body for the call `call`, its parameters bound for the length of the walk.
    fn event_function_body(&mut self, scope: &mut Scope, call: ExprId, function: DeclId) -> Option<Vec<Disjunct>> {
        let tail = self.event_function_tail(scope, function)?;
        let ExprKind::Call { args, .. } = &self.design.expr_node(call).kind else { return None };
        let arguments: Vec<ExprId> = args.iter().map(|arg| arg.value).collect();
        let before = Environment::of(scope);
        let bound = self.bind_event_arguments(scope, function, &arguments);
        let disjuncts = if bound { self.disjuncts(scope, tail) } else { None };
        before.restore(scope);
        disjuncts
    }

    /// The expression a function's body yields, when the body is that expression alone: a function that returns an `Event`
    /// is its event and nothing else (§13 rule 7). `None` after a diagnostic.
    fn event_function_tail(&mut self, scope: &Scope, function: DeclId) -> Option<ExprId> {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let Some(body) = f.body.as_ref() else {
            self.unsupported(scope.file, data.span, "a function that returns an `Event` and has no body");
            return None;
        };
        match body.stmts.as_slice() {
            [only] => match &self.design.stmt_node(*only).kind {
                StmtKind::Expr { expr, semi: false } => Some(*expr),
                _ => self.refuse_event_body(scope, data.span),
            },
            _ => self.refuse_event_body(scope, data.span),
        }
    }

    /// The refusal of a body that is more than its event, by name (`P0001`).
    fn refuse_event_body(&mut self, scope: &Scope, span: crate::msl::Span) -> Option<ExprId> {
        self.unsupported(scope.file, span, "a function that returns an `Event` with statements besides its event");
        None
    }

    /// Binds each parameter of `function` to its argument in `scope`: a handle to the caller's carrier, a `str` to its
    /// text, any other to the argument lowered where the call stands. `false` after a diagnostic.
    fn bind_event_arguments(&mut self, scope: &mut Scope, function: DeclId, arguments: &[ExprId]) -> bool {
        let params: Vec<DeclId> = self.design.decl_data(function).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        // every argument is evaluated where the call stands, before any parameter is bound (a name may be a parameter of the caller)
        let mut bindings = Vec::new();
        for (param, argument) in params.iter().zip(arguments) {
            if self.types.handle(*param) {
                bindings.push((*param, Binding::Handle(self.handle_argument_target(scope, *argument))));
            } else if matches!(self.typed(*argument), Some(Ty::Str)) {
                let Some(text) = self.text_of(scope, *argument) else {
                    self.unsupported(scope.file, self.design.expr_node(*argument).span, "a `str` that is not a text after inlining");
                    return false;
                };
                bindings.push((*param, Binding::Text(text)));
            } else {
                let Some(value) = self.lower_expr(scope, *argument) else { return false };
                bindings.push((*param, Binding::Value(value)));
            }
        }
        for (param, binding) in bindings {
            match binding {
                Binding::Handle(target) => scope.handles.insert(param, target).map(drop),
                Binding::Text(text) => scope.texts.insert(param, text).map(drop),
                Binding::Value(value) => scope.values.insert(param, value).map(drop),
            };
        }
        true
    }
}

/// What a parameter of an event function stands for in the walk of its body.
enum Binding {
    /// The caller's carrier or node.
    Handle(ExprId),
    /// The text of a `str` argument.
    Text(String),
    /// The argument lowered where the call stands.
    Value(LoweredExpr),
}
