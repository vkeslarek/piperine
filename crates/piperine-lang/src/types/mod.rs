//! # Type checking of the M1 subset
//!
//! A bidirectional checker over POM ids. [`Types::check`] starts at one module of the root crate
//! and follows what it reaches: placements lead to modules, calls to functions, paths to
//! constants. The result is a side table keyed by POM ids; the [`Design`] is only borrowed and is
//! never written (the POM navigability rule).
//!
//! The result is a table or the reasons there is none, never both.

mod aggregate;
mod aggregate_patterns;
mod analog;
mod assign;
mod attributes;
mod call;
mod carrier;
mod catalogue;
mod check;
mod class;
mod closures;
mod compound;
mod continuity;
mod control;
mod cover;
mod definition;
mod enums;
mod event;
mod exits;
mod expr;
mod flag_total;
mod flow;
mod intrinsic;
mod lengths;
mod literals;
mod loops;
mod module;
mod patterns;
mod reach;
mod reaction;
mod record;
mod result;
mod table;
mod ty;
pub(crate) mod unsupported;

pub use intrinsic::Intrinsic;
pub use table::{Builtin, Callee, Continuity, Method, Types};
pub use ty::Ty;
