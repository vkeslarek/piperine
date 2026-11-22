//! The forms of phase A in a const context (CV-36; part I §1.5.12, §1.7 to §1.8, §3): `let mut`,
//! assignment, `while`, `loop` and `for` over a range, `break`, `continue`, `return`, the patterns
//! of `match`, `if let`, `while let` and `let .. else`, closures, function values and recursion
//! evaluate at elaboration to the value they have at run time.
//!
//! A statement evaluates to a value or to nothing (`exec`): `()` is no [`Folded`]. An exit is an
//! `Err(ConstFault::Leaves)` the loop or function it leaves catches, so every `?` between the exit
//! and its target unwinds as it would in the source; the value a `break` or `return` carries
//! waits in `ConstEval::leaving`. A loop that runs more than 2^24 iterations and a call nested more
//! than 1 024 deep are panics (§1 rule 11, Q5): `E0104` at elaboration.
//!
//! A decision that is unknown (§3.3) is *not* taken: a loop or a `match` over one is
//! [`ConstFault::Unknown`] (§3.3.8: structure is decided by known values), an `if` on one runs both
//! arms and merges the locals they write and the value they give (§3.3.3), and an exit under one is
//! refused by name.

use std::collections::BTreeMap;

use piperine_ir::Constant;

use super::{Bindings, ConstEval, ConstFault, Exit, Folded};
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
    Closure { params: Vec<PatId>, body: ExprId, captured: BTreeMap<DeclId, Folded>, functions: BTreeMap<DeclId, FnValue> },
}

/// One argument of a call: its value, or the function it stands for.
pub(super) struct Argument {
    /// The value; `Unknown` for a function argument, which has none.
    pub(super) value: Folded,
    /// The function the argument is, when it is one.
    pub(super) function: Option<FnValue>,
}

/// Whether a condition holds, is false, or is unknown.
enum Truth {
    Holds,
    Fails,
    Unknown,
}

/// What a loop's body did: the loop goes on, or it ends with a value.
enum Flow {
    Next,
    Done(Option<Folded>),
}

impl ConstEval<'_, '_> {
    /// `expr` evaluated as a statement or a value: `Some` value, or `None` for `()`.
    pub(super) fn exec(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
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
    pub(super) fn exec_value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        self.exec(expr, bindings)?.ok_or(ConstFault::Unsupported { expr, what: "a form with no value where a value is needed".to_string() })
    }

    /// `break`, `continue` or `return`, with its value evaluated: the fault the loop or function it
    /// leaves catches.
    fn exit(&mut self, expr: ExprId, exit: Exit, value: Option<ExprId>, bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
        self.leaving = match value {
            Some(value) => Some(self.fold(value, bindings)?),
            None => None,
        };
        Err(ConstFault::Leaves { expr, exit })
    }

    /// The statements of a block: the value of the last one when it is an expression without `;`,
    /// else `None`. A `let` binds into the locals, a function value into the function values.
    pub(super) fn exec_block(&mut self, expr: ExprId, stmts: &[StmtId], bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
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
                _ => return Err(ConstFault::Unsupported { expr, what: "this statement in a const expression".to_string() }),
            }
        }
        Ok(value)
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
                Err(ConstFault::Unsupported { expr, what: "a `let .. else` whose `else` does not leave".to_string() })
            }
            None => Err(ConstFault::Unsupported { expr, what: "a refutable pattern in a `let` without `else`".to_string() }),
        }
    }

    // ---- decisions ---------------------------------------------------------------------------

    /// A condition: a `bool`, or `let pat = e` (which binds when it matches).
    fn condition(&mut self, cond: ExprId, bindings: &Bindings) -> Result<Truth, ConstFault> {
        if let ExprKind::Let { pat, expr } = self.design.expr_node(cond).kind.clone() {
            let value = self.fold(expr, bindings)?;
            if value == Folded::Unknown && !matches!(self.design.pat_node(pat).kind, PatKind::Wild) {
                return Ok(Truth::Unknown);
            }
            return Ok(if self.matches(pat, value, expr, bindings)? { Truth::Holds } else { Truth::Fails });
        }
        match self.fold(cond, bindings)? {
            Folded::Known(known) => Ok(if Self::boolean(cond, known)? { Truth::Holds } else { Truth::Fails }),
            Folded::Unknown => Ok(Truth::Unknown),
        }
    }

    /// `if cond { then } else { else_ }`. A known condition takes its arm; an unknown one runs both
    /// and merges what they write and give (§3.3.3); a panic in an arm makes every local and the
    /// value unknown (§1 rule 13).
    fn exec_if(&mut self, expr: ExprId, cond: ExprId, then: &BlockNode, else_: Option<ExprId>, bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
        match self.condition(cond, bindings)? {
            Truth::Holds => self.exec_block(expr, &then.stmts, bindings),
            Truth::Fails => match else_ {
                Some(otherwise) => self.exec(otherwise, bindings),
                None => Ok(None),
            },
            Truth::Unknown => {
                let before = self.locals.clone();
                let first = self.exec_block(expr, &then.stmts, bindings);
                let after_first = std::mem::replace(&mut self.locals, before.clone());
                let second = match else_ {
                    Some(otherwise) => self.exec(otherwise, bindings),
                    None => Ok(None),
                };
                let after_second = std::mem::replace(&mut self.locals, before);
                for result in [&first, &second] {
                    if let Err(ConstFault::Leaves { expr, .. }) = result {
                        return Err(ConstFault::Unsupported { expr: *expr, what: "an exit under an unknown decision in a const expression".to_string() });
                    }
                }
                match (first, second) {
                    (Ok(a), Ok(b)) => {
                        self.merge_locals(&after_first, &after_second);
                        match (a, b) {
                            (Some(a), Some(b)) => Ok(Some(a.merge(b))),
                            (None, None) => Ok(None),
                            _ => Err(ConstFault::Unsupported { expr, what: "an `if` whose arms differ in having a value".to_string() }),
                        }
                    }
                    (Err(ConstFault::Panic { .. }), Ok(_) | Err(ConstFault::Panic { .. })) | (Ok(_), Err(ConstFault::Panic { .. })) => {
                        for value in self.locals.values_mut() {
                            *value = Folded::Unknown;
                        }
                        Ok(Some(Folded::Unknown))
                    }
                    (Err(fault), _) | (_, Err(fault)) => Err(fault),
                }
            }
        }
    }

    /// The locals after an `if` on an unknown condition: what both arms left, merged; a local one
    /// arm bound and the other did not is the one arm's.
    fn merge_locals(&mut self, first: &BTreeMap<DeclId, Folded>, second: &BTreeMap<DeclId, Folded>) {
        for (decl, a) in first {
            let merged = match second.get(decl) {
                Some(b) => a.merge(*b),
                None => *a,
            };
            self.locals.insert(*decl, merged);
        }
        for (decl, b) in second {
            self.locals.entry(*decl).or_insert(*b);
        }
    }

    /// `match scrutinee { arms }`: the first arm whose pattern matches and whose guard holds. The
    /// scrutinee and the guards must be known (§3.3.8).
    fn exec_match(&mut self, expr: ExprId, scrutinee: ExprId, arms: &[Arm], bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
        let subject = self.fold(scrutinee, bindings)?;
        for arm in arms {
            if !self.matches(arm.pat, subject, scrutinee, bindings)? {
                continue;
            }
            if let Some(guard) = arm.guard {
                match self.fold(guard, bindings)? {
                    Folded::Known(known) if Self::boolean(guard, known)? => {}
                    Folded::Known(_) => continue,
                    Folded::Unknown => return Err(ConstFault::Unknown { expr: guard }),
                }
            }
            return self.exec(arm.body, bindings);
        }
        Err(ConstFault::Unsupported { expr, what: "a `match` no arm of which matches".to_string() })
    }

    // ---- loops -------------------------------------------------------------------------------

    /// One run of a loop body: it goes on (also after a `continue`) or it ends with the value of
    /// its `break`. An unknown loop condition is a fault, not a decision.
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
    fn exec_while(&mut self, expr: ExprId, cond: ExprId, body: &BlockNode, bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
        let mut trips = 0;
        loop {
            match self.condition(cond, bindings)? {
                Truth::Holds => {}
                Truth::Fails => return Ok(None),
                Truth::Unknown => return Err(ConstFault::Unknown { expr: cond }),
            }
            if let Flow::Done(value) = self.iterate(expr, body, &mut trips, bindings)? {
                return Ok(value);
            }
        }
    }

    /// `loop { body }`: its value is what its `break` carries.
    fn exec_loop(&mut self, expr: ExprId, body: &BlockNode, bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
        let mut trips = 0;
        loop {
            if let Flow::Done(value) = self.iterate(expr, body, &mut trips, bindings)? {
                return Ok(value);
            }
        }
    }

    /// `for k in low..high { body }` and `..=`: the variable is the counter of each iteration; of no
    /// value. A `for` over anything but a range of integers is refused by name.
    fn exec_for(&mut self, expr: ExprId, pat: PatId, iter: ExprId, body: &BlockNode, bindings: &Bindings) -> Result<Option<Folded>, ConstFault> {
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
        let (Folded::Known(Constant::Int(first)), Folded::Known(Constant::Int(last))) = (self.fold(lo, bindings)?, self.fold(hi, bindings)?) else {
            return Err(ConstFault::Unknown { expr: iter });
        };
        let end = if inclusive { last + 1 } else { last };
        let mut trips = 0;
        for value in first..end {
            self.matches(pat, Folded::Known(Constant::Int(value)), iter, bindings)?;
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
            return Err(ConstFault::Unsupported { expr, what: "an assignment to something that is not a local, a field or an element".to_string() });
        };
        let Some(Resolution::Decl(local)) = self.design.path_node(*path).segments.last().map(|s| s.res) else {
            return Err(ConstFault::Unsupported { expr, what: "an assignment to a name that is not a local".to_string() });
        };
        let value = self.fold(rhs, bindings)?;
        let assigned = match op {
            None => value,
            Some(op) => {
                let current = self.fold(target, bindings)?;
                match (current, value) {
                    (Folded::Known(a), Folded::Known(b)) => Folded::Known(self.known_binary(target, op, (target, a), (rhs, b))?),
                    (a, b) => self.unknown_binary(target, op, a, b),
                }
            }
        };
        self.locals.insert(local, assigned);
        Ok(())
    }

    // ---- patterns ----------------------------------------------------------------------------

    /// Whether `value`, the value of `at`, matches `pat`; a match binds the names of the pattern
    /// into the locals. A refutable pattern over an unknown value is a fault (§3.3.8).
    pub(super) fn matches(&mut self, pat: PatId, value: Folded, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        if let Some(result) = self.matches_composite(pat, value, at, bindings) {
            return result;
        }
        let kind = self.design.pat_node(pat).kind.clone();
        match &kind {
            PatKind::Wild => Ok(true),
            PatKind::Ident { res, sub, binding, .. } => match self.named_constant(*res) {
                Some(named) => self.equals(value, named, at),
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
                let named = self.known(*lit, bindings)?;
                self.equals(value, named, at)
            }
            PatKind::Path(path) => match self.named_constant(self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res)) {
                Some(named) => self.equals(value, named, at),
                None => Err(ConstFault::Unsupported { expr: at, what: "a path pattern that is not a constant or a unit variant".to_string() }),
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
                let Folded::Known(subject) = value else { return Err(ConstFault::Unknown { expr: at }) };
                let above = match lo {
                    Some(lo) => Self::order(subject, self.known(*lo, bindings)?).map(|o| o.is_ge()),
                    None => Some(true),
                };
                let below = match hi {
                    Some(hi) => Self::order(subject, self.known(*hi, bindings)?).map(|o| if *inclusive { o.is_le() } else { o.is_lt() }),
                    None => Some(true),
                };
                match (above, below) {
                    (Some(above), Some(below)) => Ok(above && below),
                    _ => Err(ConstFault::Unsupported { expr: at, what: "a range pattern over values that are not both integers or both floats".to_string() }),
                }
            }
            _ => Err(ConstFault::Unsupported { expr: at, what: "this pattern in a const expression".to_string() }),
        }
    }

    /// Whether `value` is the constant `named`; a fault when `value` is unknown.
    fn equals(&self, value: Folded, named: Constant, at: ExprId) -> Result<bool, ConstFault> {
        match value {
            Folded::Known(known) => Ok(known == named),
            Folded::Unknown => Err(ConstFault::Unknown { expr: at }),
        }
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
    pub(super) fn apply_value(&mut self, expr: ExprId, function: FnValue, arguments: Vec<Argument>, bindings: &Bindings) -> Result<Folded, ConstFault> {
        match function {
            FnValue::Named(decl) => self.apply_function(expr, decl, arguments),
            FnValue::Closure { params, body, captured, functions } => {
                self.enter(expr)?;
                let outer = std::mem::replace(&mut self.locals, captured);
                let outer_functions = std::mem::replace(&mut self.fn_values, functions);
                let bound = self.bind_parameters(expr, &params, arguments, bindings);
                let result = match bound {
                    Ok(()) => crate::stack::deep(|| self.exec(body, bindings)),
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
                (Some(_), _) => return Err(ConstFault::Unsupported { expr, what: "a function bound by a pattern that is not a name".to_string() }),
                (None, _) => {
                    if !self.matches(*pat, argument.value, expr, bindings)? {
                        return Err(ConstFault::Unsupported { expr, what: "a closure parameter pattern that does not match its argument".to_string() });
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
    pub(super) fn returned(&mut self, expr: ExprId, result: Result<Option<Folded>, ConstFault>) -> Result<Folded, ConstFault> {
        let value = match result {
            Ok(value) => value,
            Err(ConstFault::Leaves { exit: Exit::Return, .. }) => self.leaving.take(),
            Err(fault) => return Err(fault),
        };
        value.ok_or(ConstFault::Unsupported { expr, what: "a function body with no value".to_string() })
    }
}
