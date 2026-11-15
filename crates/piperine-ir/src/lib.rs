//! The Piperine IR: an elaborated design as an object model.
//!
//! Arenas owned by one [`Ir`], addressed by typed `u32` ids. A leaf crate: nothing in-tree
//! is imported. The M1 node set covers the analog subset of `tests/ngspice/*.msl`.

mod build;
mod ids;
mod model;
mod real;

pub use build::{BodySpec, FunctionSpec, InstanceSpec, IrBuilder, NodeSpec};
pub use ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NodeId, StmtId};
pub use model::{
    Addend, BinaryOp, BodyData, BodyNode, Branch, BranchKind, Child, Constant, Env, Expr,
    ExprData, FunctionData, InstanceData, Ir, Local, MathFn, NatureData, NodeData, NodeRef,
    NodeRole, Operator, Origin, Param, Quantity, QuantityRole, Site, Stmt, Terminal, UnaryOp,
    ValueType,
};
pub use real::Real;
