//! # Piperine Object Model (POM)
//!
//! The reflection API. See `docs/reflection_api.md`.

pub mod behavior;
pub mod bench;
pub mod design;
pub mod error;
pub mod module;
pub mod net_type;
pub mod node;
pub mod selection;
pub mod selector;
pub mod staging;
pub mod traits;
pub use crate::value;

pub use behavior::{Behavior, BehaviorStmt, Function, ImplBlock, MatchArm};
pub use bench::BenchBlock;
pub use design::Design;
pub use error::{ElabError, ElabErrorKind, ReflectError};
pub use module::{Connection, Instance, Module, Param, Port, Var, Wire};
pub use net_type::{NetRef, NetType, TypeRef, ValueType};
pub use node::{Id, Kind};
pub use selection::Selection;
pub use staging::OverrideMap;
pub use traits::{Kinded, Named, NetTyped};
pub use value::Value;