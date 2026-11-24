//! Functions that return `Event` (part IV §13 rule 7, UE-04, design D14): an IR function is a pure value and an event is
//! not (it has a pin, a guard and a place in a reaction's head), so a call of a `fn` whose result is `Event` is no IR
//! function. It is inlined where it stands: the event its body yields becomes the event of the head, of the body read,
//! of the guard or of the module result the call is written in, and a call inside that body nests the inlining.
//!
//! The parameters bind as `expansion.rs` binds them (the checker's slot per parameter; a handle to the caller's carrier,
//! a `str` to its text, an `Option` to its flag and payload, any other parameter to the argument lowered where the call
//! stands) and are put back when the body is done, so two calls of one function never see each other's arguments. The
//! body is its `let`s and its event; a recursion unrolls through the same expansion stack as the one of `analog`.

use super::elaborator::Elaboration;
use super::events::Disjunct;
use super::expansion::Environment;
use super::lower::Scope;
use super::unroll::Expansion;
use crate::msl::Span;
use super::instances::ConcreteValue;
use crate::types::{Callee, Substitution, Ty};
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, StmtId, StmtKind};


impl Elaboration<'_, '_> {
    /// Whether `function` is a `fn` whose declared result is an `Event` (§13 rule 7). The type of a call is not asked: a
    /// call read as a `bool` (§13 rules 2 and 9) is typed `bool`, and is the same call of the same function.
    pub(super) fn is_event_function(&self, function: DeclId) -> bool {
        matches!(&self.design.decl_data(function).kind, DeclKind::Fn(function_data) if function_data.ret.is_some_and(|ret| self.typed_written(ret) == Some(Ty::Event)))
    }

    /// The disjuncts of the call `call` of the event function `function`: those of the event its body yields, over the
    /// parameters the call binds (§13 rule 7). `None` after a diagnostic.
    pub(super) fn event_function_disjuncts(&mut self, scope: &mut Scope, call: ExprId, function: DeclId) -> Option<Vec<Disjunct>> {
        let Some(slots) = self.types.binding(call).map(<[_]>::to_vec) else {
            self.unsupported(scope.file, self.design.expr_node(call).span, "a call whose arguments the type checker did not bind, as an event");
            return None;
        };
        // a function generic over a nature runs for the nature the call is instantiated at
        if self.design.decl_data(function).generics_span.is_some() {
            let instance = self.concrete_instance(call)?;
            let substitution = self.instance_substitution(instance.item, &instance.arguments);
            return self.event_function_under(scope, call, instance.item, slots, Some(substitution));
        }
        self.event_function_under(scope, call, function, slots, None)
    }

    /// Whether the call `call` is of a method, or of a function of a trait, whose declared result is an `Event`.
    pub(super) fn is_event_method(&self, call: ExprId) -> bool {
        matches!(self.types.callee(call), Some(Callee::Method(function) | Callee::Bound { item: function }) if self.is_event_function(function))
    }

    /// The disjuncts of the call `call` of a method whose result is an `Event` (`x.fires()`, `Trait::fires(x)`, or a
    /// call through a bound): the body of the `impl`'s function for the receiver's type, or the trait's default body,
    /// over the receiver and the arguments (§3.7.8, §13 rule 7). `None` after a diagnostic.
    pub(super) fn event_method_disjuncts(&mut self, scope: &mut Scope, call: ExprId) -> Option<Vec<Disjunct>> {
        let span = self.design.expr_node(call).span;
        let (Some(Callee::Method(named) | Callee::Bound { item: named }), Some((receiver, arguments))) = (self.types.callee(call), self.method_operands(call)) else { return None };
        let receiver_type = self.typed(receiver)?;
        let target = if self.is_trait_function(named) {
            let Some(target) = self.bound_implementation(call, named, &receiver_type) else {
                self.unsupported(scope.file, span, "a method that returns an `Event` that no impl gives for this type");
                return None;
            };
            target
        } else {
            named
        };
        let instance = if self.is_trait_function(target) {
            vec![ConcreteValue::Type(receiver_type)]
        } else if self.is_generic_impl_item(target) {
            self.method_impl_arguments(scope, call, target, receiver)?
        } else {
            Vec::new()
        };
        let slots: Vec<Option<ExprId>> = std::iter::once(receiver).chain(arguments).map(Some).collect();
        let substitution = (!instance.is_empty()).then(|| self.instance_substitution(target, &instance));
        self.event_function_under(scope, call, target, slots, substitution)
    }

    /// The receiver and the arguments of the call `call` of a method: `recv.f(arguments)`, or `Trait::f(recv, arguments)`.
    fn method_operands(&self, call: ExprId) -> Option<(ExprId, Vec<ExprId>)> {
        let values: Vec<ExprId> = match &self.design.expr_node(call).kind {
            ExprKind::MethodCall { recv, args, .. } => std::iter::once(*recv).chain(args.iter().map(|arg| arg.value)).collect(),
            ExprKind::Call { args, .. } => args.iter().map(|arg| arg.value).collect(),
            _ => return None,
        };
        let (receiver, arguments) = values.split_first()?;
        Some((*receiver, arguments.to_vec()))
    }

    /// `event_function_body` under the substitution of the instance, when there is one.
    fn event_function_under(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, slots: Vec<Option<ExprId>>, substitution: Option<Substitution>) -> Option<Vec<Disjunct>> {
        let outer = substitution.map(|substitution| self.consts.set_substitution(substitution));
        let disjuncts = self.event_function_body(scope, call, function, slots);
        if let Some(outer) = outer {
            self.consts.set_substitution(outer);
        }
        disjuncts
    }

    /// The disjuncts of the event `function`'s body for the call `call` with the argument `slots`, its parameters bound
    /// for the length of the walk the way an ordinary call binds them (the checker's slot per parameter: a named
    /// argument in its place, a left-out optional one `None`, an `Option` as its flag and payload). A recursion unrolls
    /// while its decisions fold and is `E0126` naming the function when it does not (§3 rule 12).
    fn event_function_body(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, mut slots: Vec<Option<ExprId>>) -> Option<Vec<Disjunct>> {
        let span = self.design.expr_node(call).span;
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(function_data) = &data.kind else { return None };
        let Some(body) = function_data.body.as_ref() else {
            self.unsupported(scope.file, data.span, "a function that returns an `Event` and has no body");
            return None;
        };
        slots.resize(data.children.iter().filter(|child| matches!(self.design.decl_data(**child).kind, DeclKind::FnParam(_))).count(), None);
        let arguments = self.expanded_arguments(scope, call, &data.children, &slots)?;
        if !self.may_expand(scope, function, span) {
            return None;
        }
        let before = Environment::of(scope);
        for argument in arguments {
            argument.bind(scope);
        }
        scope.expanding.push(Expansion::begin(function, scope.decisions));
        let disjuncts = crate::stack::Stack::deep(|| self.event_of_statements(scope, &body.stmts, data.span));
        scope.expanding.pop();
        before.restore(scope);
        disjuncts
    }

    /// The disjuncts of the event the statements `stmts` of a body yield: each `let` binds its name, the last statement
    /// is the event (§13: the return value of a function). `None` after a diagnostic.
    fn event_of_statements(&mut self, scope: &mut Scope, stmts: &[StmtId], at: Span) -> Option<Vec<Disjunct>> {
        let Some((last, locals)) = stmts.split_last() else {
            self.unsupported(scope.file, at, "a function that returns an `Event` with an empty body");
            return None;
        };
        for stmt in locals {
            let node = self.design.stmt_node(*stmt).clone();
            match &node.kind {
                StmtKind::Empty => {}
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => self.bind_expanded_let(scope, *pat, *init, node.span)?,
                _ => {
                    self.unsupported(scope.file, node.span, "a statement that is no `let` of a name before the event of a function that returns an `Event`");
                    return None;
                }
            }
        }
        let node = self.design.stmt_node(*last).clone();
        match &node.kind {
            StmtKind::Expr { expr, semi } if !*semi || self.is_return(*expr) => self.event_of_value(scope, *expr),
            _ => {
                self.unsupported(scope.file, node.span, "a function that returns an `Event` that does not end in its event");
                None
            }
        }
    }

    /// Whether `expr` is a `return` with a value.
    fn is_return(&self, expr: ExprId) -> bool {
        matches!(self.design.expr_node(expr).kind, ExprKind::Return(Some(_)))
    }

    /// The disjuncts of the event the value `expr` of a function body is: a `return`'s value, a block, an `if` whose
    /// condition is decided at elaboration (the branch it takes), or an event expression. `None` after a diagnostic.
    fn event_of_value(&mut self, scope: &mut Scope, expr: ExprId) -> Option<Vec<Disjunct>> {
        let expr = self.peel_parens(expr);
        let node = self.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Return(Some(returned)) => self.event_of_value(scope, *returned),
            ExprKind::Block(block) => self.event_of_statements(scope, &block.stmts, node.span),
            ExprKind::If { cond, then, else_ } => match (self.folded_decision(scope, *cond), else_) {
                (Some(true), _) => self.event_of_statements(scope, &then.stmts, node.span),
                (Some(false), Some(otherwise)) => self.event_of_value(scope, *otherwise),
                _ => {
                    self.unsupported(scope.file, node.span, "an `if` of a function that returns an `Event` whose condition is not decided at elaboration or that has no `else`");
                    None
                }
            },
            _ => self.disjuncts(scope, expr),
        }
    }
}
