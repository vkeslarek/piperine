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
}

/// An end of a probe or a branch: a node of the body (by index) or ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terminal {
    Node(u32),
    Ground,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub plus: Terminal,
    pub minus: Terminal,
    pub kind: BranchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchKind {
    Flow,
    Potential,
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
    /// `cross(e)`: the real `e` went from `<= 0` to `> 0`, located by the solver.
    Cross(ExprId),
    /// `at(t)`: simulation time `t`, an `f64` that may be unknown and reads no probe. The event
    /// fires once, when the clock reaches `t`; a later change of what `t` reads re-arms it.
    At(ExprId),
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Local {
    pub name: String,
    pub ty: ValueType,
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
}

impl Stmt {
    /// The expressions the statement evaluates directly, in evaluation order: a commit's value
    /// before its delay, an emit's values in field order.
    pub fn operands(&self) -> Vec<ExprId> {
        match self {
            Stmt::Let { init, .. } => vec![*init],
            Stmt::Expr(e) => vec![*e],
            Stmt::Contribute { addends, .. } => addends.iter().map(|a| a.value).collect(),
            Stmt::Commit { value, after, .. } => [*value].into_iter().chain(*after).collect(),
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

/// The value methods an expression may call: `a.min(b)`, `a.max(b)`, `x.clamp(lo, hi)` on an
/// integer or `f64`, and `x.floor()` on `f64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Min,
    Max,
    Clamp,
    Floor,
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
