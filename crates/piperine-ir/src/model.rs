//! The storage types of the IR: the arenas of [`Ir`], the data they hold, and the
//! value/statement node set of the M1 analog subset.
//!
//! The aggregates (`Ir` and the `*Data` structs) keep their fields crate-private and are read
//! through handles. The small leaf structs a builder caller assembles (`Param`, `BodyNode`,
//! `Child`, ...) have public fields: they are inputs to [`IrBuilder`](crate::IrBuilder).

use serde::{Deserialize, Serialize};

use crate::deps::DepsMemo;
use crate::ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId};
use crate::real::Real;

/// A source position, printed `file:line:col`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

/// An elaborated design: every arena, owned by one value. Written only by `IrBuilder`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ir {
    pub(crate) natures: Vec<NatureData>,
    pub(crate) bodies: Vec<BodyData>,
    pub(crate) functions: Vec<FunctionData>,
    pub(crate) exprs: Vec<ExprData>,
    pub(crate) stmts: Vec<Stmt>,
    pub(crate) instances: Vec<InstanceData>,
    pub(crate) nodes: Vec<NodeData>,
    pub(crate) top: InstanceId,
    /// The discrete nets, the twins of `nodes` for carriers; empty in an analog design.
    #[serde(default)]
    pub(crate) nets: Vec<NetData>,
    /// The authored paths of the nodes a merge removed, each with the node that stands for it now.
    #[serde(default)]
    pub(crate) aliases: Vec<NodeAlias>,
}

/// The authored path of a node that a merge (a fixed zero-potential branch) removed from the arena,
/// and what it stands for after the merge: the surviving node, or ground. A name the author wrote
/// stays reachable (`Ir::node_at`, `Ir::is_ground_alias`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeAlias {
    pub(crate) path: String,
    pub(crate) node: NodeRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NatureData {
    pub(crate) name: String,
    pub(crate) potential: Quantity,
    pub(crate) flow: Option<Quantity>,
}

/// The potential or the flow quantity of a nature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub name: String,
    pub access: String,
    pub units: String,
    pub abstol: Real,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyData {
    pub(crate) name: String,
    pub(crate) origin: Origin,
    pub(crate) params: Vec<Param>,
    pub(crate) nodes: Vec<BodyNode>,
    pub(crate) branches: Vec<Branch>,
    pub(crate) limits: Vec<Site>,
    pub(crate) stims: Vec<Site>,
    pub(crate) locals: Vec<Local>,
    pub(crate) analog: Option<Vec<StmtId>>,
    pub(crate) children: Vec<Child>,
    /// The discrete carriers (`in`/`out` ports of a `Bit` type, `signal`s): ports first.
    #[serde(default)]
    pub(crate) carriers: Vec<Carrier>,
    /// The definitions `c = value;` and `c = value after d;`, in source order.
    #[serde(default)]
    pub(crate) drives: Vec<Drive>,
    /// The events the body's reactions wait on, by index.
    #[serde(default)]
    pub(crate) events: Vec<Event>,
    /// The `on` blocks of the body's `digital` blocks, in source order.
    #[serde(default)]
    pub(crate) reactions: Vec<Reaction>,
    /// The record streams the body's `emit!` sites write, by index.
    #[serde(default)]
    pub(crate) streams: Vec<Stream>,
    /// The derivative sites: each `ddt` that is not a whole addend of an unconditional
    /// contribution, by index (`Expr::Derivative`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) derivatives: Vec<Derivative>,
    /// The modules placed in an expression whose nodes or `analog` statements joined this body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) hoisted: Vec<Hoisted>,
    /// The fixed zero-potential branches dropped from this body: each is one node in every
    /// instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) merges: Vec<Merge>,
    /// The `pub` nodes and carriers of children this body reads (`inst.name`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) reaches: Vec<Reach>,
    /// The ammeters `p.inflow()` adds, one per port read so.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) inflows: Vec<Inflow>,
    /// The param values of each child (by index into `children`) that `Child::params` does not
    /// hold: set by `IrBuilder::child_param`, empty for a child with none. See `ChildValue`.
    #[serde(default)]
    pub(crate) child_params: Vec<Vec<(u32, ChildValue)>>,
    /// The carriers each child's carrier ports are bound to (by index into `children`): one entry
    /// per carrier port of the child's body, in its order, the parent's carrier or none (open).
    /// Set by `IrBuilder::bind_child_carriers`; empty for a child with none.
    #[serde(default)]
    pub(crate) child_carriers: Vec<Vec<Option<u32>>>,
    /// The memo of `Body::dependencies`: derived, so outside equality and serialization.
    #[serde(skip)]
    pub(crate) deps: DepsMemo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: ValueType,
    pub default: Option<Constant>,
    /// The `where` expression; it reads the param itself as `Expr::Param`.
    pub check: Option<ExprId>,
    pub structural: bool,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyNode {
    pub name: String,
    pub nature: NatureId,
    pub role: NodeRole,
    pub init: Option<Real>,
    pub origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Port,
    Signal,
    /// A node of a child the body reaches into (`inst.name`, §8.2.8): the placement binds it to
    /// the child instance's node once the child is placed (`IrBuilder::bind_reached_node`), and
    /// the body's `reaches` list names which one.
    Reached,
}

/// An end of a probe or a branch: a node of the body (by index) or ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terminal {
    Node(u32),
    Ground,
    /// The child's own node `k` (a `signal` of the child's body): valid only in a slot of
    /// `Child::nodes`, where it keeps the authored placement a full view of the source
    /// (analog-structure Q1, S4). `Ir::check` names any other use.
    Own(u32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub plus: Terminal,
    pub minus: Terminal,
    pub kind: BranchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchKind {
    /// Only flow contributions: the sum is the branch's current.
    Flow,
    /// Only potential contributions: the branch has a private flow unknown.
    Potential,
    /// Both quantities may contribute, each under its own conditions (§11 rule 2): in a solve
    /// the enabled kind decides the branch's equation. Its private flow unknown is shaped like a
    /// potential branch's.
    Switch,
}

/// A discrete carrier of a body: `in clk: Bit`, `out hs: Bit = false`, `signal count: Bit<u9> = 0`.
/// Its index in the body's list is its identity; a read of it (`Expr::Carrier`) has the type
/// `Flagged(ty)`, since it is unknown until first written (§3.3.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Carrier {
    pub name: String,
    /// The carried value type: `bool`, `f64` or an integer type.
    pub ty: ValueType,
    pub role: CarrierRole,
    pub init: Option<Constant>,
    pub origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarrierRole {
    In,
    Out,
    Signal,
    /// A carrier of a child the body reads (`inst.name`): read only, like an `in` port, bound to
    /// the child instance's net (`IrBuilder::bind_reached_net`).
    Reached,
}

/// A definition of a carrier: `sample = count == 0;` or, delayed, `g = x after tpd;`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drive {
    pub carrier: u32,
    pub value: ExprId,
    pub after: Option<ExprId>,
    pub origin: Origin,
}

/// An event of a body (§13): what fires it, and a `bool` read at the microstep that must hold
/// for it to fire. `clk.rise()` is `Changed(clk)` guarded by `clk.high()`; `above(e)` is
/// `Cross(e)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub source: EventSource,
    pub guard: Option<ExprId>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventSource {
    /// `c.changed()`: carrier `c` of the body took a new value.
    Changed(u32),
    /// `cross(e, ttol, etol)`: the real `e` went from `<= 0` to `> 0`, located by the solver
    /// within the time tolerance `ttol` and the expression tolerance `etol`, each an `f64` that
    /// reads no probe, carrier or clock, or absent.
    Cross { e: ExprId, ttol: Option<ExprId>, etol: Option<ExprId> },
    /// `at(t)`: simulation time `t`, an `f64` that may be unknown and reads no probe. The event
    /// fires once, when the clock reaches `t`; a later change of what `t` reads re-arms it.
    At(ExprId),
    /// `p.changed()` of a param `p` of the body (§5.3.1): fires when the param's pin changes, a
    /// host `set` during the run (a binding before the run is not a change).
    ParamChanged(u32),
}

/// `on <events> { … }`: statements run when any of the events fires. Its `let`s bind `locals`,
/// its commits (`Stmt::Commit`) write carriers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    /// Indices into the body's events.
    pub on: Vec<u32>,
    pub stmts: Vec<StmtId>,
    pub locals: Vec<Local>,
    pub origin: Origin,
}

/// A record stream of a body (`emit!("sim", …)`): its name and the value fields its sites write.
/// A site may leave a field out; the record then carries the field as unknown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub name: String,
    pub fields: Vec<StreamField>,
}

/// A value field of a stream, in the order a site first wrote it. Text never crosses the ABI and
/// is not a field: it stays on its site (`EmitField::Text`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamField {
    pub name: String,
    /// `bool`, `f64` or an integer type.
    pub ty: ValueType,
}

/// One `name = value` of an `emit!` site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EmitField {
    /// A value, written to field `field` of the stream (its index in `Stream::fields`).
    Value { field: u32, value: ExprId },
    /// A string literal: kept on the site, never sent.
    Text { name: String, text: String },
}

/// A `limit` or `ac_stim` site; its index in the body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    pub origin: Origin,
    /// A `limit` read under a condition: the conjunction of the enclosing conditions, a `bool`
    /// lowered standalone (outside every condition). When it is false the limiter does not run
    /// and the held value stays (abi.md A3, §14.7.4). `None` for a `limit` at the top of the
    /// `analog` block and for every `ac_stim` site, which needs none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<ExprId>,
}

/// A derivative site: a `ddt` whose result is read anywhere but as a whole addend of an
/// unconditional contribution (abi.md A1). Its rate is the private continuous pin `d<k>.rate`,
/// an unknown that is 0 in an equilibrium solve, the integrator's derivative of `state` in a
/// transient step and `jω·state` in a sweep; `Expr::Derivative(k)` reads it. Its index in the
/// body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derivative {
    pub name: String,
    /// The differentiated quantity, an `f64`. It is evaluated at every accepted solution whatever
    /// control flow surrounds the `ddt` (§14 rule 9), so it reads nothing a condition set.
    pub state: ExprId,
    /// `ddt(x, abstol)`: the absolute tolerance of the state, an `f64` that reads no probe,
    /// carrier or clock (a constant or a param expression).
    pub abstol: Option<ExprId>,
    pub origin: Origin,
}

/// A module placed in an expression whose node or `analog` statements joined this body (Appendix
/// C.1.1): `name` is the placement's name in the body (`lag`, `Lag$1`), `module` the placed
/// module. The IR keeps the walkable record so a reader sees what was placed where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hoisted {
    pub name: String,
    pub module: String,
    pub origin: Origin,
}

/// A branch dropped from a body because its kind is fixed to potential and every contribution to
/// it is a zero no condition selects, and nothing reads its flow (§11 rule 3): its two ends are
/// one node in every instance. The ends are terminals of the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Merge {
    pub plus: Terminal,
    pub minus: Terminal,
}

/// A `pub` item of the child placement `child` (an index into `BodyData::children`) that this
/// body reads as `inst.name` (§8.2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reach {
    pub child: u32,
    pub item: ReachedItem,
}

/// What a `Reach` names: the body's own node or carrier of role `Reached` (`node`, `carrier`),
/// and the child's node or carrier it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReachedItem {
    Node { node: u32, child_node: u32 },
    Carrier { carrier: u32, child_carrier: u32 },
}

/// `p.inflow()` (§6 rule 5): the flow entering the module at port `port` (a node index) is
/// measured by the zero-potential branch `branch` (an index into `branches`) between the port's
/// inside node and the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inflow {
    pub port: u32,
    pub branch: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Local {
    pub name: String,
    pub ty: ValueType,
    /// Whether the source wrote `let mut` (§1.5.12): only a mutable local is the target of a
    /// `Stmt::Assign`. A function parameter and a loop variable are never mutable.
    #[serde(default)]
    pub mutable: bool,
}

/// An authored placement inside a body (the tree the device author wrote).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Child {
    pub name: String,
    pub body: BodyId,
    pub nodes: Vec<Terminal>,
    pub params: Vec<(u32, Constant)>,
    pub origin: Origin,
}

/// The value a placement gives one param of its child, as the authored tree shows it
/// (`Child(...) { r: r_top }`). `Child::params` keeps the constant entries; an entry that reads
/// the parent's params lives beside the child (`Body::child_params`), and the flat instance
/// table holds the constant it elaborates to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChildValue {
    Const(Constant),
    /// An expression over the parent body's params: literals, params, operators, casts, math
    /// functions and calls. It reads no node, carrier, local or environment.
    Expr(ExprId),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionData {
    pub(crate) name: String,
    pub(crate) origin: Origin,
    pub(crate) params: Vec<Local>,
    pub(crate) ret: ValueType,
    pub(crate) locals: Vec<Local>,
    pub(crate) body: ExprId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprData {
    pub(crate) kind: Expr,
    pub(crate) ty: ValueType,
    pub(crate) origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Real(Real),
    Bool(bool),
    Unit,
    Local(u32),
    Param(u32),
    Unary { op: UnaryOp, expr: ExprId },
    Binary { op: BinaryOp, lhs: ExprId, rhs: ExprId },
    /// `if cond { then } else { otherwise }`. When `cond` is `Flagged` this is the merge of
    /// §3.3.3: an unknown condition takes both arms and the value (and every commit and record
    /// of the arms) is the merge of what they give, level by level.
    If { cond: ExprId, then: ExprId, otherwise: ExprId },
    Block { stmts: Vec<StmtId>, value: Option<ExprId> },
    Call { function: FunctionId, args: Vec<ExprId> },
    Math { f: MathFn, args: Vec<ExprId> },
    Closure { params: Vec<u32>, body: ExprId },
    Probe { plus: Terminal, minus: Terminal },
    Env(Env),
    Limit { site: u32, x: ExprId, f: ExprId, seed: Option<ExprId> },
    Stim { site: u32, mag: ExprId, phase: Option<ExprId> },
    /// An integer literal; its `UInt<N>` or `Int<N>` type is the expression's type.
    Int(i128),
    /// `expr as to`: among integer types and `f64`, or from `bool` to an integer type.
    Cast { expr: ExprId, to: ValueType },
    /// `receiver.method(args)`: one of the closed set of value methods.
    Method { method: Method, receiver: ExprId, args: Vec<ExprId> },
    /// The literal `Unknown` (§3.3.1): every level unknown; its type is `Flagged`.
    Unknown,
    /// `value.known_or(default)` (§3.3.4): the value when known, `default` otherwise.
    KnownOr { value: ExprId, default: ExprId },
    /// `value.is_known()`: the known flag, itself always known.
    IsKnown(ExprId),
    /// `value.high()` of a `bool`: known and true.
    High(ExprId),
    /// A read of a carrier of the body, by index; its type is `Flagged`.
    Carrier(u32),
    /// `i(a, b)` of a branch the body drives with a potential contribution: the branch's flow,
    /// an `f64`, by branch index. The branch carries a potential contribution in the body.
    Flow { branch: u32 },
    /// `c.prev()`: carrier `c`'s value before its most recent change (§3.3). Its type is
    /// `Flagged`, as a read of the carrier: unknown until the carrier has changed once.
    Previous(u32),
    /// `p.prev()` of a param `p` of the body (§5.3.1): the value `p` held before its most recent
    /// change, of the param's own type; a param is known, so it is never flagged.
    ParamPrevious(u32),
    /// The rate of the derivative site `k` of the body (`Body::derivatives`): an `f64`, read in
    /// the `analog` block, a body `let`, an event argument or a reaction.
    Derivative(u32),
    /// `fired(e)` in a reaction: whether the body's event `e` fired in this microstep (the
    /// `CHANGED` byte of its pin), a known `bool`.
    Fired(u32),
    /// `unwrap`/`expect` of an `Option` carried as the pair `(is, value)`: `value` when `is`
    /// holds, else a fault of the run naming `message` (Rust's `unwrap` text when absent). Its
    /// type is the value's.
    Unwrap { is: ExprId, value: ExprId, message: Option<String> },
    /// One half of `value.level(index)` (§3.3.4), the `Option<bool>` carried as the pair
    /// `(is, value)` that `Unwrap` reads: `part` says which. `value` is any `Repr` value (a
    /// `bool`, an `f64` or an integer type, flagged or not); `index` is a `u32`. The result is
    /// a `bool`, known when `index` is (§3.3.14) and flagged when it is not (§3.3.13).
    Level { value: ExprId, index: ExprId, part: LevelPart },
    /// `merge(a, b)` (§22.1.2): level by level, equal known levels kept and the rest unknown.
    /// `a` and `b` have one base type; the result is that type, `Flagged`.
    Merge { a: ExprId, b: ExprId },
    /// `same(a, b)` (§22.1.4): every level has the same flag and, where known, the same value.
    /// `a` and `b` have one base type; the result is a `bool`, always known.
    Same { a: ExprId, b: ExprId },
    /// `cast::<A, B>(expr)` (§22.1.3): the levels of `expr` under type `to`, which has the same
    /// number of levels. `tags` lists the tags `to` decodes when it is an enum: a pattern outside
    /// them is `Unknown`, so the result is `Flagged`; with no `tags` it is flagged only when
    /// `expr` is.
    LevelCast { expr: ExprId, to: ValueType, tags: Option<Vec<i128>> },
    /// `loop { body }` (§1.8): runs `body` until a `Break`. Its type is its breaks' value type,
    /// `()` for a `break` with no value.
    Loop { body: Vec<StmtId> },
    /// `while cond { body }`: runs `body` while `cond`, a `bool`, holds. Its type is `()`.
    While { cond: ExprId, body: Vec<StmtId> },
    /// `for local in low..high { body }`, `..=` when `inclusive`, walked from the top down when
    /// `reverse` (`.rev()`): `local` takes each integer of the range, `low` and `high` have one
    /// integer type. Its type is `()`.
    ForRange { local: u32, low: ExprId, high: ExprId, inclusive: bool, reverse: bool, body: Vec<StmtId> },
    /// `break` or `break value`: leaves the innermost loop; a value only out of a `Loop`. It
    /// fits any expected type, as it never produces one.
    Break(Option<ExprId>),
    /// `continue`: starts the innermost loop's next iteration. It fits any expected type.
    Continue,
}

impl Expr {
    /// The expressions the form evaluates directly, in evaluation order: not the statements of a
    /// block or a loop body (`Expr::statements`).
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::Unknown => Vec::new(),
            Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::ParamPrevious(_) | Expr::Derivative(_) | Expr::Fired(_) | Expr::Continue => Vec::new(),
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::LevelCast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => vec![*expr],
            Expr::Binary { lhs: a, rhs: b, .. } | Expr::KnownOr { value: a, default: b } | Expr::Unwrap { is: a, value: b, .. } | Expr::Level { value: a, index: b, .. } | Expr::Merge { a, b } | Expr::Same { a, b } => vec![*a, *b],
            Expr::If { cond, then, otherwise } => vec![*cond, *then, *otherwise],
            Expr::Block { value, .. } => value.iter().copied().collect(),
            Expr::Call { args, .. } | Expr::Math { args, .. } => args.clone(),
            Expr::Closure { body, .. } => vec![*body],
            Expr::Method { receiver, args, .. } => [*receiver].into_iter().chain(args.iter().copied()).collect(),
            Expr::Limit { x, f, seed, .. } => [*x, *f].into_iter().chain(*seed).collect(),
            Expr::Stim { mag, phase, .. } => [*mag].into_iter().chain(*phase).collect(),
            Expr::Loop { .. } => Vec::new(),
            Expr::While { cond, .. } => vec![*cond],
            Expr::ForRange { low, high, .. } => vec![*low, *high],
            Expr::Break(value) => value.iter().copied().collect(),
        }
    }

    /// The statements the form runs: a block's, a loop's body; none for any other form.
    pub fn statements(&self) -> &[StmtId] {
        match self {
            Expr::Block { stmts: body, .. } | Expr::Loop { body } | Expr::While { body, .. } | Expr::ForRange { body, .. } => body,
            _ => &[],
        }
    }
}

/// Which half of the `Option<bool>` of `x.level(k)` an `Expr::Level` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelPart {
    /// Whether the level is known: `true` for `Some`, `false` for `None`.
    Is,
    /// The level's value when it is known (`false` when it is not, the form `None` takes).
    Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Let { local: u32, init: ExprId },
    Expr(ExprId),
    Contribute { branch: u32, quantity: QuantityRole, addends: Vec<Addend> },
    /// `carrier <- value;` or `carrier <- value after delay;`, in a reaction.
    Commit { carrier: u32, value: ExprId, after: Option<ExprId> },
    /// `emit!("stream", f = value, …)`, in a reaction: one record of stream `stream` (its index
    /// in the body's streams). Counted with the commits in the reaction's statement numbering.
    Emit { stream: u32, fields: Vec<EmitField> },
    /// `bound_step(value);` at the top level of the `analog` block: the solver's next step is no
    /// longer than `value`, an `f64`. `site` numbers the statements of the body. `when` is the
    /// conjunction of the conditions the source wrote it under, a `bool` lowered standalone: a
    /// step whose solution did not execute the call (`when` false) is capped by nothing
    /// (§14.10.2). `None` for an unconditional call.
    BoundStep { site: u32, value: ExprId, when: Option<ExprId> },
    /// `place = value;`, or `place op= value;` when `op` is set (§1.5.12): the target is a place
    /// of a `mutable` local. Not an ABI statement: it writes a local, never the outside.
    Assign { place: Place, op: Option<BinaryOp>, value: ExprId },
}

/// The target of an assignment: a local and the projections that reach inside it (`s.f`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub local: u32,
    pub path: Vec<Projection>,
}

/// One step into a value from a place: a field of a struct or a tuple, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Projection {
    Field(u32),
}

impl Stmt {
    /// The expressions the statement evaluates directly, in evaluation order: a commit's value
    /// before its delay, an emit's values in field order.
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Stmt::Let { init, .. } => vec![*init],
            Stmt::Expr(e) => vec![*e],
            Stmt::Assign { value, .. } => vec![*value],
            Stmt::Contribute { addends, .. } => addends.iter().map(|a| a.value).collect(),
            Stmt::Commit { value, after, .. } => [*value].into_iter().chain(*after).collect(),
            Stmt::BoundStep { value, when, .. } => [*value].into_iter().chain(*when).collect(),
            Stmt::Emit { fields, .. } => fields
                .iter()
                .filter_map(|f| match f {
                    EmitField::Value { value, .. } => Some(*value),
                    EmitField::Text { .. } => None,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantityRole {
    Flow,
    Potential,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Addend {
    pub operator: Operator,
    pub negated: bool,
    pub value: ExprId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    Direct,
    Ddt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// `And` and `Or` short-circuit, as in the source. `BitAnd`, `BitOr` and `BitXor` are the
/// non-short-circuit `&`, `|` and `^`, on `bool` or on integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// The value methods an expression may call: the numeric rows of Appendix H.1 (integers) and
/// H.2 (`f64`). `Min`, `Max`, `Clamp`, `Signum` and `RemEuclid` take either; `Floor` through
/// `IsInfinite` are `f64` rows; `IntAbs` through `MulOverflows` are integer rows (`f64`'s `abs`
/// is `MathFn::Abs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Min,
    Max,
    Clamp,
    Floor,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sinh,
    Cosh,
    Tanh,
    Powf,
    Powi,
    Log10,
    Log2,
    Ceil,
    Round,
    Trunc,
    Fract,
    Signum,
    RemEuclid,
    Hypot,
    IsNan,
    IsFinite,
    IsInfinite,
    IntAbs,
    Pow,
    SaturatingAdd,
    SaturatingSub,
    SaturatingMul,
    AddOverflows,
    SubOverflows,
    MulOverflows,
}

impl Method {
    /// The MSL name of the method: `sin` for `x.sin()`, `abs` for an integer's `x.abs()`.
    pub fn name(self) -> &'static str {
        match self {
            Method::Min => "min",
            Method::Max => "max",
            Method::Clamp => "clamp",
            Method::Floor => "floor",
            Method::Sin => "sin",
            Method::Cos => "cos",
            Method::Tan => "tan",
            Method::Asin => "asin",
            Method::Acos => "acos",
            Method::Atan => "atan",
            Method::Atan2 => "atan2",
            Method::Sinh => "sinh",
            Method::Cosh => "cosh",
            Method::Tanh => "tanh",
            Method::Powf => "powf",
            Method::Powi => "powi",
            Method::Log10 => "log10",
            Method::Log2 => "log2",
            Method::Ceil => "ceil",
            Method::Round => "round",
            Method::Trunc => "trunc",
            Method::Fract => "fract",
            Method::Signum => "signum",
            Method::RemEuclid => "rem_euclid",
            Method::Hypot => "hypot",
            Method::IsNan => "is_nan",
            Method::IsFinite => "is_finite",
            Method::IsInfinite => "is_infinite",
            Method::IntAbs => "abs",
            Method::Pow => "pow",
            Method::SaturatingAdd => "saturating_add",
            Method::SaturatingSub => "saturating_sub",
            Method::SaturatingMul => "saturating_mul",
            Method::AddOverflows => "add_overflows",
            Method::SubOverflows => "sub_overflows",
            Method::MulOverflows => "mul_overflows",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathFn {
    Exp,
    Ln,
    Sqrt,
    Abs,
}

/// Ordered as declared: the order of `Deps::env`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Env {
    Temperature,
    Gmin,
    SourceScale,
    /// `now()`: the simulation time, an `f64`. Read by `analog` and by reactions, never by a
    /// drive, an event guard or an `at` argument (abi.md §4.2).
    Time,
    /// `analysis(Static)`: whether the analysis has no time axis, a `bool`. Read where `Time` is.
    Static,
    /// `freq()`: the analysis frequency in hertz, an `f64`; 0 outside an AC analysis (§14.4.4).
    /// An analog read: the ABI gives `FREQ` to analog functions only.
    Freq,
    /// `analysis(kind)`: whether the running analysis is of this kind, a `bool` constant during
    /// the analysis (§14.4.1).
    Analysis(AnalysisKind),
}

/// The analysis kinds `analysis(…)` names, ordered as declared (the order of `Deps::env`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnalysisKind {
    Dc,
    Transient,
    Ac,
    Noise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    F64,
    Bool,
    Unit,
    Fn(Vec<ValueType>, Box<ValueType>),
    /// `UInt<N>` (`uN`): `N` bits, arithmetic modulo `2^N`.
    UInt(u32),
    /// `Int<N>` (`iN`): `N` bits, two's complement, arithmetic modulo `2^N`.
    Int(u32),
    /// A value that may be unknown (§3.3): the type of a carrier read and of everything computed
    /// from one. The inner type is `bool`, `f64` or an integer type.
    Flagged(Box<ValueType>),
}

impl ValueType {
    /// Whether this is `UInt<N>` or `Int<N>`.
    pub fn is_integer(&self) -> bool {
        matches!(self, ValueType::UInt(_) | ValueType::Int(_))
    }

    /// The range of an integer type, `None` for any other type.
    pub fn integer_range(&self) -> Option<(i128, i128)> {
        match *self {
            ValueType::UInt(n) if (1..=64).contains(&n) => Some((0, (1i128 << n) - 1)),
            ValueType::Int(n) if (1..=64).contains(&n) => Some((-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constant {
    Real(Real),
    Bool(bool),
    /// An integer; its width and sign are the type of the param or carrier it initializes.
    Int(i128),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceData {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) body: BodyId,
    pub(crate) parent: Option<InstanceId>,
    pub(crate) children: Vec<InstanceId>,
    pub(crate) nodes: Vec<NodeRef>,
    pub(crate) params: Vec<Constant>,
    pub(crate) origin: Origin,
    /// One net per carrier of the body, ports and signals, in the body's order; empty when the
    /// body has no carrier.
    #[serde(default)]
    pub(crate) nets: Vec<NetId>,
}

/// What an instance binds to one node of its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRef {
    Node(NodeId),
    Ground,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    pub(crate) path: String,
    pub(crate) nature: NatureId,
    pub(crate) declared_by: InstanceId,
    pub(crate) init: Option<Real>,
    pub(crate) origin: Origin,
}

/// A discrete net of the design: the carriers bound to it hold one value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetData {
    pub(crate) path: String,
    pub(crate) ty: ValueType,
    pub(crate) declared_by: InstanceId,
    pub(crate) init: Option<Constant>,
    pub(crate) origin: Origin,
}
