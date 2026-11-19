//! The Piperine IR: an elaborated design as an object model.
//!
//! Arenas owned by one [`Ir`], addressed by typed `u32` ids. A leaf crate: nothing in-tree
//! is imported. The M1 node set covers the analog subset of `tests/e2e/twins/*.msl`.

mod build;
mod check;
mod contributions;
mod deps;
mod handle;
mod ids;
mod model;
mod print;
mod real;
mod statements;
mod typing;

pub use check::Broken;
pub use contributions::{Condition, Contribution};
pub use deps::{BranchIndex, Deps, ParamIndex, SiteIndex, Target};
pub use build::{BodySpec, FunctionSpec, InstanceSpec, IrBuilder, NetSpec, NodeSpec};
pub use handle::{Body, Function, Instance, Nature, Net, Node, RecordSite};
pub use ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId};
pub use model::{
    Addend, AnalysisKind, BinaryOp, BodyData, BodyNode, Branch, BranchKind, Carrier, CarrierRole, Child, ChildValue, Constant, Derivative, Drive, EmitField, Env, Event, EventSource, Expr,
    ExprData, FunctionData, Hoisted, InstanceData, Inflow, Merge, Reach, ReachedItem, Ir, LevelPart, Local, MathFn, Method, NatureData, NetData, NodeData, NodeRef,
    NodeRole, Operator, Origin, Param, Quantity, QuantityRole, Reaction, Site, Stmt, Stream, StreamField, Terminal, UnaryOp,
    ValueType,
};
pub use real::Real;
