//! One abstract walk of a body: its tasks, its scopes and the loop that runs them. The forms the
//! walk gives a meaning to live in the files below, one concern each.

use std::collections::{HashMap, HashSet};

use super::{Class, Sources, Value};
use crate::ids::{ExprId, FunctionId, StmtId};
use crate::model::{BinaryOp, Expr, Ir, Operator};

mod calls;
mod control;
mod expressions;
mod finish;
mod roots;
mod statements;

use control::LoopFrame;

/// The locals of one owner (a body's `analog` block or a function), bound as the walk passes
/// their `let`s, and the values of the expressions already walked in it.
///
/// The memo makes a shared expression id cost one walk, not one per path. It is valid while the
/// locals keep their values: a rebinding to a different value clears it. Every side effect of a
/// walk (a site's or a branch's sources joined, a local bound) is idempotent, so skipping the
/// repeat walk of a memoized expression loses none.
pub(super) struct Scope {
    owner: String,
    in_body: bool,
    locals: Vec<Option<Value>>,
    memo: HashMap<ExprId, Value>,
    /// The join of what the `return`s of this function carry; `None` for a body.
    returned: Option<Value>,
    /// How many conditions of `Walk::enclosing` held when this owner's walk began: a `return`
    /// or an assignment reads only the conditions inside the owner.
    enclosed_from: usize,
}

impl Scope {
    /// A scope of `owner`, with `locals` unbound locals; `in_body` when the owner is a body.
    fn new(owner: String, in_body: bool, locals: usize) -> Self {
        Scope { owner, in_body, locals: vec![None; locals], memo: HashMap::new(), returned: None, enclosed_from: 0 }
    }
}

/// One step of the iterative walk. Expressions leave their value on the value stack.
pub(super) enum Task {
    /// Evaluate an expression: its value is pushed.
    Expr(ExprId),
    /// The operands of an expression are on the value stack: combine them.
    Finish(ExprId),
    Stmt(StmtId),
    /// A `let` has evaluated its init (on the stack): bind the local.
    Bind(u32, ExprId),
    /// An expression statement's value is not used.
    Discard,
    /// An addend's value (on the stack) is contributed to a branch.
    Addend { branch: u32, operator: Operator, at: ExprId },
    /// The step of a `bound_step` site (its value on the stack).
    Cap(u32),
    /// A field of a trace site (its value on the stack).
    Trace(u32),
    /// The condition of the `if` `0` is on the value stack: keep it for its arms.
    Hold(ExprId),
    /// The walk enters an arm of the `if` `0`: its condition encloses what the arm contributes.
    Enter(ExprId),
    /// The walk leaves the arm it entered last.
    Leave,
    /// A closure's parameters are bound before its body is walked.
    BindParams(ExprId),
    /// A block or a closure ends: the locals it bound go out of scope.
    Restore(Vec<Option<Value>>),
    /// A callee's body has been walked (its value on the stack): leave its scope.
    Return { at: ExprId, function: FunctionId, args: Vec<Value> },
    /// The value of an assignment is on the stack: join it into the local.
    Assign { local: u32, op: Option<BinaryOp>, at: ExprId, indices: usize },
    /// The bounds of a `for` range are on the stack: the loop variable and the trip count.
    LoopBounds(ExprId),
    /// The condition of a `while` is on the stack: it decides the trip count.
    LoopCondition(ExprId),
    /// A pass over the body of loop `0` ended (`before` holds the locals it began with), or the
    /// loop has just begun (`before` is `None`): walk again while a local changed, else finish.
    LoopPass { id: ExprId, before: Option<Vec<Option<Value>>> },
    /// A `break` or `continue` (its value on the stack when `valued`) leaves the innermost loop.
    Escape { at: ExprId, valued: bool },
    /// A `return` (its value on the stack when `valued`) joins the function's result.
    Yield { at: ExprId, valued: bool },
}

/// One abstract walk of a body, and of the functions it calls. The walk is iterative (an
/// explicit task stack and value stack), so nesting depth costs no native stack.
pub(super) struct Walk<'a> {
    ir: &'a Ir,
    /// The functions being walked, innermost last: a call of one of them is a recursion.
    calls: Vec<FunctionId>,
    /// The owners being walked, innermost (the current one) last.
    scopes: Vec<Scope>,
    /// The value of each callee body already walked, by function and argument values. A
    /// function reads only its arguments and what it reads itself, and its walk has no side
    /// effect on a checked IR (sites and contributions live in bodies), so equal arguments give
    /// an equal value: a call chain costs one walk per distinct signature, not one per path.
    returns: HashMap<(FunctionId, Vec<Value>), Value>,
    values: Vec<Value>,
    flow: Vec<Sources>,
    charge: Vec<Sources>,
    affine: Vec<bool>,
    limits: Vec<Sources>,
    stims: Vec<Sources>,
    /// While a reaction is walked: the sources of every expression it evaluates.
    reads: Option<Sources>,
    /// The step of each `bound_step` site, grown as sites are met.
    caps: Vec<Sources>,
    /// What each trace site writes, by site.
    traces: Vec<Sources>,
    /// The condition of each `if` whose arms are being walked, by the `if`'s expression id.
    held: HashMap<ExprId, Value>,
    /// The conditions of the arms enclosing the statement being walked, outermost first: what a
    /// nested contribution reads beyond its own addends (analog-structure, mechanism 1).
    enclosing: Vec<Value>,
    /// The loops being walked, innermost last.
    frames: Vec<LoopFrame>,
    /// The functions a recursive call met while their own body was being walked: each is walked
    /// again until its result holds still.
    recursing: HashSet<FunctionId>,
    /// The result of each function in `recursing` so far (absent until the first walk ends).
    approximate: HashMap<FunctionId, Value>,
}

impl<'a> Walk<'a> {
    /// The current owner's scope.
    fn scope(&mut self) -> &mut Scope {
        match self.scopes.last_mut() {
            Some(scope) => scope,
            None => panic!("the dependency walk has no scope"),
        }
    }

    /// The value on top of the value stack.
    fn pop(&mut self) -> Value {
        match self.values.pop() {
            Some(value) => value,
            None => panic!("the dependency walk lost a value"),
        }
    }

    /// The last `n` values, in evaluation order.
    fn pop_n(&mut self, n: usize) -> Vec<Value> {
        let at = self.values.len().saturating_sub(n);
        self.values.split_off(at)
    }

    /// Panics naming the owner and the origin of `id`: a form the walk cannot give a meaning to.
    fn refuse(&self, id: ExprId, what: &str) -> ! {
        let o = &self.ir.exprs[id.index()].origin;
        let owner = self.scopes.last().map_or("<no owner>", |s| s.owner.as_str());
        panic!("{owner}: {what} at {}:{}:{} (run Ir::check first)", o.file, o.line, o.col)
    }

    /// Runs the walk from `root` to its end.
    fn run(&mut self, root: Task) {
        let mut stack = vec![root];
        let mut next = Vec::new();
        while let Some(task) = stack.pop() {
            match task {
                Task::Expr(id) => self.expr(id, &mut next),
                Task::Finish(id) => self.finish(id, &mut next),
                Task::Stmt(id) => self.stmt(id, &mut next),
                Task::Bind(local, at) => {
                    let value = self.pop();
                    self.bind(local, value, at);
                }
                Task::Discard => {
                    self.pop();
                }
                Task::Hold(at) => {
                    let condition = self.values.last().cloned();
                    match condition {
                        Some(condition) => {
                            self.held.insert(at, condition);
                        }
                        None => self.refuse(at, "an `if` lost its condition"),
                    }
                }
                Task::Enter(at) => match self.held.get(&at).cloned() {
                    Some(condition) => self.enclosing.push(condition),
                    None => self.refuse(at, "an arm of an `if` entered before its condition"),
                },
                Task::Leave => {
                    self.enclosing.pop();
                }
                Task::Addend { branch, operator, at } => self.contribute(branch, operator, at),
                Task::Trace(site) => {
                    let value = self.pop();
                    if let Some(slot) = self.traces.get_mut(site as usize) {
                        slot.join(&value.sources);
                    }
                }
                Task::Cap(site) => self.grow_cap(site),
                Task::BindParams(id) => self.bind_closure_parameters(id),
                Task::Restore(outside) => self.restore(outside),
                Task::Return { at, function, args } => self.leave_call(at, function, args, &mut next),
                Task::Assign { local, op, at, indices } => self.assign(local, op, at, indices),
                Task::LoopBounds(id) => self.loop_bounds(id),
                Task::LoopCondition(id) => self.loop_condition(id),
                Task::LoopPass { id, before } => self.loop_pass(id, before, &mut next),
                Task::Escape { at, valued } => self.escape(at, valued),
                Task::Yield { at, valued } => self.yield_value(at, valued),
            }
            // `next` holds the follow-up tasks in evaluation order.
            stack.extend(next.drain(..).rev());
        }
    }

    /// Binds `local` to `value` in the current scope; a rebinding to a different value clears the memo.
    fn bind(&mut self, local: u32, value: Value, at: ExprId) {
        let scope = self.scope();
        match scope.locals.get_mut(local as usize) {
            Some(slot) => {
                if slot.as_ref().is_some_and(|old| *old != value) {
                    scope.memo.clear();
                }
                *slot = Some(value);
            }
            None => self.refuse(at, &format!("local {local} out of range")),
        }
    }

    /// `value` is the value of `id`: memoized in the current scope and pushed; while a reaction is
    /// walked, its sources are among the reaction's reads.
    fn done(&mut self, id: ExprId, value: Value) {
        if let Some(reads) = &mut self.reads {
            reads.join(&value.sources);
        }
        // A form that assigns is walked on every visit: a memoized value would hide its effect.
        if !Walk::assigns(&self.ir.exprs[id.index()].kind) {
            self.scope().memo.insert(id, value.clone());
        }
        self.values.push(value);
    }

    /// An addend's value (on the stack) is contributed to `branch`: under a condition it also reads
    /// what the condition reads, and a condition that is not `Constant` in the potentials makes the
    /// sum piecewise, so no longer affine (a switch branch).
    fn contribute(&mut self, branch: u32, operator: Operator, at: ExprId) {
        let mut value = self.pop();
        let b = branch as usize;
        if b >= self.flow.len() {
            self.refuse(at, &format!("contribution to branch {branch}, out of range"));
        }
        for condition in &self.enclosing {
            value.sources.join(&condition.sources);
            if condition.class != Class::Constant {
                value.class = Class::Nonlinear;
            }
        }
        match operator {
            Operator::Direct => {
                self.flow[b].join(&value.sources);
                if value.class == Class::Nonlinear {
                    self.affine[b] = false;
                }
            }
            Operator::Ddt => self.charge[b].join(&value.sources),
        }
    }

    /// The step of a `bound_step` site (its value on the stack) joins the site's sources.
    fn grow_cap(&mut self, site: u32) {
        let value = self.pop();
        if self.caps.len() <= site as usize {
            self.caps.resize(site as usize + 1, Sources::default());
        }
        self.caps[site as usize].join(&value.sources);
    }

    /// A closure's parameters are supplied at run time (the limiter's `new` and `old`): they carry
    /// no static source and are an iterate of a potential.
    fn bind_closure_parameters(&mut self, id: ExprId) {
        if let Expr::Closure { params, .. } = &self.ir.exprs[id.index()].kind {
            for &p in params {
                self.bind(p, Value::of(Sources::default(), Class::Affine), id);
            }
        }
    }
}
