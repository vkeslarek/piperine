//! The Piperine IR: an elaborated design as an object model.
//!
//! Arenas owned by one [`Ir`], addressed by typed `u32` ids. A leaf crate: nothing in-tree
//! is imported. The M1 node set covers the analog subset of `tests/e2e/twins/*.msl`.

mod build;
mod check;
mod contributions;
mod deps;
mod differentiate;
mod handle;
mod ids;
mod model;
mod print;
mod real;
mod statements;
mod typing;
mod written;

pub use check::Broken;
pub use differentiate::{Differentiation, NotDifferentiable, Variable};
pub use contributions::{Condition, Contribution};
pub use deps::{BranchIndex, Deps, ParamIndex, SiteIndex, Target};
pub use build::{BodySpec, FunctionSpec, InstanceSpec, IrBuilder, NetSpec, NodeSpec};
pub use handle::{Body, Function, Instance, Nature, Net, Node, RecordSite};
pub use ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId, TypeId, AggregateId, WideId};
pub use model::{
    Addend, AggregateData, WideData, AnalysisKind, Derives, FieldsForm, ReprForm, TypeData, TypeKind, VariantData, BinaryOp, BodyData, BodyNode, Branch, BranchKind, Carrier, CarrierRole, Child, ChildValue, Constant, DelaySite, Derivative, Drive, NoiseSource, SessionParameter, TraceSite, PartialDerivative, EmitField, Env, Event, EventSource, Expr,
    ExprData, FunctionData, GenericInstance, Hoisted, InstanceData, Inflow, Merge, Reach, ReachedItem, Ir, LevelPart, Local, MathFn, Method, NatureData, NetData, NodeAlias, NodeData, NodeRef,
    NodeRole, Operator, Origin, Param, Place, Projection, Quantity, QuantityRole, Reaction, Site, Stmt, Stream, StreamField, Terminal, UnaryOp,
    ValueType, Extrapolation, TableControl, TableDegree, CarrierWindow, CommitTarget, EmptyArray, NodeArray,
};
pub use real::Real;
