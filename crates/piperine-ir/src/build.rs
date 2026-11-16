//! `IrBuilder`: the only writer of an [`Ir`]. Ids are handed out in call order, which makes
//! equal call sequences give equal IRs.

use crate::real::Real;
use crate::deps::DepsMemo;
use crate::ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId};
use crate::model::{
    BodyData, BodyNode, Branch, Carrier, Child, ChildValue, Constant, Drive, Event, Expr, ExprData, FunctionData, InstanceData, Ir,
    Local, NatureData, NetData, NodeData, NodeRef, Origin, Param, Quantity, Reaction, Site, Stmt, Stream, ValueType,
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

/// A discrete net of the design, the twin of `NodeSpec`.
#[derive(Debug, Clone, PartialEq)]
pub struct NetSpec {
    pub path: String,
    pub ty: ValueType,
    pub declared_by: InstanceId,
    pub init: Option<Constant>,
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
    nets: Vec<NetData>,
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
            carriers: Vec::new(),
            drives: Vec::new(),
            events: Vec::new(),
            reactions: Vec::new(),
            streams: Vec::new(),
            child_params: Vec::new(),
            deps: DepsMemo::default(),
        });
        BodyId(self.bodies.len() as u32 - 1)
    }

    /// The body `id` added before; an id this builder never handed out panics, as a handle given
    /// an id out of range does.
    fn body_data(&mut self, id: BodyId) -> &mut BodyData {
        let count = self.bodies.len();
        match self.bodies.get_mut(id.index()) {
            Some(body) => body,
            None => panic!("body id {} out of range ({count} bodies)", id.0),
        }
    }

    /// Adds a carrier to `body`, after the ones added before: returns its index, the identity
    /// `Expr::Carrier` and `Drive` read it by.
    pub fn carrier(&mut self, body: BodyId, carrier: Carrier) -> u32 {
        let carriers = &mut self.body_data(body).carriers;
        carriers.push(carrier);
        carriers.len() as u32 - 1
    }

    /// Adds a definition to `body`, in source order: returns its index.
    pub fn drive(&mut self, body: BodyId, drive: Drive) -> u32 {
        let drives = &mut self.body_data(body).drives;
        drives.push(drive);
        drives.len() as u32 - 1
    }

    /// Adds an event to `body`: returns its index, the identity `Reaction::on` names it by.
    pub fn event(&mut self, body: BodyId, event: Event) -> u32 {
        let events = &mut self.body_data(body).events;
        events.push(event);
        events.len() as u32 - 1
    }

    /// Adds a reaction to `body`, in source order: returns its index.
    pub fn reaction(&mut self, body: BodyId, reaction: Reaction) -> u32 {
        let reactions = &mut self.body_data(body).reactions;
        reactions.push(reaction);
        reactions.len() as u32 - 1
    }

    /// Sets the value `value` of param `param` of child `child` (an index into the body's
    /// children), beside the child (contract D4: `Child` itself is built literally and gains no
    /// field). A param set twice keeps both entries; `Ir::check` names that.
    pub fn child_param(&mut self, body: BodyId, child: u32, param: u32, value: ChildValue) {
        let lists = &mut self.body_data(body).child_params;
        if lists.len() <= child as usize {
            lists.resize(child as usize + 1, Vec::new());
        }
        lists[child as usize].push((param, value));
    }

    /// Adds a record stream to `body`: returns its index, the identity `Stmt::Emit` names it by.
    pub fn stream(&mut self, body: BodyId, stream: Stream) -> u32 {
        let streams = &mut self.body_data(body).streams;
        streams.push(stream);
        streams.len() as u32 - 1
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
            nets: Vec::new(),
        });
        id
    }

    pub fn net(&mut self, spec: NetSpec) -> NetId {
        self.nets.push(NetData { path: spec.path, ty: spec.ty, declared_by: spec.declared_by, init: spec.init, origin: spec.origin });
        NetId(self.nets.len() as u32 - 1)
    }

    /// Binds the carriers of `instance`'s body, in their order, to `nets`. An id this builder
    /// never handed out panics.
    pub fn bind_nets(&mut self, instance: InstanceId, nets: Vec<NetId>) {
        let count = self.instances.len();
        match self.instances.get_mut(instance.index()) {
            Some(data) => data.nets = nets,
            None => panic!("instance id {} out of range ({count} instances)", instance.0),
        }
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
            nets: self.nets,
        }
    }
}
