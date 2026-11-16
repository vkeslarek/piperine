//! # Type checking of the M1 subset
//!
//! A bidirectional checker over POM ids. [`Types::check`] starts at one module of the root crate
//! and follows what it reaches: placements lead to modules, calls to functions, paths to
//! constants. The result is a side table keyed by POM ids; the [`Design`] is only borrowed and is
//! never written (the POM navigability rule).
//!
//! The result is a table or the reasons there is none, never both.

mod analog;
mod call;
mod carrier;
mod catalogue;
mod check;
mod compound;
mod continuity;
mod control;
mod definition;
mod event;
mod expr;
mod module;
mod reaction;
mod record;
mod table;
mod ty;
mod unsupported;

pub use table::{Builtin, Callee, Continuity, Method, Types};
pub use ty::Ty;
