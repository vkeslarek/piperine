//! `IrBuilder`: the only writer of an [`Ir`]. Ids are handed out in call order, which makes
//! equal call sequences give equal IRs.

use crate::real::Real;
use crate::deps::DepsMemo;
use crate::ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NodeId, StmtId};
use crate::model::{
    BodyData, BodyNode, Branch, Child, Constant, Expr, ExprData, FunctionData, InstanceData, Ir,
    Local, NatureData, NodeData, NodeRef, Origin, Param, Quantity, Site, Stmt, ValueType,
};

/// The parts of a body, in the order a handle lists them.
#[derive(Debug, Clone, PartialEq)]
pub struct BodySpec {
    pub name: String,
    pub origin: Origin,
    pub params: Vec<Param>,
    pub nodes: Vec<BodyNode>,
    pub branches: Vec<Branch>,
    pub limits: Vec<Site>,
    pub stims: Vec<Site>,
    pub locals: Vec<Local>,
    pub analog: Option<Vec<StmtId>>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSpec {
    pub name: String,
    pub origin: Origin,
    pub params: Vec<Local>,
    pub ret: ValueType,
    pub locals: Vec<Local>,
    pub body: ExprId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeSpec {
    pub path: String,
    pub nature: NatureId,
    pub declared_by: InstanceId,
    pub init: Option<Real>,
    pub origin: Origin,
}

/// An instance, added after its parent (pre-order); the builder links it into the parent's
/// children.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceSpec {
    pub path: String,
    pub name: String,
    pub body: BodyId,
    pub parent: Option<InstanceId>,
    pub nodes: Vec<NodeRef>,
    pub params: Vec<Constant>,
    pub origin: Origin,
}

#[derive(Debug, Default)]
pub struct IrBuilder {
    natures: Vec<NatureData>,
    bodies: Vec<BodyData>,
    functions: Vec<FunctionData>,
    exprs: Vec<ExprData>,
    stmts: Vec<Stmt>,
    instances: Vec<InstanceData>,
    nodes: Vec<NodeData>,
}

impl IrBuilder {
    pub fn new() -> Self {
        IrBuilder::default()
    }

    pub fn nature(&mut self, name: &str, potential: Quantity, flow: Option<Quantity>) -> NatureId {
        self.natures.push(NatureData { name: name.to_string(), potential, flow });
        NatureId(self.natures.len() as u32 - 1)
    }

    /// The nature already added under `name`, if any.
    pub fn find_nature(&self, name: &str) -> Option<NatureId> {
        let at = self.natures.iter().position(|n| n.name == name)?;
        Some(NatureId(at as u32))
    }

    pub fn body(&mut self, spec: BodySpec) -> BodyId {
        self.bodies.push(BodyData {
            name: spec.name,
            origin: spec.origin,
            params: spec.params,
            nodes: spec.nodes,
            branches: spec.branches,
            limits: spec.limits,
            stims: spec.stims,
            locals: spec.locals,
            analog: spec.analog,
            children: spec.children,
            deps: DepsMemo::default(),
        });
        BodyId(self.bodies.len() as u32 - 1)
    }

    pub fn function(&mut self, spec: FunctionSpec) -> FunctionId {
        self.functions.push(FunctionData {
            name: spec.name,
            origin: spec.origin,
            params: spec.params,
            ret: spec.ret,
            locals: spec.locals,
            body: spec.body,
        });
        FunctionId(self.functions.len() as u32 - 1)
    }

    /// The function already added under `name`, if any.
    pub fn find_function(&self, name: &str) -> Option<FunctionId> {
        let at = self.functions.iter().position(|f| f.name == name)?;
        Some(FunctionId(at as u32))
    }

    pub fn expr(&mut self, kind: Expr, ty: ValueType, origin: Origin) -> ExprId {
        self.exprs.push(ExprData { kind, ty, origin });
        ExprId(self.exprs.len() as u32 - 1)
    }

    pub fn stmt(&mut self, stmt: Stmt) -> StmtId {
        self.stmts.push(stmt);
        StmtId(self.stmts.len() as u32 - 1)
    }

    pub fn node(&mut self, spec: NodeSpec) -> NodeId {
        self.nodes.push(NodeData {
            path: spec.path,
            nature: spec.nature,
            declared_by: spec.declared_by,
            init: spec.init,
            origin: spec.origin,
        });
        NodeId(self.nodes.len() as u32 - 1)
    }

    /// The id the next `instance` call returns, for nodes that name their declaring instance
    /// before it is added.
    pub fn next_instance_id(&self) -> InstanceId {
        InstanceId(self.instances.len() as u32)
    }

    pub fn instance(&mut self, spec: InstanceSpec) -> InstanceId {
        let id = InstanceId(self.instances.len() as u32);
        if let Some(parent) = spec.parent {
            if let Some(data) = self.instances.get_mut(parent.index()) {
                data.children.push(id);
            }
        }
        self.instances.push(InstanceData {
            path: spec.path,
            name: spec.name,
            body: spec.body,
            parent: spec.parent,
            children: Vec::new(),
            nodes: spec.nodes,
            params: spec.params,
            origin: spec.origin,
        });
        id
    }

    /// Consumes the builder: the `Ir` has no writer left.
    pub fn finish(self, top: InstanceId) -> Ir {
        Ir {
            natures: self.natures,
            bodies: self.bodies,
            functions: self.functions,
            exprs: self.exprs,
            stmts: self.stmts,
            instances: self.instances,
            nodes: self.nodes,
            top,
        }
    }
}
