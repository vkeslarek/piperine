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
mod bounds;
mod closures;
mod compound;
mod continuity;
mod control;
mod cover;
mod definition;
mod derives;
mod enums;
mod event;
mod exits;
mod expr;
mod flow;
mod generics;
mod intrinsic;
mod inference;
mod iterators;
mod lengths;
mod literals;
mod loops;
mod model;
mod module;
mod obligations;
mod patterns;
mod plain;
mod nature_rules;
mod reach;
mod reaction;
mod record;
mod result;
mod substitution;
mod table;
mod ty;
pub(crate) mod unsupported;

pub use self::{aggregate::FieldType, enums::EnumTag, intrinsic::Intrinsic, obligations::Obligation, substitution::Substitution};
pub use self::{generics::{Bound, ConstType, GenericParameter, GenericSignature, ParameterDefault, ParameterKind}, table::{Builtin, Callee, Continuity, Instantiation, Method, Types}, ty::{ConstTerm, GenericValue, Length, Nominal, Ty}};
