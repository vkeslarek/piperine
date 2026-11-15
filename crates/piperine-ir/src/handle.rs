//! Reading: `Copy` handles that pair a borrow of the [`Ir`] with an id.

use crate::real::Real;
use crate::ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NodeId, StmtId};
use crate::model::{
    BodyData, BodyNode, Branch, Child, Constant, Expr, FunctionData, InstanceData, Ir, Local,
    NatureData, NodeData, NodeRef, NodeRole, Origin, Param, Quantity, Site, Stmt, ValueType,
};

impl Ir {
    /// The kind of an expression. An id out of range panics; `Ir::check` is the gate that makes
    /// that unreachable.
    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()].kind
    }

    pub fn expr_type(&self, id: ExprId) -> &ValueType {
        &self.exprs[id.index()].ty
    }

    pub fn expr_origin(&self, id: ExprId) -> &Origin {
        &self.exprs[id.index()].origin
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.index()]
    }

    pub fn body(&self, id: BodyId) -> Body<'_> {
        Body { ir: self, id }
    }

    /// Every body, in the order they were added.
    pub fn bodies(&self) -> impl Iterator<Item = Body<'_>> {
        (0..self.bodies.len() as u32).map(|i| Body { ir: self, id: BodyId(i) })
    }

    pub fn function(&self, id: FunctionId) -> Function<'_> {
        Function { ir: self, id }
    }

    /// Every function, in the order they were added.
    pub fn functions(&self) -> impl Iterator<Item = Function<'_>> {
        (0..self.functions.len() as u32).map(|i| Function { ir: self, id: FunctionId(i) })
    }

    /// The instance the design is rooted at.
    pub fn top(&self) -> Instance<'_> {
        Instance { ir: self, id: self.top }
    }

    pub fn instance(&self, id: InstanceId) -> Instance<'_> {
        Instance { ir: self, id }
    }

    /// Every instance, in pre-order of the hierarchy.
    pub fn instances(&self) -> impl Iterator<Item = Instance<'_>> {
        (0..self.instances.len() as u32).map(|i| Instance { ir: self, id: InstanceId(i) })
    }

    pub fn node(&self, id: NodeId) -> Node<'_> {
        Node { ir: self, id }
    }

    /// Every node of the design, in the order they were added.
    pub fn nodes(&self) -> impl Iterator<Item = Node<'_>> {
        (0..self.nodes.len() as u32).map(|i| Node { ir: self, id: NodeId(i) })
    }

    /// The instance with this path, or nothing.
    pub fn instance_at(&self, path: &str) -> Option<Instance<'_>> {
        self.instances().find(|i| i.path() == path)
    }

    /// The node with this path, or nothing.
    pub fn node_at(&self, path: &str) -> Option<Node<'_>> {
        self.nodes().find(|n| n.path() == path)
    }

    pub fn nature(&self, id: NatureId) -> Nature<'_> {
        Nature { ir: self, id }
    }

    /// Every nature, in the order they were added.
    pub fn natures(&self) -> impl Iterator<Item = Nature<'_>> {
        (0..self.natures.len() as u32).map(|i| Nature { ir: self, id: NatureId(i) })
    }
}

/// One module variant.
#[derive(Debug, Clone, Copy)]
pub struct Body<'a> {
    ir: &'a Ir,
    id: BodyId,
}

impl<'a> Body<'a> {
    fn data(&self) -> &'a BodyData {
        &self.ir.bodies[self.id.index()]
    }

    pub fn id(&self) -> BodyId {
        self.id
    }

    pub fn ir(&self) -> &'a Ir {
        self.ir
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    pub fn params(&self) -> &'a [Param] {
        &self.data().params
    }

    /// Ports first, then signals.
    pub fn nodes(&self) -> &'a [BodyNode] {
        &self.data().nodes
    }

    pub fn branches(&self) -> &'a [Branch] {
        &self.data().branches
    }

    pub fn limits(&self) -> &'a [Site] {
        &self.data().limits
    }

    pub fn stims(&self) -> &'a [Site] {
        &self.data().stims
    }

    pub fn locals(&self) -> &'a [Local] {
        &self.data().locals
    }

    /// The statements of the `analog` block; `None` when the body has none.
    pub fn analog(&self) -> Option<&'a [StmtId]> {
        self.data().analog.as_deref()
    }

    /// The authored placements, in source order.
    pub fn children(&self) -> &'a [Child] {
        &self.data().children
    }
}

/// A nature.
#[derive(Debug, Clone, Copy)]
pub struct Nature<'a> {
    ir: &'a Ir,
    id: NatureId,
}

impl<'a> Nature<'a> {
    fn data(&self) -> &'a NatureData {
        &self.ir.natures[self.id.index()]
    }

    pub fn id(&self) -> NatureId {
        self.id
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn potential(&self) -> &'a Quantity {
        &self.data().potential
    }

    pub fn flow(&self) -> Option<&'a Quantity> {
        self.data().flow.as_ref()
    }
}

/// A function.
#[derive(Debug, Clone, Copy)]
pub struct Function<'a> {
    ir: &'a Ir,
    id: FunctionId,
}

impl<'a> Function<'a> {
    fn data(&self) -> &'a FunctionData {
        &self.ir.functions[self.id.index()]
    }

    pub fn id(&self) -> FunctionId {
        self.id
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    pub fn params(&self) -> &'a [Local] {
        &self.data().params
    }

    pub fn ret(&self) -> &'a ValueType {
        &self.data().ret
    }

    /// The locals, parameters first.
    pub fn locals(&self) -> &'a [Local] {
        &self.data().locals
    }

    pub fn body(&self) -> ExprId {
        self.data().body
    }
}

/// A placed instance of the elaborated design.
#[derive(Debug, Clone, Copy)]
pub struct Instance<'a> {
    ir: &'a Ir,
    id: InstanceId,
}

impl<'a> Instance<'a> {
    fn data(&self) -> &'a InstanceData {
        &self.ir.instances[self.id.index()]
    }

    pub fn id(&self) -> InstanceId {
        self.id
    }

    pub fn path(&self) -> &'a str {
        &self.data().path
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    pub fn body(&self) -> Body<'a> {
        Body { ir: self.ir, id: self.data().body }
    }

    pub fn parent(&self) -> Option<Instance<'a>> {
        self.data().parent.map(|id| Instance { ir: self.ir, id })
    }

    /// The instances placed inside this one, in declaration order.
    pub fn children(&self) -> impl Iterator<Item = Instance<'a>> {
        let ir = self.ir;
        self.data().children.iter().map(move |&id| Instance { ir, id })
    }

    /// One binding per node of the body, in the body's order.
    pub fn bindings(&self) -> &'a [NodeRef] {
        &self.data().nodes
    }

    /// What node `i` of the body is bound to.
    pub fn binding(&self, i: usize) -> Option<NodeRef> {
        self.data().nodes.get(i).copied()
    }

    /// The design node bound to node `i` of the body; `None` for ground or an index out of range.
    pub fn node(&self, i: usize) -> Option<Node<'a>> {
        match self.binding(i)? {
            NodeRef::Node(id) => Some(Node { ir: self.ir, id }),
            NodeRef::Ground => None,
        }
    }

    /// One bound value per param of the body, defaults applied.
    pub fn params(&self) -> &'a [Constant] {
        &self.data().params
    }

    pub fn param(&self, i: usize) -> Option<&'a Constant> {
        self.data().params.get(i)
    }
}

/// A node of the elaborated design.
#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
    ir: &'a Ir,
    id: NodeId,
}

impl<'a> Node<'a> {
    fn data(&self) -> &'a NodeData {
        &self.ir.nodes[self.id.index()]
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn path(&self) -> &'a str {
        &self.data().path
    }

    pub fn nature(&self) -> Nature<'a> {
        Nature { ir: self.ir, id: self.data().nature }
    }

    pub fn declared_by(&self) -> Instance<'a> {
        Instance { ir: self.ir, id: self.data().declared_by }
    }

    pub fn init(&self) -> Option<Real> {
        self.data().init
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    /// The instances this node is wired to through a port of their body, in pre-order. The
    /// instance that declares a signal binds it as a signal of its own body and is not listed.
    pub fn instances(&self) -> Vec<Instance<'a>> {
        let ir = self.ir;
        ir.instances()
            .filter(|inst| {
                let body = inst.body();
                inst.bindings().iter().enumerate().any(|(i, binding)| {
                    *binding == NodeRef::Node(self.id)
                        && body.nodes().get(i).is_some_and(|n| n.role == NodeRole::Port)
                })
            })
            .collect()
    }
}
