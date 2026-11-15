//! Static dependencies and the affine test (IR-09): what a body's addends, `limit` sites,
//! `ac_stim` sites and `where` checks read, and whether a branch's direct part is affine in the
//! node potentials. Every backend declares its reads and its partial pattern from these sets.
//!
//! One abstract walk per body, memoized on first use. Every value carries the set of sources it
//! reads and a class on the lattice `Constant < Affine < Nonlinear` (in the node potentials):
//!
//! - constants, params and environment reads are `Constant`; a probe is `Affine`;
//! - a `limit` read is the site alone, `Affine` (IR-10): the backend holds the limited iterate as an
//!   input that stands in for its input `x` (abi.md A2), so `x`, the function and the seed are
//!   the site's own dependencies (`Target::Limit`), not its readers';
//! - an `ac_stim` read is the site plus its magnitude and phase, which are evaluated where the
//!   stimulus appears; it is `Constant` when they are;
//! - `+`, `-` and unary `-` join; `*` by a `Constant` operand and `/` by a `Constant` divisor
//!   keep the other operand's class; any other operator, cast or method touching a
//!   non-`Constant` operand gives `Nonlinear`;
//! - a call reads its arguments and what the callee reads itself (walked with its parameters
//!   bound to the arguments, once per distinct function and argument values); it is `Constant`
//!   when every argument is, `Nonlinear` otherwise;
//! - an `if` reads its condition and both arms; it joins the arms when the condition is
//!   `Constant`, and is `Nonlinear` otherwise (IR-10).
//!
//! Locals are bound by scope, as `Ir::check` reads them: a `let` until its block ends, a
//! closure's parameters in its body. The walk assumes an IR that passes `Ir::check`, which names every form the walk cannot give
//! a meaning to (a local read before its `let`, a recursive call, a cycle); on an IR that fails
//! it the walk panics with the origin, never answers a default.

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use crate::handle::Body;
use crate::ids::{ExprId, FunctionId, StmtId};
use crate::model::{BinaryOp, BodyData, Env, Expr, Ir, Operator, Stmt, Terminal, UnaryOp};

/// The index of a branch in its body (`Body::branches`).
pub type BranchIndex = u32;
/// The index of a `limit` or `ac_stim` site in its body's list.
pub type SiteIndex = u32;
/// The index of a param in its body (`Body::params`).
pub type ParamIndex = u32;

/// What a dependency query is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The direct addends contributed to a branch, whatever its quantity: the resistive part.
    Flow(BranchIndex),
    /// The `ddt` addends contributed to a branch: the reactive part.
    Charge(BranchIndex),
    /// A `limit` site: its input, its function and its seed.
    Limit(SiteIndex),
    /// An `ac_stim` site: its magnitude and phase.
    Stim(SiteIndex),
    /// The `where` expression of a param; empty when the param has none.
    Check(ParamIndex),
}

/// A dependency set. Fields come in the fixed order nodes, params, environment reads, `limit`
/// sites, `ac_stim` sites; each is sorted by index (environment reads in `Env` order) and holds
/// no duplicate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deps {
    /// Body nodes probed, by index (`Body::nodes`); ground is not a node.
    pub nodes: Vec<u32>,
    /// Params read, by index (`Body::params`).
    pub params: Vec<u32>,
    pub env: Vec<Env>,
    pub limits: Vec<SiteIndex>,
    pub stims: Vec<SiteIndex>,
}

impl<'a> Body<'a> {
    /// What `target` reads, through locals and function calls. A target out of range of the
    /// body panics, as a handle given an id out of range does.
    pub fn dependencies(&self, target: Target) -> Deps {
        let deps = self.deps();
        let (list, i, what) = match target {
            Target::Flow(i) => (&deps.flow, i, "branch"),
            Target::Charge(i) => (&deps.charge, i, "branch"),
            Target::Limit(i) => (&deps.limits, i, "limit site"),
            Target::Stim(i) => (&deps.stims, i, "ac_stim site"),
            Target::Check(i) => (&deps.checks, i, "param"),
        };
        match list.get(i as usize) {
            Some(sources) => sources.to_deps(),
            None => panic!("body {}: {what} {i} out of range ({} in the body)", self.name(), list.len()),
        }
    }

    /// Whether every direct addend contributed to `branch` is affine in the node potentials: every
    /// probe reaches the sum only through `+`, `-`, unary `-`, and `*` or `/` by an expression
    /// that reads no probe and no `limit` site. A branch with no direct addend is affine.
    pub fn is_affine(&self, branch: BranchIndex) -> bool {
        let deps = self.deps();
        match deps.affine.get(branch as usize) {
            Some(affine) => *affine,
            None => panic!("body {}: branch {branch} out of range ({} in the body)", self.name(), deps.affine.len()),
        }
    }

    fn deps(&self) -> &'a BodyDeps {
        let ir = self.ir();
        let data = &ir.bodies[self.id().index()];
        data.deps.0.get_or_init(|| Walk::body(ir, data))
    }
}

/// The memo of a body's dependency walk. It is derived data: it never takes part in equality,
/// cloning or serialization, so an IR compares, clones and round-trips the same whether or not
/// it has been asked.
#[derive(Debug, Default)]
pub(crate) struct DepsMemo(OnceLock<BodyDeps>);

impl Clone for DepsMemo {
    fn clone(&self) -> Self {
        DepsMemo::default()
    }
}

impl PartialEq for DepsMemo {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The result of one body walk, one entry per branch, site and param.
#[derive(Debug)]
pub(crate) struct BodyDeps {
    flow: Vec<Sources>,
    charge: Vec<Sources>,
    affine: Vec<bool>,
    limits: Vec<Sources>,
    stims: Vec<Sources>,
    checks: Vec<Sources>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct Sources {
    nodes: BTreeSet<u32>,
    params: BTreeSet<u32>,
    env: BTreeSet<Env>,
    limits: BTreeSet<u32>,
    stims: BTreeSet<u32>,
}

impl Sources {
    fn join(&mut self, other: &Sources) {
        self.nodes.extend(&other.nodes);
        self.params.extend(&other.params);
        self.env.extend(&other.env);
        self.limits.extend(&other.limits);
        self.stims.extend(&other.stims);
    }

    fn to_deps(&self) -> Deps {
        Deps {
            nodes: self.nodes.iter().copied().collect(),
            params: self.params.iter().copied().collect(),
            env: self.env.iter().copied().collect(),
            limits: self.limits.iter().copied().collect(),
            stims: self.stims.iter().copied().collect(),
        }
    }
}

/// How a value depends on the node potentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Class {
    Constant,
    Affine,
    Nonlinear,
}

/// The abstract value of an expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Value {
    sources: Sources,
    class: Class,
}

impl Value {
    fn constant() -> Self {
        Value { sources: Sources::default(), class: Class::Constant }
    }

    fn of(sources: Sources, class: Class) -> Self {
        Value { sources, class }
    }

    /// The sources of every value; the class is `Constant` when every class is, `Nonlinear`
    /// otherwise: the rule of every operator that is not linear.
    fn nonlinear<'v>(values: impl IntoIterator<Item = &'v Value>) -> Self {
        let mut out = Value::constant();
        for v in values {
            out.sources.join(&v.sources);
            if v.class != Class::Constant {
                out.class = Class::Nonlinear;
            }
        }
        out
    }
}

/// The locals of one owner (a body's `analog` block or a function), bound as the walk passes
/// their `let`s, and the values of the expressions already walked in it.
///
/// The memo makes a shared expression id cost one walk, not one per path. It is valid while the
/// locals keep their values: a rebinding to a different value clears it. Every side effect of a
/// walk (a site's or a branch's sources joined, a local bound) is idempotent, so skipping the
/// repeat walk of a memoized expression loses none.
struct Scope {
    owner: String,
    in_body: bool,
    locals: Vec<Option<Value>>,
    memo: HashMap<ExprId, Value>,
}

impl Scope {
    fn new(owner: String, in_body: bool, locals: usize) -> Self {
        Scope { owner, in_body, locals: vec![None; locals], memo: HashMap::new() }
    }
}

/// One step of the iterative walk. Expressions leave their value on the value stack.
enum Task {
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
    /// A closure's parameters are bound before its body is walked.
    BindParams(ExprId),
    /// A block or a closure ends: the locals it bound go out of scope.
    Restore(Vec<Option<Value>>),
    /// A callee's body has been walked (its value on the stack): leave its scope.
    Return { at: ExprId, function: FunctionId, args: Vec<Value> },
}

/// One abstract walk of a body, and of the functions it calls. The walk is iterative (an
/// explicit task stack and value stack), so nesting depth costs no native stack.
struct Walk<'a> {
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
}

impl<'a> Walk<'a> {
    fn body(ir: &'a Ir, data: &'a BodyData) -> BodyDeps {
        let branches = data.branches.len();
        let mut walk = Walk {
            ir,
            calls: Vec::new(),
            scopes: Vec::new(),
            returns: HashMap::new(),
            values: Vec::new(),
            flow: vec![Sources::default(); branches],
            charge: vec![Sources::default(); branches],
            affine: vec![true; branches],
            limits: vec![Sources::default(); data.limits.len()],
            stims: vec![Sources::default(); data.stims.len()],
        };
        let owner = format!("body {}", data.name);
        let checks = data
            .params
            .iter()
            .map(|param| match param.check {
                Some(check) => {
                    walk.scopes = vec![Scope::new(owner.clone(), true, data.locals.len())];
                    walk.run(Task::Expr(check));
                    walk.pop().sources
                }
                None => Sources::default(),
            })
            .collect();
        if let Some(analog) = &data.analog {
            walk.scopes = vec![Scope::new(owner, true, data.locals.len())];
            for &stmt in analog {
                walk.run(Task::Stmt(stmt));
            }
        }
        BodyDeps {
            flow: walk.flow,
            charge: walk.charge,
            affine: walk.affine,
            limits: walk.limits,
            stims: walk.stims,
            checks,
        }
    }

    /// The current owner's scope.
    fn scope(&mut self) -> &mut Scope {
        match self.scopes.last_mut() {
            Some(scope) => scope,
            None => panic!("the dependency walk has no scope"),
        }
    }

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
                Task::Addend { branch, operator, at } => {
                    let value = self.pop();
                    let b = branch as usize;
                    if b >= self.flow.len() {
                        self.refuse(at, &format!("contribution to branch {branch}, out of range"));
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
                Task::BindParams(id) => {
                    // A closure parameter is supplied at run time (the limiter's `new` and `old`):
                    // it carries no static source and is an iterate of a potential.
                    if let Expr::Closure { params, .. } = &self.ir.exprs[id.index()].kind {
                        for &p in params {
                            self.bind(p, Value::of(Sources::default(), Class::Affine), id);
                        }
                    }
                }
                Task::Restore(locals) => {
                    let scope = self.scope();
                    if scope.locals != locals {
                        scope.memo.clear();
                        scope.locals = locals;
                    }
                }
                Task::Return { at, function, args } => {
                    let callee = self.pop();
                    self.scopes.pop();
                    self.calls.pop();
                    self.called(at, &args, &callee);
                    self.returns.insert((function, args), callee);
                }
            }
            // `next` holds the follow-up tasks in evaluation order.
            stack.extend(next.drain(..).rev());
        }
    }

    fn stmt(&mut self, id: StmtId, next: &mut Vec<Task>) {
        let ir = self.ir;
        match &ir.stmts[id.index()] {
            Stmt::Let { local, init } => next.extend([Task::Expr(*init), Task::Bind(*local, *init)]),
            Stmt::Expr(e) => next.extend([Task::Expr(*e), Task::Discard]),
            Stmt::Contribute { branch, addends, .. } => {
                for addend in addends {
                    next.push(Task::Expr(addend.value));
                    next.push(Task::Addend { branch: *branch, operator: addend.operator, at: addend.value });
                }
            }
        }
    }

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

    /// `value` is the value of `id`: memoized in the current scope and pushed.
    fn done(&mut self, id: ExprId, value: Value) {
        self.scope().memo.insert(id, value.clone());
        self.values.push(value);
    }

    /// Starts `id`: a memoized or leaf value is pushed now, any other form schedules its operands
    /// and its `Finish`.
    fn expr(&mut self, id: ExprId, next: &mut Vec<Task>) {
        let ir = self.ir;
        if let Some(value) = self.scope().memo.get(&id).cloned() {
            self.values.push(value);
            return;
        }
        let in_body = self.scope().in_body;
        match &ir.exprs[id.index()].kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::Unknown => return self.done(id, Value::constant()),
            Expr::Local(i) => {
                let value = match self.scope().locals.get(*i as usize) {
                    Some(Some(value)) => value.clone(),
                    Some(None) => self.refuse(id, &format!("local {i} read before its `let`")),
                    None => self.refuse(id, &format!("local {i} out of range")),
                };
                return self.done(id, value);
            }
            Expr::Param(i) => {
                if !in_body {
                    self.refuse(id, "param read outside a body");
                }
                let mut sources = Sources::default();
                sources.params.insert(*i);
                return self.done(id, Value::of(sources, Class::Constant));
            }
            Expr::Env(env) => {
                let mut sources = Sources::default();
                sources.env.insert(*env);
                return self.done(id, Value::of(sources, Class::Constant));
            }
            Expr::Probe { plus, minus } => {
                if !in_body {
                    self.refuse(id, "probe outside a body");
                }
                let mut sources = Sources::default();
                for t in [plus, minus] {
                    if let Terminal::Node(n) = t {
                        sources.nodes.insert(*n);
                    }
                }
                return self.done(id, Value::of(sources, Class::Affine));
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => next.push(Task::Expr(*expr)),
            Expr::KnownOr { value, default } => next.extend([Task::Expr(*value), Task::Expr(*default)]),
            Expr::Method { receiver, args, .. } => {
                next.push(Task::Expr(*receiver));
                next.extend(args.iter().map(|&a| Task::Expr(a)));
            }
            Expr::Binary { lhs, rhs, .. } => next.extend([Task::Expr(*lhs), Task::Expr(*rhs)]),
            Expr::If { cond, then, otherwise } => {
                next.extend([Task::Expr(*cond), Task::Expr(*then), Task::Expr(*otherwise)]);
            }
            Expr::Block { stmts, value } => {
                next.extend(stmts.iter().map(|&s| Task::Stmt(s)));
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                // The block's `let`s end with it.
                let outside = self.scope().locals.clone();
                next.push(Task::Restore(outside));
            }
            Expr::Call { args, .. } | Expr::Math { args, .. } => next.extend(args.iter().map(|&a| Task::Expr(a))),
            Expr::Closure { body, .. } => {
                // The parameters end with the closure.
                let outside = self.scope().locals.clone();
                next.extend([Task::BindParams(id), Task::Expr(*body), Task::Restore(outside)]);
            }
            Expr::Limit { x, f, seed, .. } => {
                if !in_body {
                    self.refuse(id, "limit outside a body");
                }
                next.extend([Task::Expr(*x), Task::Expr(*f)]);
                next.extend(seed.iter().map(|&s| Task::Expr(s)));
            }
            Expr::Stim { mag, phase, .. } => {
                if !in_body {
                    self.refuse(id, "ac_stim outside a body");
                }
                next.push(Task::Expr(*mag));
                next.extend(phase.iter().map(|&p| Task::Expr(p)));
            }
        }
        // Any form with operands is combined once they are walked.
        next.push(Task::Finish(id));
    }

    /// Combines the operand values of `id`, left on the value stack by its operands' tasks.
    fn finish(&mut self, id: ExprId, next: &mut Vec<Task>) {
        let ir = self.ir;
        let value = match &ir.exprs[id.index()].kind {
            Expr::Unary { op, .. } => {
                let value = self.pop();
                match op {
                    UnaryOp::Neg => value,
                    UnaryOp::Not => Value::nonlinear([&value]),
                }
            }
            Expr::Binary { op, .. } => {
                let r = self.pop();
                let l = self.pop();
                Walk::binary(*op, l, r)
            }
            Expr::If { .. } => {
                let o = self.pop();
                let t = self.pop();
                let c = self.pop();
                let class = if c.class == Class::Constant { t.class.max(o.class) } else { Class::Nonlinear };
                let mut sources = c.sources;
                sources.join(&t.sources);
                sources.join(&o.sources);
                Value::of(sources, class)
            }
            Expr::Block { value, .. } => match value {
                Some(_) => self.pop(),
                None => Value::constant(),
            },
            Expr::Call { function, args } => {
                let values = self.pop_n(args.len());
                self.enter(id, *function, values, next);
                return;
            }
            Expr::Math { args, .. } => {
                let values = self.pop_n(args.len());
                Value::nonlinear(&values)
            }
            Expr::Cast { .. } | Expr::IsKnown(_) | Expr::High(_) => {
                let value = self.pop();
                Value::nonlinear([&value])
            }
            Expr::KnownOr { .. } => {
                let values = self.pop_n(2);
                Value::nonlinear(&values)
            }
            Expr::Method { args, .. } => {
                let values = self.pop_n(1 + args.len());
                Value::nonlinear(&values)
            }
            Expr::Closure { .. } => {
                let value = self.pop();
                Value::of(value.sources, Class::Nonlinear)
            }
            Expr::Limit { site, seed, .. } => {
                let mut own = Sources::default();
                for value in self.pop_n(2 + usize::from(seed.is_some())) {
                    own.join(&value.sources);
                }
                match self.limits.get_mut(*site as usize) {
                    Some(slot) => slot.join(&own),
                    None => self.refuse(id, &format!("limit site {site} out of range")),
                }
                let mut sources = Sources::default();
                sources.limits.insert(*site);
                Value::of(sources, Class::Affine)
            }
            Expr::Stim { site, phase, .. } => {
                let values = self.pop_n(1 + usize::from(phase.is_some()));
                let mut value = Value::nonlinear(&values);
                match self.stims.get_mut(*site as usize) {
                    Some(slot) => slot.join(&value.sources),
                    None => self.refuse(id, &format!("ac_stim site {site} out of range")),
                }
                value.sources.stims.insert(*site);
                value
            }
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::Unknown | Expr::Local(_) | Expr::Param(_) | Expr::Env(_) | Expr::Probe { .. } => {
                self.refuse(id, "a leaf has no operands to finish")
            }
        };
        self.done(id, value);
    }

    fn binary(op: BinaryOp, l: Value, r: Value) -> Value {
        let class = match op {
            BinaryOp::Add | BinaryOp::Sub => l.class.max(r.class),
            BinaryOp::Mul if l.class == Class::Constant => r.class,
            BinaryOp::Mul | BinaryOp::Div if r.class == Class::Constant => l.class,
            _ => Value::nonlinear([&l, &r]).class,
        };
        let mut sources = l.sources;
        sources.join(&r.sources);
        Value::of(sources, class)
    }

    /// Enters `function`, called at `at` with `args`: its body is walked in a scope of its own,
    /// its parameters (its first locals) bound to the arguments; `Return` leaves it.
    fn enter(&mut self, at: ExprId, function: FunctionId, args: Vec<Value>, next: &mut Vec<Task>) {
        let ir = self.ir;
        if let Some(callee) = self.returns.get(&(function, args.clone())).cloned() {
            self.called(at, &args, &callee);
            return;
        }
        let data = &ir.functions[function.index()];
        if self.calls.contains(&function) {
            self.refuse(at, &format!("recursive call of {}", data.name));
        }
        let mut callee = Scope::new(format!("function {}", data.name), false, data.locals.len());
        if args.len() != data.params.len() || args.len() > callee.locals.len() {
            self.refuse(at, &format!("call of {} with {} args, it takes {}", data.name, args.len(), data.params.len()));
        }
        for (slot, arg) in callee.locals.iter_mut().zip(&args) {
            *slot = Some(arg.clone());
        }
        self.calls.push(function);
        self.scopes.push(callee);
        next.extend([Task::Expr(data.body), Task::Return { at, function, args }]);
    }

    /// A call at `at` returned `callee`: the call reads its arguments and what the callee reads.
    fn called(&mut self, at: ExprId, args: &[Value], callee: &Value) {
        let mut out = Value::nonlinear(args);
        out.sources.join(&callee.sources);
        self.done(at, out);
    }
}
