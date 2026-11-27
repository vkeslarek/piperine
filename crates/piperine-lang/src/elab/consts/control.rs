//! The forms of phase A in a const context (CV-36; part I §1.5.12, §1.7 to §1.8, §3): `let mut`,
//! assignment, `while`, `loop` and `for` over a range, `break`, `continue`, `return`, the patterns
//! of `match`, `if let`, `while let` and `let .. else`, closures, function values and recursion
//! evaluate at elaboration to the value they have at run time.
//!
//! A statement evaluates to a value or to nothing (`exec`): `()` is no [`Constant`]. An exit is an
//! `Err(ConstFault::Leaves)` the loop or function it leaves catches, so every `?` between the exit
//! and its target unwinds as it would in the source; the value a `break` or `return` carries
//! waits in `ConstEval::leaving`. A loop that runs more than 2^24 iterations and a call nested more
//! than 1 024 deep are panics (§1 rule 11, Q5): `E0104` at elaboration.
//!
//! A decision reads a value, and every value is a value (§3.3.1): an `if` takes one arm, a `match`
//! the first arm that matches, a loop runs while its condition holds. A read that has no constant
//! at elaboration is a fault at that read.

use std::collections::BTreeMap;

use piperine_ir::Constant;

use super::{Bindings, ConstEval, ConstFault, Exit};
use crate::refusal::forms::{ConstantForm, ValueForm};
use crate::pom::{Arm, BlockNode, DeclId, DeclKind, ExprId, ExprKind, PatId, PatKind, Resolution, StmtId, StmtKind};

/// The most iterations one loop runs in one evaluation (Q5, §1 rule 11).
const TRIPS: u64 = 1 << 24;

/// A function as a value: a name of a `fn`, or a closure with the locals it captured by value at
/// the place it was written (§3 rule 3).
#[derive(Clone)]
pub(super) enum FnValue {
    /// A `fn` item.
    Named(DeclId),
    /// A closure: the patterns of its parameters, its body, and what it captured.
    Closure { params: Vec<PatId>, body: ExprId, captured: BTreeMap<DeclId, Constant>, functions: BTreeMap<DeclId, FnValue> },
}

/// One argument of a call: its value, or the function it stands for.
pub(super) struct Argument {
    /// The value; `None` for a function argument, which has none.
    pub(super) value: Option<Constant>,
    /// The function the argument is, when it is one.
    pub(super) function: Option<FnValue>,
}

/// What a loop's body did: the loop goes on, or it ends with a value.
enum Flow {
    Next,
    Done(Option<Constant>),
}

impl ConstEval<'_, '_> {
    /// `expr` evaluated as a statement or a value: `Some` value, or `None` for `()`.
    pub(super) fn exec(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        let kind = self.design.expr_node(expr).kind.clone();
        match &kind {
            ExprKind::Block(block) => self.exec_block(expr, &block.stmts, bindings),
            ExprKind::If { cond, then, else_ } => self.exec_if(expr, *cond, then, *else_, bindings),
            ExprKind::While { cond, body } => self.exec_while(expr, *cond, body, bindings),
            ExprKind::Loop { body } => self.exec_loop(expr, body, bindings),
            ExprKind::For { pat, iter, body } => self.exec_for(expr, *pat, *iter, body, bindings),
            ExprKind::Assign { op, lhs, rhs } => self.exec_assign(expr, *op, *lhs, *rhs, bindings).map(|()| None),
            ExprKind::Match { scrutinee, arms } => self.exec_match(expr, *scrutinee, arms, bindings),
            ExprKind::Break(value) => self.exit(expr, Exit::Break, *value, bindings),
            ExprKind::Continue => self.exit(expr, Exit::Continue, None, bindings),
            ExprKind::Return(value) => self.exit(expr, Exit::Return, *value, bindings),
            _ => self.fold(expr, bindings).map(Some),
        }
    }

    /// `expr` as a value, for the forms of `exec` that may have none.
    pub(super) fn exec_value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        self.exec(expr, bindings)?.ok_or(ConstFault::refused(expr, ConstantForm::FormWithNoValueWhereValueIs))
    }

    /// `break`, `continue` or `return`, with its value evaluated: the fault the loop or function it
    /// leaves catches.
    fn exit(&mut self, expr: ExprId, exit: Exit, value: Option<ExprId>, bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        self.leaving = match value {
            Some(value) => Some(self.fold(value, bindings)?),
            None => None,
        };
        Err(ConstFault::Leaves { expr, exit })
    }

    /// The statements of a block: the value of the last one when it is an expression without `;`,
    /// else `None`. A `let` binds into the locals, a function value into the function values.
    pub(super) fn exec_block(&mut self, expr: ExprId, stmts: &[StmtId], bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        let mut value = None;
        for stmt in stmts {
            value = None;
            match self.design.stmt_node(*stmt).kind.clone() {
                StmtKind::Let { pat, init: Some(init), else_, .. } => self.exec_let(expr, pat, init, else_.as_ref(), bindings)?,
                // `let x;` (§1.7.5): the checker proves it is assigned before it is read
                StmtKind::Let { init: None, .. } => {}
                StmtKind::Expr { expr: inner, semi } => {
                    let inner_value = self.exec(inner, bindings)?;
                    if !semi {
                        value = inner_value;
                    }
                }
                StmtKind::Empty => {}
                _ => return Err(ConstFault::refused(expr, ConstantForm::StatementInConstExpression)),
            }
        }
        Ok(value)
    }

    /// The value `local` holds after the statements `stmts` run at elaboration over `bindings` (a node chosen by a `let` its body
    /// assigns, §1.4.7). A statement that does not fold (a contribution, a `let` of a probe) runs no further: its `let` binds
    /// nothing and every local it assigns is forgotten, so a later read of one does not fold either. `None` when `local` has no scalar value
    /// then.
    pub(in crate::elab) fn local_after(&mut self, at: ExprId, stmts: &[StmtId], local: DeclId, bindings: &Bindings) -> Option<Constant> {
        let outer = std::mem::take(&mut self.locals);
        let outer_functions = self.fn_values.clone();
        for stmt in stmts {
            if self.exec_block(at, &[*stmt], bindings).is_err() {
                self.leaving = None;
                for written in self.written_by(*stmt) {
                    self.locals.remove(&written);
                }
            }
        }
        let value = self.locals.get(&local).copied().filter(|value| self.composite_of(*value).is_none());
        self.locals = outer;
        self.fn_values = outer_functions;
        value
    }

    /// The value of `expr` over `bindings` with locals of its own: what it assigns (`{ s = !s; s }`) is gone once it is folded.
    pub(in crate::elab) fn value_in_a_scope_of_its_own(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let outer = std::mem::take(&mut self.locals);
        let value = self.value(expr, bindings);
        self.locals = outer;
        value
    }

    /// The locals the statement `stmt` assigns, anywhere in it.
    fn written_by(&self, stmt: StmtId) -> Vec<DeclId> {
        let mut exprs = Vec::new();
        for child in crate::pom::BodyWalk::new(self.design).statement_children(stmt) {
            crate::pom::BodyWalk::new(self.design).descendants_depth_first(child, &mut exprs);
        }
        exprs
            .into_iter()
            .filter_map(|expr| match &self.design.expr_node(expr).kind {
                ExprKind::Assign { lhs, .. } => self.root_local(*lhs),
                _ => None,
            })
            .collect()
    }

    /// The local a place is rooted in: `x` of `x`, `x.f` and `x[i]`.
    fn root_local(&self, place: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(self.peel(place)).kind {
            ExprKind::Field { expr, .. } | ExprKind::Index { expr, .. } => self.root_local(*expr),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(local)) => Some(local),
                _ => None,
            },
            _ => None,
        }
    }

    /// `let pat = init;`, with `else { .. }` when the pattern may not match (§1.7.5, `let .. else`).
    fn exec_let(&mut self, expr: ExprId, pat: PatId, init: ExprId, otherwise: Option<&BlockNode>, bindings: &Bindings) -> Result<(), ConstFault> {
        if let Some(function) = self.fn_value_of(init)
            && let PatKind::Ident { binding, sub: None, .. } = self.design.pat_node(pat).kind
        {
            self.fn_values.insert(binding, function);
            return Ok(());
        }
        let value = self.fold(init, bindings)?;
        if self.matches(pat, value, init, bindings)? {
            return Ok(());
        }
        match otherwise {
            Some(block) => {
                // the `else` diverges: its exit is the fault that leaves the enclosing form
                self.exec_block(expr, &block.stmts, bindings)?;
                Err(ConstFault::refused(expr, ConstantForm::LetElseWhoseElseDoesNotLeave))
            }
            None => Err(ConstFault::refused(expr, ConstantForm::RefutablePatternInLetWithoutElse)),
        }
    }

    // ---- decisions ---------------------------------------------------------------------------

    /// A condition: a `bool`, or `let pat = e` (which binds when it matches).
    fn condition(&mut self, cond: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        if let ExprKind::Let { pat, expr } = self.design.expr_node(cond).kind.clone() {
            let value = self.fold(expr, bindings)?;
            return self.matches(pat, value, expr, bindings);
        }
        let value = self.fold(cond, bindings)?;
        Self::boolean(cond, value)
    }

    /// `if cond { then } else { else_ }`: the arm the condition takes.
    fn exec_if(&mut self, expr: ExprId, cond: ExprId, then: &BlockNode, else_: Option<ExprId>, bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        if self.condition(cond, bindings)? {
            return self.exec_block(expr, &then.stmts, bindings);
        }
        match else_ {
            Some(otherwise) => self.exec(otherwise, bindings),
            None => Ok(None),
        }
    }

    /// `match scrutinee { arms }`: the first arm whose pattern matches and whose guard holds.
    fn exec_match(&mut self, expr: ExprId, scrutinee: ExprId, arms: &[Arm], bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        let subject = self.fold(scrutinee, bindings)?;
        for arm in arms {
            if !self.matches(arm.pat, subject, scrutinee, bindings)? {
                continue;
            }
            if let Some(guard) = arm.guard {
                let held = self.fold(guard, bindings)?;
                if !Self::boolean(guard, held)? {
                    continue;
                }
            }
            return self.exec(arm.body, bindings);
        }
        Err(ConstFault::refused(expr, ConstantForm::MatchNoArmOfWhichMatches))
    }

    // ---- loops -------------------------------------------------------------------------------

    /// One run of a loop body: it goes on (also after a `continue`) or it ends with the value of
    /// its `break`.
    fn iterate(&mut self, expr: ExprId, body: &BlockNode, trips: &mut u64, bindings: &Bindings) -> Result<Flow, ConstFault> {
        *trips += 1;
        if *trips > TRIPS {
            return Err(ConstFault::Panic { expr, what: "a loop that runs more than 2^24 iterations (§1 rule 11)".to_string() });
        }
        match self.exec_block(expr, &body.stmts, bindings) {
            Ok(_) | Err(ConstFault::Leaves { exit: Exit::Continue, .. }) => Ok(Flow::Next),
            Err(ConstFault::Leaves { exit: Exit::Break, .. }) => Ok(Flow::Done(self.leaving.take())),
            Err(fault) => Err(fault),
        }
    }

    /// `while cond { body }` and `while let pat = e { body }`: of no value.
    fn exec_while(&mut self, expr: ExprId, cond: ExprId, body: &BlockNode, bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        let mut trips = 0;
        loop {
            if !self.condition(cond, bindings)? {
                return Ok(None);
            }
            if let Flow::Done(value) = self.iterate(expr, body, &mut trips, bindings)? {
                return Ok(value);
            }
        }
    }

    /// `loop { body }`: its value is what its `break` carries.
    fn exec_loop(&mut self, expr: ExprId, body: &BlockNode, bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        let mut trips = 0;
        loop {
            if let Flow::Done(value) = self.iterate(expr, body, &mut trips, bindings)? {
                return Ok(value);
            }
        }
    }

    /// `for k in low..high { body }` and `..=`: the variable is the counter of each iteration; of no
    /// value. A `for` over anything but a range of integers is refused by name.
    fn exec_for(&mut self, expr: ExprId, pat: PatId, iter: ExprId, body: &BlockNode, bindings: &Bindings) -> Result<Option<Constant>, ConstFault> {
        let iter = self.peel(iter);
        let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.design.expr_node(iter).kind else {
            // an array, a named range or an iterator chain: its elements in order (`composite.rs`)
            let sequence = self.fold(iter, bindings)?;
            let elements = self.elements_of(iter, sequence)?;
            let mut trips = 0;
            for element in elements {
                self.matches(pat, element, iter, bindings)?;
                if let Flow::Done(value) = self.iterate(expr, body, &mut trips, bindings)? {
                    return Ok(value);
                }
            }
            return Ok(None);
        };
        let (Constant::Int(first), Constant::Int(last)) = (self.fold(lo, bindings)?, self.fold(hi, bindings)?) else {
            return Err(ConstFault::refused(iter, ConstantForm::ForRangeWhoseBoundsAreNotIntegers));
        };
        let end = if inclusive { last + 1 } else { last };
        let mut trips = 0;
        for value in first..end {
            self.matches(pat, Constant::Int(value), iter, bindings)?;
            if let Flow::Done(value) = self.iterate(expr, body, &mut trips, bindings)? {
                return Ok(value);
            }
        }
        Ok(None)
    }

    /// `place = value;` and `place op= value;` over a local: the local takes the value (§1.5.12). A
    /// compound operator reads the local and wraps as the type of the local does.
    fn exec_assign(&mut self, expr: ExprId, op: Option<crate::pom::BinOp>, lhs: ExprId, rhs: ExprId, bindings: &Bindings) -> Result<(), ConstFault> {
        let target = self.peel(lhs);
        if let Some(result) = self.assign_into(expr, target, op, rhs, bindings) {
            return result;
        }
        let ExprKind::Path(path) = &self.design.expr_node(target).kind else {
            return Err(ConstFault::refused(expr, ConstantForm::AssignmentToSomethingThatIsNotLocal));
        };
        let Some(Resolution::Decl(local)) = self.design.path_node(*path).segments.last().map(|s| s.res) else {
            return Err(ConstFault::refused(expr, ValueForm::AssignmentToNameThatIsNotLocal));
        };
        let value = self.fold(rhs, bindings)?;
        let assigned = match op {
            None => value,
            Some(op) => {
                let current = self.fold(target, bindings)?;
                self.known_binary(target, op, (target, current), (rhs, value))?
            }
        };
        self.locals.insert(local, assigned);
        Ok(())
    }

    // ---- patterns ----------------------------------------------------------------------------

    /// Whether `value`, the value of `at`, matches `pat`; a match binds the names of the pattern
    /// into the locals.
    pub(super) fn matches(&mut self, pat: PatId, value: Constant, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        if let Some(result) = self.matches_composite(pat, value, at, bindings) {
            return result;
        }
        let kind = self.design.pat_node(pat).kind.clone();
        match &kind {
            PatKind::Wild => Ok(true),
            PatKind::Ident { res, sub, binding, .. } => match self.named_constant(*res) {
                Some(named) => Ok(self.equals(value, named)),
                None => {
                    if let Some(sub) = sub
                        && !self.matches(*sub, value, at, bindings)?
                    {
                        return Ok(false);
                    }
                    self.locals.insert(*binding, value);
                    Ok(true)
                }
            },
            PatKind::Lit(lit) => {
                let named = self.fold(*lit, bindings)?;
                Ok(self.equals(value, named))
            }
            PatKind::Path(path) => match self.named_constant(self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res)) {
                Some(named) => Ok(self.equals(value, named)),
                None => Err(ConstFault::refused(at, ConstantForm::PathPatternThatIsNotConstantOr)),
            },
            PatKind::Or(alternatives) => {
                for alternative in alternatives {
                    if self.matches(*alternative, value, at, bindings)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            PatKind::Range { lo, hi, inclusive } => {
                let subject = value;
                let above = match lo {
                    Some(lo) => Self::order(subject, self.fold(*lo, bindings)?).map(|o| o.is_ge()),
                    None => Some(true),
                };
                let below = match hi {
                    Some(hi) => Self::order(subject, self.fold(*hi, bindings)?).map(|o| if *inclusive { o.is_le() } else { o.is_lt() }),
                    None => Some(true),
                };
                match (above, below) {
                    (Some(above), Some(below)) => Ok(above && below),
                    _ => Err(ConstFault::refused(at, ConstantForm::RangePatternOverValuesThatAreNot)),
                }
            }
            _ => Err(ConstFault::refused(at, ConstantForm::PatternInConstExpression)),
        }
    }

    /// Whether `value` is the constant `named`.
    fn equals(&self, value: Constant, named: Constant) -> bool {
        value == named
    }

    /// `a` against `b` for two integers or two floats.
    pub(super) fn order(a: Constant, b: Constant) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Constant::Int(a), Constant::Int(b)) => Some(a.cmp(&b)),
            (Constant::Real(a), Constant::Real(b)) => a.get().partial_cmp(&b.get()),
            _ => None,
        }
    }

    /// The value of a `const` or a unit variant (its tag) a name resolves to; `None` for any other.
    fn named_constant(&mut self, res: Resolution) -> Option<Constant> {
        let Resolution::Decl(decl) = res else { return None };
        match self.design.decl_data(decl).kind {
            DeclKind::Const(_) => self.constant(decl).ok(),
            DeclKind::Variant(_) => super::EnumLayout::variant_tag(self.design, decl).map(Constant::Int),
            _ => None,
        }
    }

    /// `expr` without the parentheses around it.
    fn peel(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    // ---- function values ---------------------------------------------------------------------

    /// The function `expr` is when it is one: a closure (capturing the locals as they are now), the
    /// name of a `fn`, or a local that holds a function.
    pub(super) fn fn_value_of(&mut self, expr: ExprId) -> Option<FnValue> {
        let expr = self.peel(expr);
        match &self.design.expr_node(expr).kind {
            ExprKind::Closure { params, body, .. } => Some(FnValue::Closure { params: params.iter().map(|p| p.pat).collect(), body: *body, captured: self.locals.clone(), functions: self.fn_values.clone() }),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => match self.design.decl_data(decl).kind {
                    DeclKind::Fn(_) => Some(FnValue::Named(decl)),
                    DeclKind::Binding(_) | DeclKind::FnParam(_) => self.fn_values.get(&decl).cloned(),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    /// The function a call `expr` calls through a local, when its callee is one.
    pub(super) fn called_function_value(&mut self, expr: ExprId) -> Option<FnValue> {
        let ExprKind::Call { callee, .. } = &self.design.expr_node(expr).kind else { return None };
        self.fn_value_of(*callee)
    }

    /// A call of the function value `function` with `arguments`: a `fn` as any call of it, a
    /// closure with its captured locals and its parameters bound. `return` in a closure returns from
    /// it.
    pub(super) fn apply_value(&mut self, expr: ExprId, function: FnValue, arguments: Vec<Argument>, bindings: &Bindings) -> Result<Constant, ConstFault> {
        match function {
            FnValue::Named(decl) => self.apply_function(expr, decl, arguments),
            FnValue::Closure { params, body, captured, functions } => {
                self.enter(expr)?;
                let outer = std::mem::replace(&mut self.locals, captured);
                let outer_functions = std::mem::replace(&mut self.fn_values, functions);
                let bound = self.bind_parameters(expr, &params, arguments, bindings);
                let result = match bound {
                    Ok(()) => crate::stack::Stack::deep(|| self.exec(body, bindings)),
                    Err(fault) => Err(fault),
                };
                self.depth -= 1;
                self.locals = outer;
                self.fn_values = outer_functions;
                self.returned(expr, result)
            }
        }
    }

    /// The parameters of a closure bound to its arguments, a function argument as a function value.
    fn bind_parameters(&mut self, expr: ExprId, params: &[PatId], arguments: Vec<Argument>, bindings: &Bindings) -> Result<(), ConstFault> {
        for (pat, argument) in params.iter().zip(arguments) {
            match (argument.function, self.design.pat_node(*pat).kind.clone()) {
                (Some(function), PatKind::Ident { binding, sub: None, .. }) => {
                    self.fn_values.insert(binding, function);
                }
                (Some(_), _) => return Err(ConstFault::refused(expr, ConstantForm::FunctionBoundByPatternThatIsNot)),
                (None, _) => {
                    let Some(value) = argument.value else { return Err(ConstFault::refused(expr, ConstantForm::ArgumentWithNeitherValueNorFunction)) };
                    if !self.matches(*pat, value, expr, bindings)? {
                        return Err(ConstFault::refused(expr, ConstantForm::ClosureParameterPatternThatDoesNotMatch));
                    }
                }
            }
        }
        Ok(())
    }

    /// Opens one more nested call: a panic past 1 024 (§1 rule 11).
    pub(super) fn enter(&mut self, expr: ExprId) -> Result<(), ConstFault> {
        if self.depth >= super::CALL_DEPTH {
            return Err(ConstFault::Panic { expr, what: "a call nested deeper than 1 024 (§1 rule 11)".to_string() });
        }
        self.depth += 1;
        Ok(())
    }

    /// The value a function or closure body gave: what it evaluated to, or what its `return`
    /// carried.
    pub(super) fn returned(&mut self, expr: ExprId, result: Result<Option<Constant>, ConstFault>) -> Result<Constant, ConstFault> {
        let value = match result {
            Ok(value) => value,
            Err(ConstFault::Leaves { exit: Exit::Return, .. }) => self.leaving.take(),
            Err(fault) => return Err(fault),
        };
        value.ok_or(ConstFault::refused(expr, ConstantForm::FunctionBodyWithNoValue))
    }
}
