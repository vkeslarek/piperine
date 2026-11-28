//! The body of a `fn` called inside a differentiated value, run symbolically (Part IV §14.2.3, analog-operators AO-01).
//!
//! The call is expanded where it stands (`expansion.rs`) into one expression, so the derivative is built over it as
//! over any other. The body is a list of steps, run in order over an environment that maps each parameter and each
//! `let` to the expression it holds (`Scope::values`):
//!
//! * a `let` and an assignment (`=` and `+=`) set a name to the expression of their right side;
//! * an `if` whose condition folds takes its branch; one that does not forks the run: each branch runs on to the end
//!   of the body, and the value is `if cond { first } else { second }`. So an early `return` inside an `if`, and
//!   assignments in one branch only, need no merge: the code after the `if` runs in each branch;
//! * a `for` over a range whose bounds fold is unrolled, its variable the constant of each iteration;
//! * a `return` ends the run with its value; the last expression of the body without a `;` is the value too.
//!
//! A form with no finite symbolic reading (a `while`, a `loop`, a `break`, a `match` statement, a `let` that takes
//! a pattern, a call made for its effect) is `P0001` naming it (GAPS L74), never a wrong value.

use piperine_ir::{Constant, Expr, ExprId as LoweredExpr};

use super::elaborator::Elaboration;
use super::expansion::Environment;
use super::lower::Scope;
use crate::refusal::forms::AnalogForm;
use crate::msl::Span;
use crate::pom::{BlockNode, DeclId, ExprId, ExprKind, PatKind, Resolution, StmtId, StmtKind};
use crate::types::Ty;

/// The most forks one expansion makes: each `if` that does not fold doubles the paths still to run.
const FORKS: u32 = 1 << 12;

/// The most iterations one loop of an expanded body is unrolled into.
const ITERATIONS: i128 = 1 << 12;

/// What an expanded body still has to run; the next step is the last of the list.
#[derive(Clone)]
pub(super) enum Step {
    /// A statement that is no expression: a `let`, a lone `;`.
    Statement(StmtId),
    /// An expression statement, and whether it is the value of the body.
    Expression { expr: ExprId, value: bool },
    /// A loop variable takes the constant of an iteration.
    Iterate(DeclId, i128),
    /// A loop variable goes out of scope.
    Leave(DeclId),
}

/// What a body is expanded for: it words the refusal of a form the expansion cannot run, so a diagnostic names the
/// construct the author wrote.
#[derive(Clone, Copy)]
pub(super) enum ExpandedBody {
    /// A `fn` called inside a differentiated value (§14.2.3).
    Differentiated,
    /// A `fn` whose result is an `Event` (§13 rule 7).
    EventFunction,
}

impl ExpandedBody {
    /// The body as a diagnostic names it.
    fn describes(self) -> &'static str {
        match self {
            ExpandedBody::Differentiated => "a `fn` called inside a `ddx`",
            ExpandedBody::EventFunction => "a `fn` that returns an `Event`",
        }
    }
}

/// Where a step leaves the run: the body is done with a value, or the next step runs.
pub(super) enum Flow<T = LoweredExpr> {
    /// The body is done: the value of the call.
    Done(T),
    /// The next step runs.
    Next,
}

impl Elaboration<'_, '_> {
    /// The value of the body `body` of a `fn` expanded at the call at `at`, its parameters bound in `scope`. `None`
    /// after a diagnostic.
    pub(super) fn run_body(&mut self, scope: &mut Scope, body: &BlockNode, at: Span) -> Option<LoweredExpr> {
        let mut steps = Vec::new();
        self.push_block(&mut steps, &body.stmts, true);
        self.run_steps(scope, steps, at)
    }

    /// Runs `steps` to the end of the body. `None` after a diagnostic.
    fn run_steps(&mut self, scope: &mut Scope, mut steps: Vec<Step>, at: Span) -> Option<LoweredExpr> {
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
                Step::Statement(stmt) => self.run_statement(scope, stmt)?,
                Step::Expression { expr, value } => self.run_expression(scope, expr, value, &mut steps, at)?,
            };
            if let Flow::Done(value) = flow {
                return Some(value);
            }
        }
        self.refuse(scope.values.file, at, AnalogForm::FnWithNoValueInsideDdx);
        None
    }

    /// Pushes the statements `stmts` of a block on `steps`, the first to run last; the last of them, written with
    /// no `;`, is the value when `value`.
    pub(super) fn push_block(&self, steps: &mut Vec<Step>, stmts: &[StmtId], value: bool) {
        for (index, stmt) in stmts.iter().enumerate().rev() {
            steps.push(match &self.state.design.stmt_node(*stmt).kind {
                StmtKind::Expr { expr, semi } => Step::Expression { expr: *expr, value: value && !*semi && index + 1 == stmts.len() },
                _ => Step::Statement(*stmt),
            });
        }
    }

    /// A statement that is no expression: a `let` of a name, or a lone `;`.
    fn run_statement(&mut self, scope: &mut Scope, stmt: StmtId) -> Option<Flow> {
        let node = self.state.design.stmt_node(stmt).clone();
        let file = self.state.design.decl_data(node.owner).file;
        match &node.kind {
            StmtKind::Empty => Some(Flow::Next),
            StmtKind::Let { pat, init: Some(init), else_: None, .. } => {
                self.bind_expanded_let(scope, *pat, *init, node.span, ExpandedBody::Differentiated)?;
                Some(Flow::Next)
            }
            _ => {
                self.refuse(file, node.span, AnalogForm::StatementThatIsNoLetOfName);
                None
            }
        }
    }

    /// `let name = init;`: the name holds the lowered initializer (an `Option` its pair, a `str` its text).
    pub(super) fn bind_expanded_let(&mut self, scope: &mut Scope, pat: crate::pom::PatId, init: ExprId, span: Span, body: ExpandedBody) -> Option<()> {
        let PatKind::Ident { binding, sub: None, .. } = self.state.design.pat_node(pat).kind else {
            self.refuse_with(scope.values.file, span, AnalogForm::LetOfPatternThatIsNoName, [body.describes().to_string()]);
            return None;
        };
        match self.typed_local(binding) {
            Some(Ty::Str) => {
                let text = self.text_of(scope, init)?;
                scope.digital.texts.insert(binding, text);
            }
            Some(Ty::Option(_)) => {
                let pair = self.lower_option(scope, init)?;
                scope.analog.option_values.insert(binding, (pair.is, pair.val));
            }
            _ => self.set_expanded_name(scope, binding, init)?,
        }
        Some(())
    }

    /// The name `binding` holds the value of `source` from here on, and the constant it folds to, when it does.
    fn set_expanded_name(&mut self, scope: &mut Scope, binding: DeclId, source: ExprId) -> Option<()> {
        let folded = self.fold_scalar(scope, source);
        let value = self.lower_expr(scope, source)?;
        scope.analog.values.insert(binding, value);
        match folded {
            Some(constant) => scope.values.unrolled.bind(binding, constant),
            None => {
                scope.values.unrolled.unbind(binding);
            }
        }
        Some(())
    }

    /// An expression statement, or the value of the body when `value`.
    fn run_expression(&mut self, scope: &mut Scope, expr: ExprId, value: bool, steps: &mut Vec<Step>, at: Span) -> Option<Flow> {
        let expr = self.peel_parens(expr);
        let node = self.state.design.expr_node(expr).clone();
        let file = self.state.design.decl_data(node.owner).file;
        match &node.kind {
            ExprKind::Return(Some(returned)) => Some(Flow::Done(self.lower_expr(scope, *returned)?)),
            ExprKind::Assign { .. } => {
                self.run_assignment(scope, expr, ExpandedBody::Differentiated)?;
                Some(Flow::Next)
            }
            ExprKind::If { cond, then, else_ } => self.run_if(scope, *cond, then, *else_, value, steps, at),
            ExprKind::Block(block) => {
                self.push_block(steps, &block.stmts, value);
                Some(Flow::Next)
            }
            ExprKind::For { pat, iter, body } => {
                self.unroll_expanded(scope, *pat, *iter, body, steps, node.span, ExpandedBody::Differentiated)?;
                Some(Flow::Next)
            }
            ExprKind::While { .. } | ExprKind::Loop { .. } => {
                self.refuse(file, node.span, AnalogForm::WhileOrLoopInFnCalledInside);
                None
            }
            ExprKind::Break(_) | ExprKind::Continue | ExprKind::Return(None) => {
                self.refuse(file, node.span, AnalogForm::BreakContinueOrReturnWithNoValue);
                None
            }
            _ if value => Some(Flow::Done(self.lower_expr(scope, expr)?)),
            _ => {
                self.refuse(file, node.span, AnalogForm::ExpressionStatementThatIsNoAssignmentIf);
                None
            }
        }
    }

    /// `name = value;` and `name op= value;`: the name of a parameter or a `let` holds the new expression.
    pub(super) fn run_assignment(&mut self, scope: &mut Scope, assignment: ExprId, body: ExpandedBody) -> Option<()> {
        let node = self.state.design.expr_node(assignment).clone();
        let ExprKind::Assign { op, lhs, rhs } = node.kind else { return None };
        let target = self.peel_parens(lhs);
        let named = match &self.state.design.expr_node(target).kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if scope.analog.values.contains_key(&decl) => Some(decl),
                _ => None,
            },
            _ => None,
        };
        let Some(decl) = named else {
            self.refuse_with(scope.values.file, node.span, AnalogForm::AssignmentToSomethingThatIsNoName, [body.describes().to_string()]);
            return None;
        };
        let Some(op) = op else { return self.set_expanded_name(scope, decl, rhs) };
        if matches!(self.state.types.callee(assignment), Some(crate::types::Callee::Method(_))) {
            self.refuse_with(scope.values.file, node.span, AnalogForm::CompoundAssignmentThroughOperatorOfTypeIn, [body.describes().to_string()]);
            return None;
        }
        let old = scope.analog.values[&decl];
        let operand = self.lower_expr(scope, rhs)?;
        let (ty, origin) = (self.type_of(old), self.expr_origin(assignment));
        let value = self.emit(Expr::Binary { op: Self::binary_op(op), lhs: old, rhs: operand }, ty, origin);
        scope.analog.values.insert(decl, value);
        scope.values.unrolled.unbind(decl);
        Some(())
    }

    /// `if cond { then } else { else_ }` in the body. A condition that folds takes its branch; one that does not forks
    /// the run into the two branches, each followed by the rest of the body.
    fn run_if(&mut self, scope: &mut Scope, cond: ExprId, then: &BlockNode, else_: Option<ExprId>, value: bool, steps: &mut Vec<Step>, at: Span) -> Option<Flow> {
        let span = self.state.design.expr_node(cond).span;
        if matches!(self.state.design.expr_node(cond).kind, ExprKind::Let { .. }) {
            self.refuse(scope.values.file, span, AnalogForm::IfLetInFnCalledInsideDdx);
            return None;
        }
        if let Some(holds) = self.folded_decision(scope, cond) {
            match (holds, else_) {
                (true, _) => self.push_block(steps, &then.stmts, value),
                (false, Some(otherwise)) => self.push_else(scope, otherwise, value, steps, ExpandedBody::Differentiated)?,
                (false, None) => {}
            }
            return Some(Flow::Next);
        }
        scope.analog.forks += 1;
        if scope.analog.forks > FORKS {
            self.refuse(scope.values.file, span, AnalogForm::FnCalledInsideDdxThatForksInto);
            return None;
        }
        let test = self.lower_expr(scope, cond)?;
        let origin = self.expr_origin(cond);
        let before = Environment::of(scope);
        scope.values.decisions += 1;
        let mut taken = steps.clone();
        self.push_block(&mut taken, &then.stmts, value);
        let first = self.run_steps(scope, taken, at);
        before.clone().restore(scope);
        let mut skipped = steps.clone();
        let pushed = match else_ {
            Some(otherwise) => self.push_else(scope, otherwise, value, &mut skipped, ExpandedBody::Differentiated),
            None => Some(()),
        };
        let second = pushed.and_then(|()| self.run_steps(scope, skipped, at));
        before.restore(scope);
        scope.values.decisions -= 1;
        let (first, second) = (first?, second?);
        let ty = self.type_of(first);
        Some(Flow::Done(self.emit(Expr::If { cond: test, then: first, otherwise: second }, ty, origin)))
    }

    /// The `else` of an `if`: a block, or the `if` of an `else if`.
    pub(super) fn push_else(&mut self, scope: &Scope, otherwise: ExprId, value: bool, steps: &mut Vec<Step>, body: ExpandedBody) -> Option<()> {
        match &self.state.design.expr_node(otherwise).kind {
            ExprKind::Block(block) => self.push_block(steps, &block.stmts, value),
            ExprKind::If { .. } => steps.push(Step::Expression { expr: otherwise, value }),
            _ => {
                self.refuse_with(scope.values.file, self.state.design.expr_node(otherwise).span, AnalogForm::ElseThatIsNoBlockOrIf, [body.describes().to_string()]);
                return None;
            }
        }
        Some(())
    }

    /// `for i in lo..hi { body }` over bounds that fold: the body once per value of `i`, pushed on `steps`.
    pub(super) fn unroll_expanded(&mut self, scope: &mut Scope, pat: crate::pom::PatId, iter: ExprId, body: &BlockNode, steps: &mut Vec<Step>, span: Span, context: ExpandedBody) -> Option<()> {
        let iter = self.peel_parens(iter);
        let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.state.design.expr_node(iter).kind else {
            self.refuse_with(scope.values.file, span, AnalogForm::ForThatIsNotOverRangeWith, [context.describes().to_string()]);
            return None;
        };
        let variable = match self.state.design.pat_node(pat).kind {
            PatKind::Ident { binding, sub: None, .. } => Some(binding),
            PatKind::Wild => None,
            _ => {
                self.refuse_with(scope.values.file, span, AnalogForm::ForWhoseVariableIsNoNameIn, [context.describes().to_string()]);
                return None;
            }
        };
        let (Some(Constant::Int(first)), Some(Constant::Int(last))) = (self.fold_scalar(scope, lo), self.fold_scalar(scope, hi)) else {
            self.refuse_with(scope.values.file, span, AnalogForm::ForWhoseBoundsDoNotFoldTo, [context.describes().to_string()]);
            return None;
        };
        let end = if inclusive { last + 1 } else { last };
        if end - first > ITERATIONS {
            self.refuse_with(scope.values.file, span, AnalogForm::ForOfMoreThanIterationsIn, [context.describes().to_string()]);
            return None;
        }
        if self.body_exprs(body).iter().any(|e| matches!(self.state.design.expr_node(*e).kind, ExprKind::Break(_) | ExprKind::Continue | ExprKind::While { .. } | ExprKind::Loop { .. })) {
            self.refuse_with(scope.values.file, span, AnalogForm::BreakContinueOrNestedWhileInLoop, [context.describes().to_string()]);
            return None;
        }
        if let Some(variable) = variable {
            steps.push(Step::Leave(variable));
        }
        for value in (first..end.max(first)).rev() {
            self.push_block(steps, &body.stmts, false);
            if let Some(variable) = variable {
                steps.push(Step::Iterate(variable, value));
            }
        }
        Some(())
    }
}
