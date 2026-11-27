//! The reflection face: `Copy` handles that borrow a [`Design`], and the
//! [`Reflect`] trait every declaration handle shares.
//!
//! One file per handle family: `nature.rs`, `module.rs`, `behavior.rs`, `instance.rs` for the
//! hardware declarations, `items.rs` for the value items, `body.rs` for expressions, statements,
//! patterns, types and paths; `handle.rs` holds what every handle shares, `lookups.rs` the
//! lookups of the design.

mod attribute;
mod behavior;
mod body;
mod crates;
mod handle;
mod index;
mod instance;
mod items;
mod lookups;
mod module;
mod nature;

use super::design::Design;
use super::ids::DeclId;

pub use attribute::Attr;
pub use behavior::{Behavior, Reaction};
pub use body::{Argument, Block, Commit, Contribution, Expr, FieldBinding, Pat, Path, SourceFile, Stmt, Type};
pub use crates::Crate;
pub use handle::Reflect;
pub use index::{Reference, Writer};
pub use instance::Instance;
pub use items::{Const, Enum, Field, FnParam, Function, GenericParam, Impl, ImportRef, Namespace, Struct, Trait, Use, Variant};
pub use module::{Binding, Definition, GenArm, Generation, Let, Module, Param, Port, Signal};
pub use nature::{Nature, Quantity};

/// The generic declaration handle: any kind, read through [`Decl::kind`].
#[derive(Clone, Copy)]
pub struct Decl<'d> {
    design: &'d Design,
    id: DeclId,
}
