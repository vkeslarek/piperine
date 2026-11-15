//! Reading: `Copy` handles that pair a borrow of the [`Ir`] with an id.

use crate::ids::{BodyId, ExprId, NatureId, StmtId};
use crate::model::{
    BodyData, BodyNode, Branch, Child, Expr, Ir, Local, NatureData, Origin, Param, Quantity, Site,
    Stmt, ValueType,
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
