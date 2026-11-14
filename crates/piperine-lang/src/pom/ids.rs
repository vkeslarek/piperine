//! The eight index types of the [`Design`](super::Design) arenas.
//!
//! Each id is a plain `u32` newtype. They are written out one after another
//! (no macro): ids are data, and the arenas they index never shrink.

use serde::{Deserialize, Serialize};

/// A source file of the design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileId(pub u32);

/// A crate of the design or of a crate graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CrateId(pub u32);

/// A declaration of any kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeclId(pub u32);

/// An expression node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExprId(pub u32);

/// A statement node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StmtId(pub u32);

/// A pattern node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PatId(pub u32);

/// A type node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TypeId(pub u32);

/// A path node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PathId(pub u32);

impl FileId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl CrateId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl DeclId {
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

impl PatId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl TypeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl PathId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
