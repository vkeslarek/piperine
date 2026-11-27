//! The storage types of the IR: the arenas of [`Ir`], the data they hold, and the
//! value/statement node set of the M1 analog subset.
//!
//! The aggregates (`Ir` and the `*Data` structs) keep their fields crate-private and are read
//! through handles. The small leaf structs a builder caller assembles (`Param`, `BodyNode`,
//! `Child`, ...) have public fields: they are inputs to [`IrBuilder`](crate::IrBuilder).

use serde::{Deserialize, Serialize};

use crate::ids::{BodyId, InstanceId, NatureId, NetId, NodeId};
use crate::real::Real;

mod analog;
mod body;
mod digital;
mod function;
mod types;
mod value;

pub use analog::{DelaySite, Derivative, Hoisted, NoiseSource, PartialDerivative, SessionParameter, Site, TraceSite};
pub use body::{BodyData, BodyNode, Branch, BranchKind, Child, ChildValue, Inflow, Local, Merge, NodeRole, Param, Reach, ReachedItem, Terminal};
pub use digital::{Carrier, CarrierRole, CarrierWindow, CommitTarget, Drive, EmitField, EmptyArray, Event, EventSource, NodeArray, Reaction, Stream, StreamField};
pub use function::{FunctionData, GenericInstance};
pub use types::{AggregateData, Constant, Derives, FieldsForm, ReprForm, TypeData, TypeKind, ValueType, VariantData, WideData};
pub use value::{Addend, AnalysisKind, BinaryOp, Env, Expr, ExprData, Extrapolation, LevelPart, MathFn, Method, Operator, Place, Projection, QuantityRole, Stmt, TableControl, TableDegree, UnaryOp};

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
    /// The constants of integer types wider than 64 bits: `Constant::Wide` and `Expr::Wide` name an entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) wides: Vec<WideData>,
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
    /// What the net is made of when it is a resolved net: its drives and its resolver instance (RN-03).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) resolution: Option<Resolution>,
}

/// The holder of one drive of a resolved net: a carrier of an instance. An instance whose two resolved carriers sit on one
/// net holds two drives, one per carrier (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holder {
    /// The instance whose body writes the carrier.
    pub instance: InstanceId,
    /// The carrier's index in the instance's body.
    pub carrier: u32,
}

/// What a resolved net is made of below the POM: one drive net per drive, the resolver instance that combines them and
/// the nets each holder reads `others()` through (RN-03). The value net is the net that holds this record; its type is
/// the nature's `V`, each drive net's is `driven`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resolution {
    /// The generated resolver instance (`<net path>$resolve`).
    pub resolver: InstanceId,
    /// The driven type `X` every drive of the net holds (§5.2.15).
    pub driven: ValueType,
    /// One drive net per drive, in instance-path order, the host's first (§4 rule 2).
    pub drives: Vec<NetId>,
    /// The carrier holding each drive, index for index with `drives` (`None`: the host).
    pub holders: Vec<Option<Holder>>,
    /// The others net of each holder that reads `others()`, by drive index.
    pub others: Vec<(u32, NetId)>,
}
