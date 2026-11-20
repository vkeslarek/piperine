//! The index types of the [`Ir`](crate::Ir) arenas.
//!
//! Each id is a plain `u32` newtype. They are written out one after another
//! (no macro): ids are data, and the arenas they index never shrink.

use serde::{Deserialize, Serialize};

/// A nature of the design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NatureId(pub u32);

/// A body: one module variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyId(pub u32);

/// A function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FunctionId(pub u32);

/// An expression node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExprId(pub u32);

/// A statement node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StmtId(pub u32);

/// A placed instance of the elaborated design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub u32);

/// A node of the elaborated design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// A discrete net of the elaborated design: the twin of a node for carriers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NetId(pub u32);

/// A source struct or enum of the design: an entry of the type arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TypeId(pub u32);

impl NatureId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl BodyId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl FunctionId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl ExprId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl StmtId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl InstanceId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl NetId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl TypeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
