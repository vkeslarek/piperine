//! Static dependencies and the affine test (IR-09): what a body's addends, `limit` sites,
//! `ac_stim` sites and `where` checks read, and whether a branch's direct part is affine in the
//! node potentials. Every backend declares its reads and its partial pattern from these sets.
//!
//! One abstract walk per body, memoized on first use. Every value carries the set of sources it
//! reads and a class on the lattice `Constant < Affine < Nonlinear` (in the node potentials):
//!
//! - constants, params and environment reads are `Constant`; a probe is `Affine`;
//! - a `limit` read is the site alone, `Affine`: the backend holds the limited iterate as an
//!   input that stands in for its input `x` (abi.md A2), so `x`, the function and the seed are
//!   the site's own dependencies (`Target::Limit`), not its readers';
//! - an `ac_stim` read is the site plus its magnitude and phase, which are evaluated where the
//!   stimulus appears; it is `Constant` when they are;
//! - `+`, `-` and unary `-` join; `*` by a `Constant` operand and `/` by a `Constant` divisor
//!   keep the other operand's class; any other operator touching a non-`Constant` operand gives
//!   `Nonlinear`;
//! - a call reads its arguments and what the callee reads itself (walked with its parameters
//!   bound to the arguments); it is `Constant` when every argument is, `Nonlinear` otherwise;
//! - an `if` reads its condition and both arms; it joins the arms when the condition is
//!   `Constant`, and is `Nonlinear` otherwise.
//!
//! The walk assumes an IR that passes `Ir::check`; a form `check` does not exclude and the walk
//! cannot give a meaning to (a local read before its `let`, a recursive call) panics with its
//! origin, never answers a default.

use std::collections::BTreeSet;
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

#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Constant,
    Affine,
    Nonlinear,
}

/// The abstract value of an expression.
#[derive(Debug, Clone)]
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
/// their `let`s.
struct Scope {
    owner: String,
    in_body: bool,
    locals: Vec<Option<Value>>,
}

/// One abstract walk of a body, and of the functions it calls.
struct Walk<'a> {
    ir: &'a Ir,
    /// The functions being walked, innermost last: a call of one of them is a recursion.
    calls: Vec<FunctionId>,
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
                    let mut scope = Scope { owner: owner.clone(), in_body: true, locals: vec![None; data.locals.len()] };
                    walk.expr(&mut scope, check).sources
                }
                None => Sources::default(),
            })
            .collect();
        if let Some(analog) = &data.analog {
            let mut scope = Scope { owner, in_body: true, locals: vec![None; data.locals.len()] };
            for &stmt in analog {
                walk.stmt(&mut scope, stmt);
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

    /// Panics naming the owner and the origin of `id`: a form the walk cannot give a meaning to.
    fn refuse(&self, scope: &Scope, id: ExprId, what: &str) -> ! {
        let o = &self.ir.exprs[id.index()].origin;
        panic!("{}: {what} at {}:{}:{} (run Ir::check first)", scope.owner, o.file, o.line, o.col)
    }

    fn stmt(&mut self, scope: &mut Scope, id: StmtId) {
        let ir = self.ir;
        match &ir.stmts[id.index()] {
            Stmt::Let { local, init } => {
                let value = self.expr(scope, *init);
                self.bind(scope, *local, value, *init);
            }
            Stmt::Expr(e) => {
                self.expr(scope, *e);
            }
            Stmt::Contribute { branch, addends, .. } => {
                let b = *branch as usize;
                for addend in addends {
                    let value = self.expr(scope, addend.value);
                    if b >= self.flow.len() {
                        self.refuse(scope, addend.value, &format!("contribution to branch {branch}, out of range"));
                    }
                    match addend.operator {
                        Operator::Direct => {
                            self.flow[b].join(&value.sources);
                            if value.class == Class::Nonlinear {
                                self.affine[b] = false;
                            }
                        }
                        Operator::Ddt => self.charge[b].join(&value.sources),
                    }
                }
            }
        }
    }

    fn bind(&self, scope: &mut Scope, local: u32, value: Value, at: ExprId) {
        match scope.locals.get_mut(local as usize) {
            Some(slot) => *slot = Some(value),
            None => self.refuse(scope, at, &format!("local {local} out of range")),
        }
    }

    fn expr(&mut self, scope: &mut Scope, id: ExprId) -> Value {
        let ir = self.ir;
        match &ir.exprs[id.index()].kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit => Value::constant(),
            Expr::Local(i) => match scope.locals.get(*i as usize) {
                Some(Some(value)) => value.clone(),
                Some(None) => self.refuse(scope, id, &format!("local {i} read before its `let`")),
                None => self.refuse(scope, id, &format!("local {i} out of range")),
            },
            Expr::Param(i) => {
                if !scope.in_body {
                    self.refuse(scope, id, "param read outside a body");
                }
                let mut sources = Sources::default();
                sources.params.insert(*i);
                Value::of(sources, Class::Constant)
            }
            Expr::Env(env) => {
                let mut sources = Sources::default();
                sources.env.insert(*env);
                Value::of(sources, Class::Constant)
            }
            Expr::Probe { plus, minus } => {
                if !scope.in_body {
                    self.refuse(scope, id, "probe outside a body");
                }
                let mut sources = Sources::default();
                for t in [plus, minus] {
                    if let Terminal::Node(n) = t {
                        sources.nodes.insert(*n);
                    }
                }
                Value::of(sources, Class::Affine)
            }
            Expr::Unary { op, expr } => {
                let value = self.expr(scope, *expr);
                match op {
                    UnaryOp::Neg => value,
                    UnaryOp::Not => Value::nonlinear([&value]),
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.expr(scope, *lhs);
                let r = self.expr(scope, *rhs);
                self.binary(*op, l, r)
            }
            Expr::If { cond, then, otherwise } => {
                let c = self.expr(scope, *cond);
                let t = self.expr(scope, *then);
                let o = self.expr(scope, *otherwise);
                let class = if c.class == Class::Constant { t.class.max(o.class) } else { Class::Nonlinear };
                let mut sources = c.sources;
                sources.join(&t.sources);
                sources.join(&o.sources);
                Value::of(sources, class)
            }
            Expr::Block { stmts, value } => {
                for &s in stmts {
                    self.stmt(scope, s);
                }
                match value {
                    Some(v) => self.expr(scope, *v),
                    None => Value::constant(),
                }
            }
            Expr::Call { function, args } => {
                let values: Vec<Value> = args.iter().map(|&a| self.expr(scope, a)).collect();
                let callee = self.call(scope, id, *function, &values);
                let mut out = Value::nonlinear(&values);
                out.sources.join(&callee.sources);
                out
            }
            Expr::Math { args, .. } => {
                let values: Vec<Value> = args.iter().map(|&a| self.expr(scope, a)).collect();
                Value::nonlinear(&values)
            }
            Expr::Closure { params, body } => {
                // A closure parameter is supplied at run time (the limiter's `new` and `old`): it
                // carries no static source and is an iterate of a potential.
                for &p in params {
                    self.bind(scope, p, Value::of(Sources::default(), Class::Affine), id);
                }
                let value = self.expr(scope, *body);
                Value::of(value.sources, Class::Nonlinear)
            }
            Expr::Limit { site, x, f, seed } => {
                if !scope.in_body {
                    self.refuse(scope, id, "limit outside a body");
                }
                let mut own = self.expr(scope, *x).sources;
                own.join(&self.expr(scope, *f).sources);
                if let Some(s) = seed {
                    own.join(&self.expr(scope, *s).sources);
                }
                match self.limits.get_mut(*site as usize) {
                    Some(slot) => slot.join(&own),
                    None => self.refuse(scope, id, &format!("limit site {site} out of range")),
                }
                let mut sources = Sources::default();
                sources.limits.insert(*site);
                Value::of(sources, Class::Affine)
            }
            Expr::Stim { site, mag, phase } => {
                if !scope.in_body {
                    self.refuse(scope, id, "ac_stim outside a body");
                }
                let mut values = vec![self.expr(scope, *mag)];
                if let Some(p) = phase {
                    values.push(self.expr(scope, *p));
                }
                let mut value = Value::nonlinear(&values);
                match self.stims.get_mut(*site as usize) {
                    Some(slot) => slot.join(&value.sources),
                    None => self.refuse(scope, id, &format!("ac_stim site {site} out of range")),
                }
                value.sources.stims.insert(*site);
                value
            }
        }
    }

    fn binary(&self, op: BinaryOp, l: Value, r: Value) -> Value {
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

    /// What `function` reads when called with `args`: its body walked in a scope of its own,
    /// its parameters (its first locals) bound to the arguments.
    fn call(&mut self, scope: &Scope, at: ExprId, function: FunctionId, args: &[Value]) -> Value {
        let ir = self.ir;
        let data = &ir.functions[function.index()];
        if self.calls.contains(&function) {
            self.refuse(scope, at, &format!("recursive call of {}", data.name));
        }
        let mut callee = Scope { owner: format!("function {}", data.name), in_body: false, locals: vec![None; data.locals.len()] };
        if args.len() != data.params.len() || args.len() > callee.locals.len() {
            self.refuse(scope, at, &format!("call of {} with {} args, it takes {}", data.name, args.len(), data.params.len()));
        }
        for (slot, arg) in callee.locals.iter_mut().zip(args) {
            *slot = Some(arg.clone());
        }
        self.calls.push(function);
        let value = self.expr(&mut callee, data.body);
        self.calls.pop();
        value
    }
}
