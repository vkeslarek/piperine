//! The Piperine IR: an elaborated design as an object model.
//!
//! Arenas owned by one [`Ir`], addressed by typed `u32` ids. A leaf crate: nothing in-tree
//! is imported. The M1 node set covers the analog subset of `tests/ngspice/*.msl`.

mod build;
mod check;
mod deps;
mod handle;
mod ids;
mod model;
mod print;
mod real;

pub use check::Broken;
pub use deps::{BranchIndex, Deps, ParamIndex, SiteIndex, Target};
pub use build::{BodySpec, FunctionSpec, InstanceSpec, IrBuilder, NodeSpec};
pub use handle::{Body, Function, Instance, Nature, Node};
pub use ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NodeId, StmtId};
pub use model::{
    Addend, BinaryOp, BodyData, BodyNode, Branch, BranchKind, Child, Constant, Env, Expr,
    ExprData, FunctionData, InstanceData, Ir, Local, MathFn, NatureData, NodeData, NodeRef,
    NodeRole, Operator, Origin, Param, Quantity, QuantityRole, Site, Stmt, Terminal, UnaryOp,
    ValueType,
};
pub use real::Real;
