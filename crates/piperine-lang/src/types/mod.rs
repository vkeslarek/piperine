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
mod assign; mod associated;
mod attributes;
mod call;
mod carrier;
mod catalogue;
mod check;
mod class; mod classes;
mod bounds; mod bundle; mod bundle_handles;
mod closures;
mod coherence;
mod compound;
mod continuity;
mod control; mod coverage;
mod cover;
mod definition;
mod derives; mod element_target;
mod enums;
mod event;
mod exits;
mod expr;
mod flow;
mod generation; mod generic_values; mod generics; mod handles;
mod intrinsic; mod inference;
mod iterators;
mod lengths;
mod literals; mod widths;
mod logic_pattern; mod loops;
mod model;
mod methods;
mod module; mod module_bounds; mod module_generics; mod templates; mod template_items; mod param_requirement;
mod obligations; mod operators;
mod patterns;
mod plain;
mod nature; mod nature_rules;
mod reach;
mod reaction;
mod repr;
mod record;
mod result;
mod substitution;
mod table;
mod traits;
mod ty;
pub(crate) mod unsupported;

pub(crate) use self::{coverage::ElementCoverage, logic_pattern::LogicPattern};
pub use self::{nature::ResolveImpl, traits::{BoundOracle, ImplEntry, ImplTable, ImplTarget}, aggregate::FieldType, enums::EnumTag, intrinsic::Intrinsic, widths::FloatText, obligations::Obligation, substitution::Substitution, param_requirement::ParamRequirement};
pub use self::{generics::{Bound, ConstType, GenericParameter, GenericSignature, ParameterDefault, ParameterKind}, table::{Builtin, Callee, Continuity, Instantiation, Method, Types}, ty::{ConstTerm, GenericValue, Length, Nominal, Ty}};
