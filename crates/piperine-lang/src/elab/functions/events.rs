//! Functions that return `Event` (part IV §13 rule 7, UE-04, design D14): an IR function is a pure value and an event is
//! not (it has a pin, a guard and a place in a reaction's head), so a call of a `fn` whose result is `Event` is no IR
//! function. It is inlined where it stands: the event its body yields becomes the event of the head, of the body read,
//! of the guard or of the module result the call is written in, and a call inside that body nests the inlining.
//!
//! The parameters bind as `expansion.rs` binds them (the checker's slot per parameter; a handle to the caller's carrier,
//! a `str` to its text, an `Option` to its flag and payload, any other parameter to the argument lowered where the call
//! stands) and are put back when the body is done, so two calls of one function never see each other's arguments. The
//! body is run as the body of a `fn` called inside a `ddx` is (`symbolic.rs`): a `let` and an assignment set a name,
//! a `for` over constant bounds unrolls, and an `if` or a `match` whose choice folds takes its branch. One that is
//! decided at the instant chooses between events, so each branch runs on to the end of the body and its disjuncts are
//! guarded by the choice, `(c && e₁) || (!c && e₂)` (§13 rule 1: a guard is evaluated in the microstep the event fires
//! in). A recursion unrolls through the same expansion stack as the one of `analog`.

use super::FunctionLowering;
use crate::elab::digital::events::Disjunct;
use crate::elab::analog::expansion::Environment;
use super::instances::ConcreteValue;
use crate::elab::elaborator::Scope;
use crate::elab::value::patterns::Subject;
use crate::elab::analog::symbolic::{ExpandedBody, Flow, Step};
use crate::elab::analog::unroll::Expansion;
use crate::refusal::forms::EventFunctionForm;
use crate::msl::Span;
use crate::pom::{Arm, DeclId, DeclKind, ExprId, ExprKind, StmtId, StmtKind};
use crate::types::{Callee, Substitution, Ty};
use piperine_ir::{Constant, Expr, ValueType};


impl FunctionLowering<'_, '_, '_> {
    /// Whether `function` is a `fn` whose declared result is an `Event` (§13 rule 7). The type of a call is not asked: a
    /// call read as a `bool` (§13 rules 2 and 9) is typed `bool`, and is the same call of the same function.
    pub(in crate::elab) fn is_event_function(&self, function: DeclId) -> bool {
        matches!(&self.state.design.decl_data(function).kind, DeclKind::Fn(function_data) if function_data.ret.is_some_and(|ret| self.typed_written(ret) == Some(Ty::Event)))
    }

    /// The disjuncts of the call `call` of the event function `function`: those of the event its body yields, over the
    /// parameters the call binds (§13 rule 7). `None` after a diagnostic.
    pub(in crate::elab) fn event_function_disjuncts(&mut self, scope: &mut Scope, call: ExprId, function: DeclId) -> Option<Vec<Disjunct>> {
        let Some(slots) = self.state.types.binding(call).map(<[_]>::to_vec) else {
            self.elaboration.refuse(scope.values.file, self.state.design.expr_node(call).span, EventFunctionForm::CallWhoseArgumentsTypeCheckerDidNot);
            return None;
        };
        // a function generic over a nature runs for the nature the call is instantiated at
        if self.state.design.decl_data(function).generics_span.is_some() {
            let instance = self.concrete_instance(call)?;
            let substitution = self.instance_substitution(instance.item, &instance.arguments);
            return self.event_function_under(scope, call, instance.item, slots, Some(substitution));
        }
        self.event_function_under(scope, call, function, slots, None)
    }

    /// Whether the call `call` is of a method, or of a function of a trait, whose declared result is an `Event`.
    pub(in crate::elab) fn is_event_method(&self, call: ExprId) -> bool {
        matches!(self.state.types.callee(call), Some(Callee::Method(function) | Callee::Bound { item: function }) if self.is_event_function(function))
    }

    /// The disjuncts of the call `call` of a method whose result is an `Event` (`x.fires()`, `Trait::fires(x)`, or a
    /// call through a bound): the body of the `impl`'s function for the receiver's type, or the trait's default body,
    /// over the receiver and the arguments (§3.7.8, §13 rule 7). `None` after a diagnostic.
    pub(in crate::elab) fn event_method_disjuncts(&mut self, scope: &mut Scope, call: ExprId) -> Option<Vec<Disjunct>> {
        let span = self.state.design.expr_node(call).span;
        let (Some(Callee::Method(named) | Callee::Bound { item: named }), Some((receiver, arguments))) = (self.state.types.callee(call), self.method_operands(call)) else { return None };
        let receiver_type = self.typed(receiver)?;
        let target = if self.is_trait_function(named) {
            let Some(target) = self.values().bound_implementation(call, named, &receiver_type) else {
                self.refuse(scope.values.file, span, EventFunctionForm::MethodThatReturnsEventThatNoImpl);
                return None;
            };
            target
        } else {
            named
        };
        let instance = if self.is_trait_function(target) {
            vec![ConcreteValue::Type(receiver_type)]
        } else if self.is_generic_impl_item(target) {
            self.values().method_impl_arguments(scope, call, target, receiver)?
        } else {
            Vec::new()
        };
        let slots: Vec<Option<ExprId>> = std::iter::once(receiver).chain(arguments).map(Some).collect();
        let substitution = (!instance.is_empty()).then(|| self.instance_substitution(target, &instance));
        self.event_function_under(scope, call, target, slots, substitution)
    }

    /// The receiver and the arguments of the call `call` of a method: `recv.f(arguments)`, or `Trait::f(recv, arguments)`.
    fn method_operands(&self, call: ExprId) -> Option<(ExprId, Vec<ExprId>)> {
        let values: Vec<ExprId> = match &self.state.design.expr_node(call).kind {
            ExprKind::MethodCall { recv, args, .. } => std::iter::once(*recv).chain(args.iter().map(|arg| arg.value)).collect(),
            ExprKind::Call { args, .. } => args.iter().map(|arg| arg.value).collect(),
            _ => return None,
        };
        let (receiver, arguments) = values.split_first()?;
        Some((*receiver, arguments.to_vec()))
    }

    /// `event_function_body` under the substitution of the instance, when there is one.
    fn event_function_under(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, slots: Vec<Option<ExprId>>, substitution: Option<Substitution>) -> Option<Vec<Disjunct>> {
        let outer = substitution.map(|substitution| self.state.consts.set_substitution(substitution));
        let disjuncts = self.event_function_body(scope, call, function, slots);
        if let Some(outer) = outer {
            self.state.consts.set_substitution(outer);
        }
        disjuncts
    }

    /// The disjuncts of the event `function`'s body for the call `call` with the argument `slots`, its parameters bound
    /// for the length of the walk the way an ordinary call binds them (the checker's slot per parameter: a named
    /// argument in its place, a left-out optional one `None`, an `Option` as its flag and payload). A recursion unrolls
    /// while its decisions fold and is `E0126` naming the function when it does not (§3 rule 12).
    fn event_function_body(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, mut slots: Vec<Option<ExprId>>) -> Option<Vec<Disjunct>> {
        let span = self.state.design.expr_node(call).span;
        let data = self.state.design.decl_data(function).clone();
        let DeclKind::Fn(function_data) = &data.kind else { return None };
        let Some(body) = function_data.body.as_ref() else {
            self.refuse(scope.values.file, data.span, EventFunctionForm::FunctionThatReturnsEventAndHasNo);
            return None;
        };
        slots.resize(data.children.iter().filter(|child| matches!(self.state.design.decl_data(**child).kind, DeclKind::FnParam(_))).count(), None);
        let arguments = self.analog().expanded_arguments(scope, call, &data.children, &slots)?;
        if !self.analog().may_expand(scope, function, span) {
            return None;
        }
        let before = Environment::of(scope);
        for argument in arguments {
            argument.bind(scope);
        }
        scope.analog.expanding.push(Expansion::begin(function, scope.values.decisions));
        let disjuncts = crate::stack::Stack::deep(|| self.run_event_body(scope, &body.stmts, data.span));
        scope.analog.expanding.pop();
        before.restore(scope);
        disjuncts
    }

    /// The disjuncts of the event the statements `stmts` of a body yield, run in order (see the module): the value of
    /// the body is its event (§13: the return value of a function). `None` after a diagnostic.
    fn run_event_body(&mut self, scope: &mut Scope, stmts: &[StmtId], at: Span) -> Option<Vec<Disjunct>> {
        if stmts.is_empty() {
            self.refuse(scope.values.file, at, EventFunctionForm::FunctionThatReturnsEventWithEmptyBody);
            return None;
        }
        let mut steps = Vec::new();
        self.analog().push_block(&mut steps, stmts, true);
        self.run_event_steps(scope, steps, at)
    }

    /// Runs `steps` to the event the body yields. `None` after a diagnostic.
    fn run_event_steps(&mut self, scope: &mut Scope, mut steps: Vec<Step>, at: Span) -> Option<Vec<Disjunct>> {
        while let Some(step) = steps.pop() {
            let flow = match step {
                Step::Iterate(variable, value) => {
                    scope.values.unrolled.bind(variable, Constant::Int(value));
                    Flow::Next
                }
                Step::Leave(variable) => {
                    scope.values.unrolled.unbind(variable);
                    Flow::Next
                }
                Step::Statement(stmt) => self.run_event_statement(scope, stmt)?,
                Step::Expression { expr, value } => self.run_event_expression(scope, expr, value, &mut steps, at)?,
            };
            if let Flow::Done(disjuncts) = flow {
                return Some(disjuncts);
            }
        }
        self.refuse(scope.values.file, at, EventFunctionForm::FunctionThatReturnsEventThatDoesNot);
        None
    }

    /// A statement that is no expression: a `let` of a name, or a lone `;`.
    fn run_event_statement(&mut self, scope: &mut Scope, stmt: StmtId) -> Option<Flow<Vec<Disjunct>>> {
        let node = self.state.design.stmt_node(stmt).clone();
        match &node.kind {
            StmtKind::Empty => Some(Flow::Next),
            StmtKind::Let { pat, init: Some(init), else_: None, .. } => {
                self.analog().bind_expanded_let(scope, *pat, *init, node.span, ExpandedBody::EventFunction)?;
                Some(Flow::Next)
            }
            _ => {
                self.refuse(scope.values.file, node.span, EventFunctionForm::StatementThatIsNoLetOfNameBefore);
                None
            }
        }
    }

    /// An expression statement of the body, or the value of the body when `value`.
    fn run_event_expression(&mut self, scope: &mut Scope, expr: ExprId, value: bool, steps: &mut Vec<Step>, at: Span) -> Option<Flow<Vec<Disjunct>>> {
        let expr = self.peel_parens(expr);
        let node = self.state.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Return(Some(returned)) => {
                let event = vec![Step::Expression { expr: *returned, value: true }];
                Some(Flow::Done(self.run_event_steps(scope, event, at)?))
            }
            ExprKind::Assign { .. } => {
                self.analog().run_assignment(scope, expr, ExpandedBody::EventFunction)?;
                Some(Flow::Next)
            }
            ExprKind::Block(block) => {
                self.analog().push_block(steps, &block.stmts, value);
                Some(Flow::Next)
            }
            ExprKind::For { pat, iter, body } => {
                self.analog().unroll_expanded(scope, *pat, *iter, body, steps, node.span, ExpandedBody::EventFunction)?;
                Some(Flow::Next)
            }
            ExprKind::If { cond, then, else_ } => self.run_event_if(scope, *cond, then, *else_, value, steps, at),
            ExprKind::Match { scrutinee, arms } if value => self.run_event_match(scope, *scrutinee, arms, steps, at),
            ExprKind::While { .. } | ExprKind::Loop { .. } | ExprKind::Break(_) | ExprKind::Continue | ExprKind::Return(None) => {
                self.refuse(scope.values.file, node.span, EventFunctionForm::WhileLoopBreakContinueOrReturnWith);
                None
            }
            _ if value => Some(Flow::Done(self.digital().disjuncts(scope, expr)?)),
            _ => {
                self.refuse(scope.values.file, node.span, EventFunctionForm::StatementThatIsNoLetAssignmentIf);
                None
            }
        }
    }

    /// `if cond { then } else { else_ }` in the body. A condition that folds takes its branch. One decided at the
    /// instant forks the run: each branch runs on to the end of the body and the event it yields is guarded by `cond`,
    /// or by `!cond` for the `else`.
    fn run_event_if(&mut self, scope: &mut Scope, cond: ExprId, then: &crate::pom::BlockNode, else_: Option<ExprId>, value: bool, steps: &mut Vec<Step>, at: Span) -> Option<Flow<Vec<Disjunct>>> {
        let span = self.state.design.expr_node(cond).span;
        if matches!(self.state.design.expr_node(cond).kind, ExprKind::Let { .. }) {
            self.refuse(scope.values.file, span, EventFunctionForm::IfLetInFunctionThatReturnsEvent);
            return None;
        }
        if let Some(holds) = self.analog().folded_decision(scope, cond) {
            match (holds, else_) {
                (true, _) => self.analog().push_block(steps, &then.stmts, value),
                (false, Some(otherwise)) => self.analog().push_else(scope, otherwise, value, steps, ExpandedBody::EventFunction)?,
                (false, None) => {}
            }
            return Some(Flow::Next);
        }
        let before = Environment::of(scope);
        scope.values.decisions += 1;
        let mut taken = steps.clone();
        self.analog().push_block(&mut taken, &then.stmts, value);
        let first = self.run_event_steps(scope, taken, at);
        before.clone().restore(scope);
        let mut skipped = steps.clone();
        let pushed = match else_ {
            Some(otherwise) => self.analog().push_else(scope, otherwise, value, &mut skipped, ExpandedBody::EventFunction),
            None => Some(()),
        };
        let second = pushed.and_then(|()| self.run_event_steps(scope, skipped, at));
        before.restore(scope);
        scope.values.decisions -= 1;
        let (mut first, mut second) = (first?, second?);
        for disjunct in &mut first {
            self.guard_by_condition(scope, cond, false, disjunct)?;
        }
        for disjunct in &mut second {
            self.guard_by_condition(scope, cond, true, disjunct)?;
        }
        first.extend(second);
        Some(Flow::Done(first))
    }

    /// The condition `cond`, or its negation, joins the guard of `disjunct`; each disjunct has its own lowering (an IR
    /// expression has one place). `None` after a diagnostic.
    fn guard_by_condition(&mut self, scope: &mut Scope, cond: ExprId, negated: bool, disjunct: &mut Disjunct) -> Option<()> {
        let locals = scope.values.locals.len();
        let mut lowered = self.values().lower_expr(scope, cond)?;
        if negated {
            let origin = self.expr_origin(cond);
            lowered = self.emit(Expr::Unary { op: piperine_ir::UnaryOp::Not, expr: lowered }, ValueType::Bool, origin);
        }
        let span = self.state.design.expr_node(cond).span;
        self.digital().add_conjunct(scope, lowered, locals, span, disjunct)
    }

    /// `match scrutinee { arms }` that yields the event. A scrutinee that folds takes its arm. One decided at the instant
    /// forks the run per arm, the event of arm `k` guarded by the tests that select it: no earlier pattern matched
    /// and this one did. An arm with a guard or a name in its pattern is refused. `None` after a diagnostic.
    fn run_event_match(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[Arm], steps: &mut Vec<Step>, at: Span) -> Option<Flow<Vec<Disjunct>>> {
        if let Some(selected) = self.analog().folded_selection(scope, scrutinee, arms) {
            steps.push(Step::Expression { expr: arms[selected].body, value: true });
            return Some(Flow::Next);
        }
        let subject = if matches!(self.typed(scrutinee), Some(Ty::Option(_))) { Subject::RereadPair(scrutinee) } else { Subject::Reread(scrutinee) };
        for arm in arms {
            let span = self.state.design.expr_node(arm.body).span;
            if arm.guard.is_some() {
                self.refuse(scope.values.file, span, EventFunctionForm::MatchArmWithGuardInFunctionThat);
                return None;
            }
            if !self.values().test_pattern(scope, &subject, arm.pat)?.bindings.is_empty() {
                self.refuse(scope.values.file, span, EventFunctionForm::MatchArmWhosePatternBindsNameIn);
                return None;
            }
        }
        scope.values.decisions += 1;
        let all = self.event_of_arms(scope, scrutinee, arms, at);
        scope.values.decisions -= 1;
        Some(Flow::Done(all?))
    }

    /// The disjuncts of every arm of a `match` on `scrutinee`, each guarded by the tests that select its arm.
    fn event_of_arms(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[Arm], at: Span) -> Option<Vec<Disjunct>> {
        let before = Environment::of(scope);
        let mut all = Vec::new();
        for (position, arm) in arms.iter().enumerate() {
            let body = vec![Step::Expression { expr: arm.body, value: true }];
            let found = self.run_event_steps(scope, body, at);
            before.clone().restore(scope);
            let last = position + 1 == arms.len();
            let earlier: Vec<_> = arms[..position].iter().map(|earlier| earlier.pat).collect();
            for mut disjunct in found? {
                let tests = self.values().standalone_arm_tests(scope, scrutinee, &earlier, (!last).then_some(arm.pat))?;
                let span = self.state.design.expr_node(arm.body).span;
                for test in tests {
                    self.digital().add_conjunct(scope, test, scope.values.locals.len(), span, &mut disjunct)?;
                }
                all.push(disjunct);
            }
        }
        Some(all)
    }
}
