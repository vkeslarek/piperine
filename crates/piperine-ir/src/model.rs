//! The storage types of the IR: the arenas of [`Ir`], the data they hold, and the
//! value/statement node set of the M1 analog subset.
//!
//! The aggregates (`Ir` and the `*Data` structs) keep their fields crate-private and are read
//! through handles. The small leaf structs a builder caller assembles (`Param`, `BodyNode`,
//! `Child`, ...) have public fields: they are inputs to [`IrBuilder`](crate::IrBuilder).

use serde::{Deserialize, Serialize};

use crate::deps::DepsMemo;
use crate::ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NodeId, StmtId};
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Let { local: u32, init: ExprId },
    Expr(ExprId),
    Contribute { branch: u32, quantity: QuantityRole, addends: Vec<Addend> },
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

/// `And` and `Or` short-circuit, as in the source.
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    F64,
    Bool,
    Unit,
    Fn(Vec<ValueType>, Box<ValueType>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constant {
    Real(Real),
    Bool(bool),
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
