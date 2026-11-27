//! The structure of one body: its params, nodes, branches, children and the records that tie them together.

use serde::{Deserialize, Serialize};

use crate::deps::DepsMemo;
use crate::ids::{BodyId, ExprId, NatureId, StmtId};
use crate::real::Real;
use crate::model::{Carrier, CarrierWindow, Constant, DelaySite, Derivative, Drive, EmptyArray, Event, GenericInstance, Hoisted, NodeArray, NoiseSource, Origin, PartialDerivative, Reaction, SessionParameter, Site, Stream, TraceSite, ValueType};

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
    /// The instance of a generic module this body lowers (GN-15); `None` for a module as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instance: Option<GenericInstance>,
    /// The discrete carriers (`in`/`out` ports of a `Bit` type, `signal`s): ports first.
    #[serde(default)]
    pub(crate) carriers: Vec<Carrier>,
    /// The arrays of nodes: each names `count` consecutive entries of `nodes`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) node_arrays: Vec<NodeArray>,
    /// The arrays declared with no element, each by its name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) empty_arrays: Vec<EmptyArray>,
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
    /// The delay sites: each `delay(x, td, max)` (§14.6), by index (`Expr::Delay`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) delays: Vec<DelaySite>,
    /// The noise sources: each `noise(psd, name)` (§14.8), by index (`Expr::Noise`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) noise_sources: Vec<NoiseSource>,
    /// The session parameters the body reads (`simparam`, §14.4), one per name, by index
    /// (`Expr::SessionParameter`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) session_parameters: Vec<SessionParameter>,
    /// The trace sites: each `emit!` in the `analog` block (§19 rule 4), by index
    /// (`Stmt::Trace`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) traces: Vec<TraceSite>,
    /// The `ddx` calls the elaborator differentiated (§14.2), by index, kept so a reader and the
    /// interpreter see where one was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) partial_derivatives: Vec<PartialDerivative>,
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

/// What a `Reach` names: the body's own node or carrier of role `Reached` or `Resolved` (`node`, `carrier`),
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
