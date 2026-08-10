//! The IR core (MD-40): five concepts — value class, net, value graph,
//! cell, block — plus arena storage. One file per concept; this module
//! wires them together with the id newtypes every concept crosses.
//!
//! Id newtypes are forward-declared here so each concept file can refer to
//! any id without import cycles: `Cell.writer: BlockId` needs the id in
//! `cell.rs` even though `Block` itself is defined in `block.rs`.
//!
//! Nothing in the public API uses `usize` — distinct newtypes prevent a
//! `CellId` being passed where a `ValueId` belongs.

mod module;
mod value;

pub use module::{Module, Program};
pub use value::{Const, Ty};

/// A typed value slot's id — the `Module::values` arena maps this to its [`Ty`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub u32);

/// A constant's id (the `Module::consts` arena).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConstId(pub u32);

/// A declared cell's id. The single-writer rule is a field on `Cell` (T4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellId(pub u32);

/// A declared net's id (T3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetId(pub u32);

/// A declared block's id (T4 references it; `Block` itself is T5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

/// A discipline nature's id — the IR carries the id, never the name (T3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NatureId(pub u32);

/// An intrinsic-catalog entry's id (T5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntrinsicId(pub u32);
