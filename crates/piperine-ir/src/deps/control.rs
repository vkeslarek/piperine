//! The dependency walk over mutation, loops and exits (composite-values CV-01, CV-02): the
//! methods of [`Walk`] that give an assignment, a loop and a `break` their abstract value.
//!
//! The lattice stays finite, so every fixed point here ends:
//!
//! - an **assignment** joins the assigned value (and the conditions it sits under) into the
//!   local's, so a local only grows;
//! - a **loop** body is walked again from the locals the last pass left until no local changes,
//!   so a read late in the body sees an assignment made later in it on the next iteration; the
//!   bounds of a range, the condition of a `while` and the conditions a `break` or `continue`
//!   sits under then reach every local the loop assigned, since they decide how often it ran.

use std::collections::BTreeSet;

use super::{Class, Sources, Task, Value, Walk};
use crate::ids::ExprId;
use crate::model::{BinaryOp, Expr, ValueType};

/// One loop being walked: what decides its trip count and which locals its body assigns.
pub(super) struct LoopFrame {
    id: ExprId,
    /// The depth of `Walk::scopes` the loop sits at: an assignment names a local of this owner
    /// only when it is walked at the same depth.
    depth: usize,
    /// How many of `Walk::enclosing`'s conditions held when the loop began: an exit reads only
    /// the conditions it sits under inside the loop.
    outer_conditions: usize,
    /// What decides how many times the body runs: the bounds of a range, the condition of a
    /// `while`, and the conditions a `break` or a `continue` sits under.
    exits: Sources,
    /// The join of the values the loop's `break`s carry.
    broken: Option<Value>,
    /// The value of a range's variable: its bounds joined.
    variable: Value,
    /// The locals of the loop's owner that the body assigns.
    assigned: BTreeSet<u32>,
}

impl LoopFrame {
    /// The frame of loop `id`, entered at scope depth `depth` under `outer_conditions` conditions.
    fn new(id: ExprId, depth: usize, outer_conditions: usize) -> Self {
        LoopFrame { id, depth, outer_conditions, exits: Sources::default(), broken: None, variable: Value::constant(), assigned: BTreeSet::new() }
    }
}

impl Value {
    /// The least value above `self` and `other`: the sources of both, the larger class.
    fn join(&mut self, other: &Value) {
        self.sources.join(&other.sources);
        self.class = self.class.max(other.class);
    }

    /// `self`, read under `conditions`: it also reads what they read, and is no longer affine
    /// when one is not `Constant`, as a contribution under an `if` is not (a switch).
    fn under(&mut self, conditions: &[Value]) {
        for condition in conditions {
            self.sources.join(&condition.sources);
            if condition.class != Class::Constant {
                self.class = Class::Nonlinear;
            }
        }
    }
}

impl<'a> Walk<'a> {
    /// Whether a form's walk assigns a local, directly or in a part: its value is not memoized,
    /// since a memoized value would hide the assignment on the next visit.
    pub(super) fn assigns(kind: &Expr) -> bool {
        match kind {
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::If { .. } => true,
            Expr::Block { stmts, .. } => !stmts.is_empty(),
            _ => false,
        }
    }

    /// A block, a loop pass or a closure ends: the locals it bound (`outside` holds none of them)
    /// go out of scope, while a local bound before it keeps what the code assigned to it.
    pub(super) fn restore(&mut self, outside: Vec<Option<Value>>) {
        let scope = self.scope();
        let kept: Vec<Option<Value>> = scope.locals.iter().zip(outside).map(|(now, before)| before.and(now.clone())).collect();
        if scope.locals != kept {
            scope.memo.clear();
            scope.locals = kept;
        }
    }

    /// `local = value` or `local op= value` at `at`, the value on the stack: the local's value
    /// joins the assigned one, and the conditions the assignment sits under inside this owner.
    pub(super) fn assign(&mut self, local: u32, op: Option<BinaryOp>, at: ExprId) {
        let assigned = self.pop();
        let old = match self.scope().locals.get(local as usize) {
            Some(Some(value)) => value.clone(),
            Some(None) => self.refuse(at, &format!("local {local} assigned before its `let`")),
            None => self.refuse(at, &format!("local {local} out of range")),
        };
        let mut value = match op {
            None => assigned,
            Some(op) => Walk::binary(op, old.clone(), assigned),
        };
        value.join(&old);
        let from = self.scope().enclosed_from;
        let conditions = self.enclosing.get(from..).unwrap_or_default().to_vec();
        value.under(&conditions);
        let depth = self.scopes.len();
        for frame in self.frames.iter_mut().filter(|frame| frame.depth == depth) {
            frame.assigned.insert(local);
        }
        self.bind(local, value, at);
    }

    /// The loop `id` begins: its frame opens, a range's bounds are walked, then the first pass.
    pub(super) fn begin_loop(&mut self, id: ExprId, next: &mut Vec<Task>) {
        self.frames.push(LoopFrame::new(id, self.scopes.len(), self.enclosing.len()));
        if let Expr::ForRange { low, high, .. } = &self.ir.exprs[id.index()].kind {
            next.extend([Task::Expr(*low), Task::Expr(*high), Task::LoopBounds(id)]);
        }
        next.push(Task::LoopPass { id, before: None });
    }

    /// The frame of loop `id`; it is open while the loop's body is walked.
    fn frame(&mut self, id: ExprId) -> &mut LoopFrame {
        match self.frames.iter().rposition(|frame| frame.id == id) {
            Some(position) => &mut self.frames[position],
            None => self.refuse(id, "a loop walked without its frame"),
        }
    }

    /// The bounds of the range of loop `id` are on the stack: they decide the trip count and are
    /// the value of its variable.
    pub(super) fn loop_bounds(&mut self, id: ExprId) {
        let high = self.pop();
        let low = self.pop();
        let variable = Value::nonlinear([&low, &high]);
        let frame = self.frame(id);
        frame.exits.join(&variable.sources);
        frame.variable = variable;
    }

    /// The condition of the `while` `id` is on the stack: it decides the trip count, and a
    /// condition that may be unknown is a decision on an unknown value (§3.3.3).
    pub(super) fn loop_condition(&mut self, id: ExprId) {
        let condition = self.pop();
        let unknown = match &self.ir.exprs[id.index()].kind {
            Expr::While { cond, .. } => matches!(self.ir.exprs[cond.index()].ty, ValueType::Flagged(_)),
            _ => false,
        };
        let frame = self.frame(id);
        frame.exits.join(&condition.sources);
        frame.exits.unknown_control |= unknown;
    }

    /// A pass over the body of `id` ended with the locals `before` held at its start (`None`
    /// before the first): walk the body again while a local changed, else the loop is over.
    pub(super) fn loop_pass(&mut self, id: ExprId, before: Option<Vec<Option<Value>>>, next: &mut Vec<Task>) {
        match before {
            Some(before) if self.scope().locals == before => self.end_loop(id),
            _ => self.start_pass(id, next),
        }
    }

    /// Schedules one more pass over the body of `id`, from the locals as they stand.
    fn start_pass(&mut self, id: ExprId, next: &mut Vec<Task>) {
        let before = self.scope().locals.clone();
        // A value memoized in the last pass read the locals as that pass left them.
        self.scope().memo.clear();
        let ir = self.ir;
        let kind = &ir.exprs[id.index()].kind;
        match kind {
            Expr::ForRange { local, .. } => {
                let variable = self.frame(id).variable.clone();
                self.bind(*local, variable, id);
            }
            Expr::While { cond, .. } => next.extend([Task::Expr(*cond), Task::LoopCondition(id)]),
            _ => {}
        }
        next.extend(kind.statements().iter().map(|&stmt| Task::Stmt(stmt)));
        next.extend([Task::Restore(before.clone()), Task::LoopPass { id, before: Some(before) }]);
    }

    /// No local changed in the last pass: the frame closes. What decided the trip count reaches
    /// every local the body assigned and the loop's own value, which also joins its `break`s.
    fn end_loop(&mut self, id: ExprId) {
        let frame = match self.frames.pop() {
            Some(frame) if frame.id == id => frame,
            _ => self.refuse(id, "a loop closed out of order"),
        };
        for &local in &frame.assigned {
            let Some(Some(mut value)) = self.scope().locals.get(local as usize).cloned() else { continue };
            value.sources.join(&frame.exits);
            self.bind(local, value, id);
        }
        let mut value = frame.broken.unwrap_or_else(Value::constant);
        value.sources.join(&frame.exits);
        self.done(id, value);
    }

    /// A `break` or `continue` at `at` (its value on the stack when `valued`) leaves the innermost
    /// loop of this owner: the conditions it sits under decide the trip count, and a value joins
    /// the loop's. The expression itself yields nothing (it never completes).
    pub(super) fn escape(&mut self, at: ExprId, valued: bool) {
        let value = valued.then(|| self.pop());
        let depth = self.scopes.len();
        let Some(position) = self.frames.iter().rposition(|frame| frame.depth == depth) else {
            self.refuse(at, "a `break` or `continue` outside a loop");
        };
        let from = self.frames[position].outer_conditions;
        let conditions: Vec<Sources> = self.enclosing.get(from..).unwrap_or_default().iter().map(|held| held.sources.clone()).collect();
        let frame = &mut self.frames[position];
        for sources in &conditions {
            frame.exits.join(sources);
        }
        if let Some(value) = value {
            match &mut frame.broken {
                Some(broken) => broken.join(&value),
                None => frame.broken = Some(value),
            }
        }
        self.values.push(Value::constant());
    }
}
