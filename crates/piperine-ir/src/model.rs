//! The storage types of the IR: the arenas of [`Ir`], the data they hold, and the
//! value/statement node set of the M1 analog subset.
//!
//! The aggregates (`Ir` and the `*Data` structs) keep their fields crate-private and are read
//! through handles. The small leaf structs a builder caller assembles (`Param`, `BodyNode`,
//! `Child`, ...) have public fields: they are inputs to [`IrBuilder`](crate::IrBuilder).

use serde::{Deserialize, Serialize};

use crate::deps::DepsMemo;
use crate::ids::{AggregateId, BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId, TypeId};
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
    /// The source structs and enums the values of the design are typed by: `ValueType::Struct` and
    /// `ValueType::Enum` name an entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) types: Vec<TypeData>,
    /// The composite constants: `Constant::Aggregate` names an entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) aggregates: Vec<AggregateData>,
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
    /// The arrays of nodes: each names `count` consecutive entries of `nodes`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) node_arrays: Vec<NodeArray>,
    /// The window of an array of carriers each child's port is given, parallel to `child_carriers`: `None` for a port
    /// bound whole or to a scalar carrier.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) child_windows: Vec<Vec<Option<CarrierWindow>>>,
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
/// `ty`: a carrier holds a value from `(0, 0)` on (§3.3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Carrier {
    pub name: String,
    /// The carried value type: `bool`, `f64` or an integer type.
    pub ty: ValueType,
    pub role: CarrierRole,
    pub init: Option<Constant>,
    pub origin: Origin,
    /// `Some(k)`: an array of `k` carriers of type `ty`, one array net stored sparsely (CV-23, §5.1.8); `None`: one
    /// carrier. An element form (`Expr::CarrierElement` and the rest) names an array carrier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
}

/// An array of nodes (`signal bl: [Electrical; 4]`): `count` consecutive `BodyNode`s from `first` named `bl[0]`,
/// `bl[1]`, ..., kept as one name for the host and the printer (CV-23, CV-26).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeArray {
    pub name: String,
    /// The body index of element 0.
    pub first: u32,
    pub count: u32,
    pub origin: Origin,
}

/// The part of an array of carriers a child's port is given (CV-23): a run of `length` elements from `first`, or one
/// element as a scalar when `length` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierWindow {
    pub first: u64,
    pub length: Option<u64>,
}

/// What a commit or a definition names inside its carrier (CV-25): an element of an array of carriers, then
/// projections into the element's value (a field, an element of a value array). Empty is the whole carrier.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CommitTarget {
    /// The element of an array carrier, an integer expression.
    pub element: Option<ExprId>,
    /// The steps into the value, outermost first.
    pub path: Vec<Projection>,
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
    /// The element of an array carrier the definition names (`bits[0] = a;`, CV-25); `None` for the whole carrier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<u64>,
    /// The steps into the value (a field, an element of a value array) the definition names inside the carrier or
    /// its element; empty for all of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Projection>,
}

/// An event of a body (§13): what fires it, and a `bool` read at the microstep that must hold
/// for it to fire. `clk.rise()` is `Changed(clk)` guarded by a read of `clk`; `above(e)` is
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
    /// `at(t)`: simulation time `t`, an `f64` that reads no probe. The event
    /// fires once, when the clock reaches `t`; a later change of what `t` reads re-arms it.
    At(ExprId),
    /// `p.changed()` of a param `p` of the body (§5.3.1): fires when the param's pin changes, a
    /// host `set` during the run (a binding before the run is not a change).
    ParamChanged(u32),
    /// `settle` (§13 rule 17): fires once per time point, in the settle phase that closes the
    /// instant (§15 step 4), when its guard, if any, holds.
    Settle,
    /// `x[k].changed()`, `.rise()`, `.fall()` of element `element` of the array carrier `carrier` (CV-24): the
    /// element took a new value; a constant element, so the plan gives it its own private event pin.
    ElementChanged { carrier: u32, element: u64 },
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
/// A site that leaves a field out carries the field's `Default` (§19 rule 11).
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
    /// Whether the body assigns the local (§1.5.12), whatever its `let` wrote: only an assignable
    /// local is the target of a `Stmt::Assign`. A function parameter and a loop variable are never
    /// assignable, and a generated `let` keeps `mut` exactly for a local that is.
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
    /// The generic item this function is one instantiation of; `None` for a function as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instance: Option<GenericInstance>,
}

/// One instantiation of a generic item: the item and the arguments, each printed as the source
/// spells it (`swap::<f64>` is item `swap`, arguments `["f64"]`). The IR name of the instance is
/// [`GenericInstance::name`], which `Ir::check` holds the function or type to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericInstance {
    /// The generic item's name as written.
    pub item: String,
    /// The generic arguments in order, a type, a const value or a function, each as printed.
    pub arguments: Vec<String>,
}

impl GenericInstance {
    /// The name of the instance, `item::<a, b>` (design D8): what its IR function carries.
    pub fn name(&self) -> String {
        format!("{}::<{}>", self.item, self.arguments.join(", "))
    }

    /// The name of the instance of a generic type, `item<a, b>` (design D8).
    pub fn type_name(&self) -> String {
        format!("{}<{}>", self.item, self.arguments.join(", "))
    }

    /// Whether `name` is this instance's name, or that name followed by the half of an `Option`
    /// result the function computes (`.is` or `.val`, `elab/variants.rs`).
    pub fn names(&self, name: &str) -> bool {
        let own = self.name();
        name.strip_prefix(&own).is_some_and(|rest| matches!(rest, "" | ".is" | ".val"))
    }
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
    /// `if cond { then } else { otherwise }`: runs the arm `cond` selects.
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
    /// A read of a carrier of the body, by index; its type is the carrier's.
    Carrier(u32),
    /// `i(a, b)` of a branch the body drives with a potential contribution: the branch's flow,
    /// an `f64`, by branch index. The branch carries a potential contribution in the body.
    Flow { branch: u32 },
    /// `c.prev()`: carrier `c`'s value before its most recent change (§3.3), of the carrier's type;
    /// before the first change it is the initial value.
    Previous(u32),
    /// `p.prev()` of a param `p` of the body (§5.3.1): the value `p` held before its most recent
    /// change, of the param's own type.
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
    /// `same(a, b)` (§22.1.4): every level has the same value. `a` and `b` have one type; the
    /// result is a `bool`.
    Same { a: ExprId, b: ExprId },
    /// `cast::<A, B>(expr)` (§22.1.3): the levels of `expr` under type `to`, which has the same
    /// number of levels; the result is an `Option<to>`, `None` for a tag that names no variant.
    /// `tags` lists the tags `to` decodes when it is an enum.
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
    /// `return` or `return value` in a function body: the value has the function's result type.
    /// It fits any expected type.
    Return(Option<ExprId>),
    /// A function used as a value (§1.5.10): the name of a function, of type `Fn`.
    FunctionRef(FunctionId),
    /// `callee(args)` where `callee` is a value of type `Fn`, not a named function.
    CallValue { callee: ExprId, args: Vec<ExprId> },
    /// `(a, b, ..)`: a tuple of the operands, lowest field first. Its type is `Tuple`.
    Tuple(Vec<ExprId>),
    /// `[a, b, ..]`: an array of the operands, all of one type. Its type is `Array`.
    Array(Vec<ExprId>),
    /// `[element; count]`: `count` copies of one value (§1.5.25).
    ArrayRepeat { element: ExprId, count: u64 },
    /// `S { x: a, y: b }` or `S(a, b)`: a struct of the arena, its fields in declaration order.
    StructOf { ty: TypeId, fields: Vec<ExprId> },
    /// A variant of an enum of the arena with its fields in declaration order.
    VariantOf { ty: TypeId, variant: u32, fields: Vec<ExprId> },
    /// `expr.field`, `expr.0`: field `index` of a struct or a tuple.
    Field { expr: ExprId, index: u32 },
    /// `expr[index]`: an element of an array; `index` has an integer type.
    Index { expr: ExprId, index: ExprId },
    /// `expr.len()`: the length of an array, a `UInt<64>`.
    Length(ExprId),
    /// `matches!(expr, Enum::Variant ..)`: whether the enum value is variant `variant`. A `bool`.
    IsVariant { expr: ExprId, variant: u32 },
    /// Field `field` of variant `variant` of an enum value: defined where `IsVariant` holds.
    VariantField { expr: ExprId, variant: u32, field: u32 },
    /// The `Option<T>` made from the pair `(is, value)` that value code keeps (MD-67): `is` a
    /// `bool`, `value` a `T`.
    OptionPack { is: ExprId, value: ExprId },
    /// One half of an `Option<T>` as the pair: `Is` gives a `bool`, `Value` the `T`.
    OptionPart { option: ExprId, part: LevelPart },
    /// `T::default()` (H.8 rule 4): the type is the one written.
    Default(ValueType),
    /// `table_model(x, table, control)` (§22.2.15, PI-10): the interpolant of the `table` of `(abscissa,
    /// value)` samples, strictly increasing in the abscissa, read at `x`, an `f64`. The table and the
    /// control are elaboration constants, so the form holds them and the run only evaluates it.
    Table { x: ExprId, table: Vec<(Real, Real)>, control: TableControl },
    /// The derivative of the `Table` with the same `table` and `control` with respect to `x`: the slope of
    /// its interpolant at `x` (§22.2.15 rule 5). An `f64`.
    TableSlope { x: ExprId, table: Vec<(Real, Real)>, control: TableControl },
    /// `x[index]`: the value of one element of the array carrier `carrier`, `index` an integer expression (any
    /// integer; a run-time index past the end is a fault of the run, CV-25 AC 5). Typed as the carrier's `ty`.
    CarrierElement { carrier: u32, index: ExprId },
    /// `x[index].prev()`: the value an element had before its last change; typed as the carrier's `ty`.
    ElementPrevious { carrier: u32, index: ExprId },
    /// `x[index].changed()` read as a value in a reaction (§13 rule 9): a `bool`.
    ElementChanged { carrier: u32, index: ExprId },
}

/// The control string of a `table_model` as the elaborator parsed it (§22.2.15 rules 2 to 4): the
/// interpolation, and the extrapolation at each end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableControl {
    /// The interpolation character.
    pub degree: TableDegree,
    /// What happens below the first abscissa.
    pub low: Extrapolation,
    /// What happens above the last abscissa.
    pub high: Extrapolation,
}

/// The interpolation of a `table_model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableDegree {
    /// `D`: the closest sample, no interpolation.
    Closest,
    /// `1`: linear.
    Linear,
    /// `2`: a quadratic spline.
    Quadratic,
    /// `3`: a cubic spline.
    Cubic,
}

/// What a `table_model` does outside its table, at one end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Extrapolation {
    /// `C`: the end value.
    Constant,
    /// `L`: the line of the end slope.
    Linear,
    /// `E`: an error, `E0705`, when `x` is outside the table.
    Error,
}

impl TableControl {
    /// The control string that parses to this control: the interpolation character then the two
    /// extrapolation characters.
    pub fn text(&self) -> String {
        let degree = match self.degree {
            TableDegree::Closest => 'D',
            TableDegree::Linear => '1',
            TableDegree::Quadratic => '2',
            TableDegree::Cubic => '3',
        };
        let end = |extrapolation: Extrapolation| match extrapolation {
            Extrapolation::Constant => 'C',
            Extrapolation::Linear => 'L',
            Extrapolation::Error => 'E',
        };
        format!("{degree}{}{}", end(self.low), end(self.high))
    }
}

impl Expr {
    /// The expressions the form evaluates directly, in evaluation order: not the statements of a
    /// block or a loop body (`Expr::statements`).
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) => Vec::new(),
            Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::ParamPrevious(_) | Expr::Derivative(_) | Expr::Fired(_) | Expr::Continue | Expr::FunctionRef(_) => Vec::new(),
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::LevelCast { expr, .. } => vec![*expr],
            Expr::Binary { lhs: a, rhs: b, .. } | Expr::Unwrap { is: a, value: b, .. } | Expr::Same { a, b } => vec![*a, *b],
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
            Expr::Break(value) | Expr::Return(value) => value.iter().copied().collect(),
            Expr::CallValue { callee, args } => [*callee].into_iter().chain(args.iter().copied()).collect(),
            Expr::Tuple(parts) | Expr::Array(parts) | Expr::StructOf { fields: parts, .. } | Expr::VariantOf { fields: parts, .. } => parts.clone(),
            Expr::ArrayRepeat { element, .. } => vec![*element],
            Expr::Field { expr, .. } | Expr::Length(expr) | Expr::IsVariant { expr, .. } | Expr::VariantField { expr, .. } => vec![*expr],
            Expr::OptionPack { is, value } => vec![*is, *value],
            Expr::OptionPart { option, .. } => vec![*option],
            Expr::Default(_) => Vec::new(),
            Expr::Table { x, .. } | Expr::TableSlope { x, .. } => vec![*x],
            Expr::Index { expr, index } => vec![*expr, *index],
            Expr::CarrierElement { index, .. } | Expr::ElementPrevious { index, .. } | Expr::ElementChanged { index, .. } => vec![*index],
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
    Commit { carrier: u32, value: ExprId, after: Option<ExprId>, #[serde(default, skip_serializing_if = "Option::is_none")] target: Option<CommitTarget> },
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
    /// The index of the local the assignment writes.
    pub local: u32,
    /// The steps from the local to the written part, outermost first; empty for the whole local.
    pub path: Vec<Projection>,
}

/// One step into a value from a place: a field of a struct or a tuple, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Projection {
    /// `.n` or `.name`: the field at this index of a struct or a tuple.
    Field(u32),
    /// `[index]`: an element of an array, the index an expression of an integer type.
    Index(ExprId),
}

impl Place {
    /// The index expressions of the place's projections, outermost first.
    pub fn index_operands(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.path.iter().filter_map(|step| match step {
            Projection::Index(index) => Some(*index),
            Projection::Field(_) => None,
        })
    }
}

impl CommitTarget {
    /// The index expressions the target holds: the element, then the projections' indices, outermost first.
    pub fn index_operands(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.element.into_iter().chain(self.path.iter().filter_map(|step| match step {
            Projection::Index(index) => Some(*index),
            Projection::Field(_) => None,
        }))
    }
}

impl Stmt {
    /// The expressions the statement evaluates directly, in evaluation order: a commit's value
    /// before its delay, an emit's values in field order.
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Stmt::Let { init, .. } => vec![*init],
            Stmt::Expr(e) => vec![*e],
            Stmt::Assign { place, value, .. } => [*value].into_iter().chain(place.index_operands()).collect(),
            Stmt::Contribute { addends, .. } => addends.iter().map(|a| a.value).collect(),
            Stmt::Commit { value, after, target, .. } => {
                [*value].into_iter().chain(*after).chain(target.iter().flat_map(CommitTarget::index_operands)).collect()
            }
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
    /// A tuple `(A, B, ..)`: its fields, lowest first.
    Tuple(Vec<ValueType>),
    /// An array `[T; N]`: the element type and the length.
    Array(Box<ValueType>, u64),
    /// A source struct, named by its entry of the type arena.
    Struct(TypeId),
    /// A source enum, named by its entry of the type arena.
    Enum(TypeId),
    /// An `Option<T>` held as one value: a field, an element, an item, a carrier.
    Option(Box<ValueType>),
}

/// How the fields of a struct or a variant are written in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldsForm {
    /// `{ x: f64, y: f64 }`: the fields have names.
    Named,
    /// `(f64, f64)`: the fields are numbered, `0`, `1`, ...
    Tuple,
    /// No fields.
    Unit,
}

/// The traits of Appendix H.8 a source type derives, one flag each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derives {
    pub clone: bool,
    pub copy: bool,
    pub partial_eq: bool,
    pub eq: bool,
    pub partial_ord: bool,
    pub ord: bool,
    pub default: bool,
    pub debug: bool,
    pub hash: bool,
    /// `Repr` (§2): the type crosses the device ABI.
    pub repr: bool,
}

/// One source struct or enum: its name, where it was written, and its fields or variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeData {
    /// The bare name, `Pair`.
    pub name: String,
    /// The path of the declaration in the source, `bus::Pair`.
    pub path: String,
    pub origin: Origin,
    pub kind: TypeKind,
    pub derives: Derives,
    /// The generic item this type is one instantiation of (`Pair<f64>`); `None` for a type as
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<GenericInstance>,
    /// How the type crosses the device ABI: derived from its fields (§2) or written by the author.
    #[serde(default, skip_serializing_if = "ReprForm::is_derived")]
    pub repr: ReprForm,
}

/// Where the layout of a type that implements `Repr` comes from (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReprForm {
    /// `#[derive(Repr)]`: the layout follows the fields.
    #[default]
    Derived,
    /// `impl Repr for T`: `width` levels, written to levels by the function `encode` (the type to
    /// `[bool; width]`) and read back by `decode` (`[bool; width]` to `Option` of the type).
    HandWritten { width: u32, encode: FunctionId, decode: FunctionId },
}

impl ReprForm {
    /// Whether the layout is derived, which serde leaves unwritten.
    pub fn is_derived(&self) -> bool {
        matches!(self, ReprForm::Derived)
    }
}

/// The shape of a source type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeKind {
    /// A struct and its fields, named or numbered (the names of a tuple struct are `0`, `1`, ..).
    Struct { form: FieldsForm, fields: Vec<(String, ValueType)> },
    /// An enum: the width of its tag in levels (§2 rule 5) and its variants.
    Enum { tag_levels: u32, variants: Vec<VariantData> },
}

/// One variant of an enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantData {
    pub name: String,
    /// The tag value of the variant (§1.3.7).
    pub discriminant: i128,
    pub form: FieldsForm,
    pub fields: Vec<(String, ValueType)>,
}

impl ValueType {
    /// Whether this is a tuple, an array, a struct or an `Option`: a value of parts, which a
    /// param or a pin holds as bytes rather than as one scalar. An enum is not counted (its
    /// params belong to `params-and-io`).
    pub fn is_composite(&self) -> bool {
        matches!(self, ValueType::Tuple(_) | ValueType::Array(..) | ValueType::Struct(_) | ValueType::Option(_))
    }

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
    /// An integer; its width and sign are the type of the param or carrier it initializes. For a type with a
    /// hand-written `Repr` it is the levels word: level `k` is bit `k`.
    Int(i128),
    /// A composite value (a tuple, an array, a struct, an enum variant, an `Option`): an entry of
    /// the arena of composite constants. The handle keeps `Constant` `Copy`.
    Aggregate(AggregateId),
}

/// One composite constant: its type and its parts in declaration order (§1.5.25). A part is
/// itself a `Constant`, so aggregates nest through `Constant::Aggregate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateData {
    /// The type of the value: a tuple, an array, a struct, an enum or an `Option`.
    pub ty: ValueType,
    /// The variant of an enum value; `None` for every other type.
    pub variant: Option<u32>,
    /// The fields, the elements, or the fields of the variant. An `Option` has none (`None`) or
    /// one (`Some`).
    pub parts: Vec<Constant>,
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
