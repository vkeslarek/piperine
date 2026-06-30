//! # Piperine Object Model (POM)
//!
//! The reflection API exposes an elaborated Piperine design as a graph of
//! typed runtime objects. See `docs/reflection_api.md` for the full spec.
//!
//! ## Current state
//!
//! Step 1 (auxiliary types): `Value`, `Selection<T>`, `Id`, `Kind`,
//! `ReflectError` are defined here. The concrete node types (`Module`,
//! `Instance`, `Port`, …) will be the renamed `Elab*` types from
//! `crate::elab::ir`, exposed with `pub(crate)` fields and public
//! accessor methods (Steps 3–5).

pub mod error;
pub mod node;
pub mod selection;
pub mod staging;
pub mod value;

pub use error::ReflectError;
pub use node::{Id, Kind};
pub use selection::Selection;
pub use staging::OverrideMap;
pub use value::Value;