//! Handles over behavior blocks and reactions.

use crate::pom::BodyWalk;
use super::{Block, Commit, Contribution, Decl, Expr, Generation, Module, Path, Reflect};
use crate::pom::body::{BehaviorKind, StmtKind};
use crate::pom::decl::{
    BehaviorData, DeclKind, ReactionData,
};



/// `analog<N> for Target { .. }` or `digital for Target { .. }`
#[derive(Clone, Copy)]
pub struct Behavior<'d> {
    decl: Decl<'d>,
}


impl<'d> Behavior<'d> {
    fn data(self) -> &'d BehaviorData {
        match self.decl.kind() {
            DeclKind::Behavior(b) => b,
            _ => unreachable!("Behavior handle over a non-behavior declaration"),
        }
    }

    pub fn kind(self) -> BehaviorKind {
        self.data().kind
    }

    /// The generic names of `analog<N, M>`, in order.
    pub fn names(self) -> Vec<&'d str> {
        self.data().names.iter().map(|n| n.text.as_str()).collect()
    }

    /// The module this block belongs to: the one its target names, when that
    /// module is declared in the same file and is not `extern`.
    pub fn module(self) -> Option<Module<'d>> {
        self.data().module.and_then(|m| Decl::new(self.decl.design(), m).as_module())
    }

    /// The target path as written, generic arguments included.
    pub fn target(self) -> Path<'d> {
        self.decl.design().path(self.data().target)
    }

    /// The statements of an `analog` block; none for `digital`.
    pub fn statements(self) -> Vec<super::Stmt<'d>> {
        self.data().body.iter().map(|s| self.decl.design().stmt(*s)).collect()
    }

    /// Every contribution of an `analog` block at any depth, in source order.
    pub fn contributions(self) -> Vec<Contribution<'d>> {
        let design = self.decl.design();
        BodyWalk::new(design).nested_statements(&self.data().body)
            .into_iter()
            .filter(|s| matches!(design.stmt_node(*s).kind, StmtKind::Contribution { .. }))
            .map(|s| Contribution::new(design, s))
            .collect()
    }

    /// The reactions of a `digital` block in source order, including those
    /// inside generation.
    pub fn reactions(self) -> Vec<Reaction<'d>> {
        let mut out = Vec::new();
        self.decl.deep_items(&mut out);
        out.into_iter().filter_map(|d| d.as_reaction()).collect()
    }

    /// The generation items written directly in a `digital` block.
    pub fn generations(self) -> Vec<Generation<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generation()).collect()
    }
}


/// `on event { .. }`
#[derive(Clone, Copy)]
pub struct Reaction<'d> {
    decl: Decl<'d>,
}


impl<'d> Reaction<'d> {
    fn data(self) -> &'d ReactionData {
        match self.decl.kind() {
            DeclKind::Reaction(r) => r,
            _ => unreachable!("Reaction handle over a non-reaction declaration"),
        }
    }

    pub fn event(self) -> Expr<'d> {
        self.decl.design().expr(self.data().event)
    }

    /// The body block; the raw block is [`Block::node`].
    pub fn body(self) -> Block<'d> {
        Block::new(self.decl.design(), &self.data().body)
    }

    /// Every commit of the body at any depth, in source order.
    pub fn commits(self) -> Vec<Commit<'d>> {
        let design = self.decl.design();
        BodyWalk::new(design).nested_statements(&self.data().body.stmts)
            .into_iter()
            .filter(|s| matches!(design.stmt_node(*s).kind, StmtKind::Commit { .. }))
            .map(|s| Commit::new(design, s))
            .collect()
    }
}


impl<'d> Reflect<'d> for Behavior<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}


impl PartialEq for Behavior<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}


impl std::fmt::Debug for Behavior<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Behavior({:?})", self.decl.id())
    }
}


impl<'d> Reflect<'d> for Reaction<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}


impl PartialEq for Reaction<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}


impl std::fmt::Debug for Reaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reaction({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_behavior(self) -> Option<Behavior<'d>> {
        matches!(self.kind(), DeclKind::Behavior(_)).then_some(Behavior { decl: self })
    }

    pub fn as_reaction(self) -> Option<Reaction<'d>> {
        matches!(self.kind(), DeclKind::Reaction(_)).then_some(Reaction { decl: self })
    }
}
