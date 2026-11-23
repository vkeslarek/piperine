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
//! - a `limit` under a condition also reads what its `enabled` reads, and a `bound_step` what its
//!   `when` reads (`Target::Limit`, `Target::BoundStep`);
//! - an `ac_stim` read is the site plus its magnitude and phase, which are evaluated where the
//!   stimulus appears; it is `Constant` when they are;
//! - `+`, `-` and unary `-` join; `*` by a `Constant` operand and `/` by a `Constant` divisor
//!   keep the other operand's class; any other operator, cast or method touching a
//!   non-`Constant` operand gives `Nonlinear`;
//! - a call reads its arguments and what the callee reads itself (walked with its parameters
//!   bound to the arguments, once per distinct function and argument values); it is `Constant`
//!   when every argument is, `Nonlinear` otherwise;
//! - an `if` reads its condition and both arms; it joins the arms when the condition is
//!   `Constant`, and is `Nonlinear` otherwise (IR-10);
//! - a contribution under an `if` also reads what the conditions of its enclosing arms read, and
//!   a condition that is not `Constant` makes its branch non-affine (a switch).
//!
//! - a carrier read is `Constant` in the node potentials and reads the carrier: a discrete value
//!   holds between events (`Body::carrier_reads` lists them; `Deps` keeps its M1 fields).
//!
//! A reaction reads every source of every expression its statements evaluate, conditions and
//! the values and delays of its commits included.
//!
//! Locals are bound by scope, as `Ir::check` reads them: a `let` until its block ends, a
//! closure's parameters in its body. The walk assumes an IR that passes `Ir::check`, which names every form the walk cannot give
//! a meaning to (a local read before its `let`, a cycle); on an IR that fails
//! it the walk panics with the origin, never answers a default.
//!
//! Mutation, loops and recursion (`control.rs`) stay inside the finite lattice: an assignment
//! joins the assigned value into the local's, a loop's body is walked again until no local
//! changes, and a recursive call reads the function's result so far, which the function's own
//! walk grows until it holds still. A loop's trip count and the conditions its exits sit under
//! reach every local the loop assigns, and a function value is walked as a call whose
//! arguments are unknown.

mod control;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

use crate::handle::Body;
use crate::ids::{ExprId, FunctionId, StmtId};
use crate::model::{BinaryOp, BodyData, EmitField, Env, EventSource, Expr, Ir, Operator, Stmt, Terminal, UnaryOp};
use control::LoopFrame;

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
    /// The value of a drive (`Body::drives`), by index.
    Drive(u32),
    /// The delay of a drive; empty when the drive has no `after`.
    DriveAfter(u32),
    /// The guard of an event (`Body::events`); empty when it has none.
    Guard(u32),
    /// The crossed expression of a `Cross` event; empty for a `Changed` or an `At` one.
    Cross(u32),
    /// The time argument of an `At` event (`Body::events`); empty for any other event.
    At(u32),
    /// Everything a reaction (`Body::reactions`) reads.
    Reaction(u32),
    /// The step of a `bound_step` site (`Stmt::BoundStep::site`); empty for a site the body
    /// does not have.
    BoundStep(u32),
    /// The `ttol` of a `Cross` event; empty when the event has none or is not a `Cross`.
    Ttol(u32),
    /// The `etol` of a `Cross` event; empty when the event has none or is not a `Cross`.
    Etol(u32),
    /// The state of a derivative site (`Body::derivatives`): the quantity it differentiates.
    Derivative(u32),
    /// The `abstol` of a derivative site; empty when the site has none.
    DerivativeAbstol(u32),
    /// The delayed quantity of a delay site (`Body::delays`).
    DelayInput(u32),
    /// The delay time of a delay site.
    DelayTime(u32),
    /// The `max` of a delay site; empty when the site has none.
    DelayMaximum(u32),
    /// The power density of a noise source (`Body::noise_sources`).
    NoiseDensity(u32),
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
    /// Delay sites whose output (`Expr::Delay`) is read, by index (`Body::delays`).
    pub delays: Vec<u32>,
    /// Noise sources whose value (`Expr::Noise`) is read, by index (`Body::noise_sources`).
    pub noise_sources: Vec<u32>,
}

impl<'a> Body<'a> {
    /// What `target` reads, through locals and function calls. A target out of range of the
    /// body panics, as a handle given an id out of range does.
    pub fn dependencies(&self, target: Target) -> Deps {
        self.sources_of(target).to_deps()
    }

    /// The carriers `target` reads, by index (`Body::carriers`), sorted, through locals and
    /// function calls: the discrete part of its dependency set.
    pub fn carrier_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).carriers.iter().copied().collect()
    }

    /// The branches whose flow `target` reads (`Expr::Flow`), by index (`Body::branches`),
    /// sorted, through locals and function calls: the flow part of its dependency set.
    pub fn flow_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).flows.iter().copied().collect()
    }

    /// The carriers whose previous value (`Expr::Previous`) `target` reads, by index
    /// (`Body::carriers`), sorted, through locals and function calls. A read of a previous value
    /// is not in `carrier_reads`: it reads the carrier's `PREVIOUS` and not its value.
    pub fn previous_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).previous.iter().copied().collect()
    }

    /// The params whose previous value (`Expr::ParamPrevious`) `target` reads, by index
    /// (`Body::params`), sorted, through locals and function calls.
    pub fn param_previous_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).param_previous.iter().copied().collect()
    }

    /// The params whose `changed` event (`EventSource::ParamChanged`) `target` reads through
    /// `Expr::Fired`, by index (`Body::params`), sorted and without repeats.
    pub fn param_changes(&self, target: Target) -> Vec<u32> {
        let events = self.events();
        let params: BTreeSet<u32> = self
            .fired_reads(target)
            .into_iter()
            .filter_map(|e| match events.get(e as usize).map(|event| &event.source) {
                Some(EventSource::ParamChanged(p)) => Some(*p),
                _ => None,
            })
            .collect();
        params.into_iter().collect()
    }

    /// The derivative sites whose rate (`Expr::Derivative`) `target` reads, by index
    /// (`Body::derivatives`), sorted, through locals and function calls.
    pub fn derivative_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).derivatives.iter().copied().collect()
    }

    /// The noise sources whose value (`Expr::Noise`) `target` reads, by index
    /// (`Body::noise_sources`), sorted, through locals and function calls.
    pub fn noise_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).noises.iter().copied().collect()
    }

    /// The delay sites whose output (`Expr::Delay`) `target` reads, by index (`Body::delays`),
    /// sorted, through locals and function calls.
    pub fn delay_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).delays.iter().copied().collect()
    }

    /// The events whose `fired` byte `target` reads (`Expr::Fired`), by index (`Body::events`),
    /// sorted, through locals and function calls.
    pub fn fired_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).fired.iter().copied().collect()
    }

    fn sources_of(&self, target: Target) -> &'a Sources {
        let deps = self.deps();
        let (list, i, what) = match target {
            Target::Flow(i) => (&deps.flow, i, "branch"),
            Target::Charge(i) => (&deps.charge, i, "branch"),
            Target::Limit(i) => (&deps.limits, i, "limit site"),
            Target::Stim(i) => (&deps.stims, i, "ac_stim site"),
            Target::Check(i) => (&deps.checks, i, "param"),
            Target::Drive(i) => (&deps.drives, i, "drive"),
            Target::DriveAfter(i) => (&deps.drive_afters, i, "drive"),
            Target::Guard(i) => (&deps.guards, i, "event"),
            Target::Cross(i) => (&deps.crosses, i, "event"),
            Target::At(i) => (&deps.ats, i, "event"),
            Target::Reaction(i) => (&deps.reactions, i, "reaction"),
            Target::BoundStep(i) => (&deps.caps, i, "bound_step site"),
            Target::Ttol(i) => (&deps.ttols, i, "event"),
            Target::Etol(i) => (&deps.etols, i, "event"),
            Target::Derivative(i) => (&deps.derivative_states, i, "derivative site"),
            Target::DerivativeAbstol(i) => (&deps.derivative_abstols, i, "derivative site"),
            Target::DelayInput(i) => (&deps.delay_inputs, i, "delay site"),
            Target::DelayTime(i) => (&deps.delay_times, i, "delay site"),
            Target::DelayMaximum(i) => (&deps.delay_maxima, i, "delay site"),
            Target::NoiseDensity(i) => (&deps.noise_densities, i, "noise source"),
        };
        match list.get(i as usize) {
            Some(sources) => sources,
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
    drives: Vec<Sources>,
    drive_afters: Vec<Sources>,
    guards: Vec<Sources>,
    crosses: Vec<Sources>,
    ats: Vec<Sources>,
    reactions: Vec<Sources>,
    caps: Vec<Sources>,
    ttols: Vec<Sources>,
    etols: Vec<Sources>,
    derivative_states: Vec<Sources>,
    derivative_abstols: Vec<Sources>,
    delay_inputs: Vec<Sources>,
    delay_times: Vec<Sources>,
    delay_maxima: Vec<Sources>,
    noise_densities: Vec<Sources>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct Sources {
    nodes: BTreeSet<u32>,
    params: BTreeSet<u32>,
    env: BTreeSet<Env>,
    limits: BTreeSet<u32>,
    stims: BTreeSet<u32>,
    carriers: BTreeSet<u32>,
    /// The branches whose flow is read (`Expr::Flow`).
    flows: BTreeSet<u32>,
    /// The carriers whose previous value is read (`Expr::Previous`).
    previous: BTreeSet<u32>,
    /// The events whose `CHANGED` byte this microstep is read (`Expr::Fired`).
    fired: BTreeSet<u32>,
    /// The derivative sites whose rate is read (`Expr::Derivative`).
    derivatives: BTreeSet<u32>,
    /// The delay sites whose output is read (`Expr::Delay`).
    delays: BTreeSet<u32>,
    /// The noise sources whose value is read (`Expr::Noise`).
    noises: BTreeSet<u32>,
    /// The params whose previous value is read (`Expr::ParamPrevious`).
    param_previous: BTreeSet<u32>,
}

impl Sources {
    fn join(&mut self, other: &Sources) {
        self.nodes.extend(&other.nodes);
        self.params.extend(&other.params);
        self.env.extend(&other.env);
        self.limits.extend(&other.limits);
        self.stims.extend(&other.stims);
        self.carriers.extend(&other.carriers);
        self.flows.extend(&other.flows);
        self.previous.extend(&other.previous);
        self.fired.extend(&other.fired);
        self.derivatives.extend(&other.derivatives);
        self.delays.extend(&other.delays);
        self.noises.extend(&other.noises);
        self.param_previous.extend(&other.param_previous);
    }

    fn to_deps(&self) -> Deps {
        Deps {
            nodes: self.nodes.iter().copied().collect(),
            params: self.params.iter().copied().collect(),
            env: self.env.iter().copied().collect(),
            limits: self.limits.iter().copied().collect(),
            stims: self.stims.iter().copied().collect(),
            delays: self.delays.iter().copied().collect(),
            noise_sources: self.noises.iter().copied().collect(),
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
    /// The join of what the `return`s of this function carry; `None` for a body.
    returned: Option<Value>,
    /// How many conditions of `Walk::enclosing` held when this owner's walk began: a `return`
    /// or an assignment reads only the conditions inside the owner.
    enclosed_from: usize,
}

impl Scope {
    fn new(owner: String, in_body: bool, locals: usize) -> Self {
        Scope { owner, in_body, locals: vec![None; locals], memo: HashMap::new(), returned: None, enclosed_from: 0 }
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
    /// The step of a `bound_step` site (its value on the stack).
    Cap(u32),
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
    /// While a reaction is walked: the sources of every expression it evaluates.
    reads: Option<Sources>,
    /// The step of each `bound_step` site, grown as sites are met.
    caps: Vec<Sources>,
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
            reads: None,
            caps: Vec::new(),
            held: HashMap::new(),
            enclosing: Vec::new(),
            frames: Vec::new(),
            recursing: HashSet::new(),
            approximate: HashMap::new(),
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
        let mut drives = Vec::with_capacity(data.drives.len());
        let mut drive_afters = Vec::with_capacity(data.drives.len());
        for drive in &data.drives {
            walk.scopes = vec![Scope::new(owner.clone(), true, data.locals.len())];
            walk.run(Task::Expr(drive.value));
            drives.push(walk.pop().sources);
            drive_afters.push(match drive.after {
                Some(after) => {
                    walk.scopes = vec![Scope::new(owner.clone(), true, data.locals.len())];
                    walk.run(Task::Expr(after));
                    walk.pop().sources
                }
                None => Sources::default(),
            });
        }
        let mut guards = Vec::with_capacity(data.events.len());
        let mut ttols = Vec::with_capacity(data.events.len());
        let mut etols = Vec::with_capacity(data.events.len());
        let mut crosses = Vec::with_capacity(data.events.len());
        let mut ats = Vec::with_capacity(data.events.len());
        for event in &data.events {
            let mut one = |root: Option<ExprId>| match root {
                Some(e) => {
                    walk.scopes = vec![Scope::new(owner.clone(), true, data.locals.len())];
                    walk.run(Task::Expr(e));
                    walk.pop().sources
                }
                None => Sources::default(),
            };
            crosses.push(one(match event.source {
                EventSource::Cross { e, .. } => Some(e),
                EventSource::Changed(_) | EventSource::At(_) | EventSource::ParamChanged(_) | EventSource::Settle => None,
            }));
            ttols.push(one(match event.source {
                EventSource::Cross { ttol, .. } => ttol,
                EventSource::Changed(_) | EventSource::At(_) | EventSource::ParamChanged(_) | EventSource::Settle => None,
            }));
            etols.push(one(match event.source {
                EventSource::Cross { etol, .. } => etol,
                EventSource::Changed(_) | EventSource::At(_) | EventSource::ParamChanged(_) | EventSource::Settle => None,
            }));
            ats.push(one(match event.source {
                EventSource::At(e) => Some(e),
                EventSource::Changed(_) | EventSource::Cross { .. } | EventSource::ParamChanged(_) | EventSource::Settle => None,
            }));
            guards.push(one(event.guard));
        }
        let mut one = |root: Option<ExprId>| match root {
            Some(e) => {
                walk.scopes = vec![Scope::new(owner.clone(), true, data.locals.len())];
                walk.run(Task::Expr(e));
                walk.pop().sources
            }
            None => Sources::default(),
        };
        let derivative_states: Vec<Sources> = data.derivatives.iter().map(|d| one(Some(d.state))).collect();
        let derivative_abstols: Vec<Sources> = data.derivatives.iter().map(|d| one(d.abstol)).collect();
        let delay_inputs: Vec<Sources> = data.delays.iter().map(|d| one(Some(d.input))).collect();
        let delay_times: Vec<Sources> = data.delays.iter().map(|d| one(Some(d.time))).collect();
        let delay_maxima: Vec<Sources> = data.delays.iter().map(|d| one(d.maximum)).collect();
        let noise_densities: Vec<Sources> = data.noise_sources.iter().map(|n| one(Some(n.density))).collect();
        let mut reactions = Vec::with_capacity(data.reactions.len());
        for reaction in &data.reactions {
            walk.scopes = vec![Scope::new(owner.clone(), true, reaction.locals.len())];
            walk.reads = Some(Sources::default());
            for &stmt in &reaction.stmts {
                walk.run(Task::Stmt(stmt));
            }
            reactions.push(walk.reads.take().unwrap_or_default());
        }
        // What enables a `limit` under a condition is read by its site, as its input is.
        for (i, site) in data.limits.iter().enumerate() {
            let Some(enabled) = site.enabled else { continue };
            walk.scopes = vec![Scope::new(owner.clone(), true, data.locals.len())];
            walk.run(Task::Expr(enabled));
            let value = walk.pop();
            walk.limits[i].join(&value.sources);
        }
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
            drives,
            drive_afters,
            guards,
            crosses,
            ats,
            reactions,
            caps: walk.caps,
            ttols,
            etols,
            derivative_states,
            derivative_abstols,
            delay_inputs,
            delay_times,
            delay_maxima,
            noise_densities,
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
                Task::Addend { branch, operator, at } => {
                    let mut value = self.pop();
                    let b = branch as usize;
                    if b >= self.flow.len() {
                        self.refuse(at, &format!("contribution to branch {branch}, out of range"));
                    }
                    // A contribution under a condition depends on what the condition reads; a
                    // condition that is not `Constant` in the potentials makes the sum piecewise,
                    // so no longer affine (a switch branch).
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
                Task::Cap(site) => {
                    let value = self.pop();
                    if self.caps.len() <= site as usize {
                        self.caps.resize(site as usize + 1, Sources::default());
                    }
                    self.caps[site as usize].join(&value.sources);
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

    fn stmt(&mut self, id: StmtId, next: &mut Vec<Task>) {
        let ir = self.ir;
        match &ir.stmts[id.index()] {
            Stmt::Let { local, init } => next.extend([Task::Expr(*init), Task::Bind(*local, *init)]),
            Stmt::Expr(e) => next.extend([Task::Expr(*e), Task::Discard]),
            Stmt::Assign { place, op, value } => {
                next.push(Task::Expr(*value));
                let indices: Vec<ExprId> = place.index_operands().collect();
                next.extend(indices.iter().map(|&index| Task::Expr(index)));
                next.push(Task::Assign { local: place.local, op: *op, at: *value, indices: indices.len() });
            }
            Stmt::BoundStep { site, value, when } => {
                next.extend([Task::Expr(*value), Task::Cap(*site)]);
                // What decides whether the call runs is read by the same site.
                if let Some(when) = when {
                    next.extend([Task::Expr(*when), Task::Cap(*site)]);
                }
            }
            Stmt::Contribute { branch, addends, .. } => {
                for addend in addends {
                    next.push(Task::Expr(addend.value));
                    next.push(Task::Addend { branch: *branch, operator: addend.operator, at: addend.value });
                }
            }
            Stmt::Commit { value, after, .. } => {
                next.extend([Task::Expr(*value), Task::Discard]);
                if let Some(after) = after {
                    next.extend([Task::Expr(*after), Task::Discard]);
                }
            }
            Stmt::Emit { fields, .. } => {
                for f in fields {
                    if let EmitField::Value { value, .. } = f {
                        next.extend([Task::Expr(*value), Task::Discard]);
                    }
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
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) => return self.done(id, Value::constant()),
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
            // The microstep's event byte: a read of its own, not in `Deps` (`fired_reads`).
            Expr::Fired(e) => {
                let mut sources = Sources::default();
                sources.fired.insert(*e);
                return self.done(id, Value::of(sources, Class::Constant));
            }
            Expr::Carrier(i) => {
                if !in_body {
                    self.refuse(id, "carrier read outside a body");
                }
                let mut sources = Sources::default();
                sources.carriers.insert(*i);
                return self.done(id, Value::of(sources, Class::Constant));
            }
            Expr::Previous(i) => {
                if !in_body {
                    self.refuse(id, "previous-value read outside a body");
                }
                let mut sources = Sources::default();
                sources.previous.insert(*i);
                return self.done(id, Value::of(sources, Class::Constant));
            }
            Expr::Flow { branch } => {
                if !in_body {
                    self.refuse(id, "flow read outside a body");
                }
                // The flow of a branch is one more unknown of the system, as a node potential
                // is: it reaches a sum affinely.
                let mut sources = Sources::default();
                sources.flows.insert(*branch);
                return self.done(id, Value::of(sources, Class::Affine));
            }
            Expr::ParamPrevious(p) => {
                if !in_body {
                    self.refuse(id, "previous-value read of a param outside a body");
                }
                let mut sources = Sources::default();
                sources.param_previous.insert(*p);
                return self.done(id, Value::of(sources, Class::Constant));
            }
            Expr::Derivative(k) => {
                if !in_body {
                    self.refuse(id, "derivative read outside a body");
                }
                // The rate of a site is one more unknown of the system, as a flow is: it
                // reaches a sum affinely.
                let mut sources = Sources::default();
                sources.derivatives.insert(*k);
                return self.done(id, Value::of(sources, Class::Affine));
            }
            Expr::Delay(k) => {
                if !in_body {
                    self.refuse(id, "delay read outside a body");
                }
                // The output of a site is one more unknown of the system, as a flow is: it
                // reaches a sum affinely.
                let mut sources = Sources::default();
                sources.delays.insert(*k);
                return self.done(id, Value::of(sources, Class::Affine));
            }
            Expr::Noise(k) => {
                if !in_body {
                    self.refuse(id, "noise read outside a body");
                }
                // The value of a source is an input that no node potential moves: it is a
                // constant of the sum.
                let mut sources = Sources::default();
                sources.noises.insert(*k);
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
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => next.push(Task::Expr(*expr)),
            Expr::Same { a, b } => next.extend([Task::Expr(*a), Task::Expr(*b)]),
            Expr::LevelCast { expr, .. } => next.push(Task::Expr(*expr)),
            Expr::Unwrap { is, value, .. } => next.extend([Task::Expr(*is), Task::Expr(*value)]),
            Expr::Method { receiver, args, .. } => {
                next.push(Task::Expr(*receiver));
                next.extend(args.iter().map(|&a| Task::Expr(a)));
            }
            Expr::Binary { lhs, rhs, .. } => next.extend([Task::Expr(*lhs), Task::Expr(*rhs)]),
            Expr::If { cond, then, otherwise } => {
                // Each arm is walked inside its condition, which a contribution there reads.
                next.extend([Task::Expr(*cond), Task::Hold(id)]);
                for arm in [*then, *otherwise] {
                    next.extend([Task::Enter(id), Task::Expr(arm), Task::Leave]);
                }
            }
            Expr::Block { stmts, value } => {
                next.extend(stmts.iter().map(|&s| Task::Stmt(s)));
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                // The block's `let`s end with it.
                let outside = self.scope().locals.clone();
                next.push(Task::Restore(outside));
            }
            Expr::Call { args, .. } | Expr::Math { args, .. } => next.extend(args.iter().map(|&a| Task::Expr(a))),
            // An aggregate is one abstract value: what its parts read (`finish`).
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } => {
                next.extend(ir.exprs[id.index()].kind.operands().into_iter().map(Task::Expr));
            }
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } => return self.begin_loop(id, next),
            Expr::Break(value) => {
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                next.push(Task::Escape { at: id, valued: value.is_some() });
                return;
            }
            Expr::Continue => {
                next.push(Task::Escape { at: id, valued: false });
                return;
            }
            Expr::Return(value) => {
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                next.push(Task::Yield { at: id, valued: value.is_some() });
                return;
            }
            // A function used as a value reads what its body reads, for arguments nothing is known of.
            Expr::FunctionRef(function) => {
                let unknown = Value::of(Sources::default(), Class::Nonlinear);
                let arguments = vec![unknown; ir.functions[function.index()].params.len()];
                return self.enter(id, *function, arguments, next);
            }
            Expr::CallValue { callee, args } => next.extend([Task::Expr(*callee)].into_iter().chain(args.iter().map(|&a| Task::Expr(a)))),
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
            Expr::CallValue { args, .. } => {
                let values = self.pop_n(1 + args.len());
                Value::nonlinear(&values)
            }
            Expr::Cast { .. } | Expr::LevelCast { .. } => {
                let value = self.pop();
                Value::nonlinear([&value])
            }
            Expr::Unwrap { .. } | Expr::Same { .. } => {
                let values = self.pop_n(2);
                Value::nonlinear(&values)
            }
            Expr::Method { args, .. } => {
                let values = self.pop_n(1 + args.len());
                Value::nonlinear(&values)
            }
            // An aggregate and each of its projections read everything its parts read.
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } => {
                let values = self.pop_n(ir.exprs[id.index()].kind.operands().len());
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
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::Carrier(_) | Expr::Local(_) | Expr::Param(_) | Expr::Env(_) | Expr::Probe { .. } | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::ParamPrevious(_) | Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) => {
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
        if let Some(callee) = self.returns.get(&(function, args.clone())).cloned() {
            self.called(at, &args, &callee);
            return;
        }
        if self.calls.contains(&function) {
            // A recursive call reads the result known so far (§3 rule 12): the walk of the
            // function that began it grows that result until it holds still (`leave_call`).
            self.recursing.insert(function);
            let known = self.approximate.get(&function).cloned().unwrap_or_else(Value::constant);
            self.called(at, &args, &known);
            return;
        }
        self.start_call(at, function, args, next);
    }

    /// Walks the body of `function`, called at `at` with `args`, in a scope of its own.
    fn start_call(&mut self, at: ExprId, function: FunctionId, args: Vec<Value>, next: &mut Vec<Task>) {
        let ir = self.ir;
        let data = &ir.functions[function.index()];
        let mut callee = Scope::new(format!("function {}", data.name), false, data.locals.len());
        if args.len() != data.params.len() || args.len() > callee.locals.len() {
            self.refuse(at, &format!("call of {} with {} args, it takes {}", data.name, args.len(), data.params.len()));
        }
        for (slot, arg) in callee.locals.iter_mut().zip(&args) {
            *slot = Some(arg.clone());
        }
        callee.enclosed_from = self.enclosing.len();
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
