//! # Type checking of the supported subset
//!
//! A bidirectional checker over POM ids. [`Types::check`] starts at one module of the root crate
//! and follows what it reaches: placements lead to modules, calls to functions, paths to
//! constants. The result is a side table keyed by POM ids; the [`Design`] is only borrowed and is
//! never written (the POM navigability rule).
//!
//! The result is a table or the reasons there is none, never both.

mod analog;
mod attributes;
mod carrier;
mod catalogue;
mod check;
mod continuity;
mod coverage;
mod definition;
mod element_target;
mod event;
mod generics;
mod model;
mod module;
mod nature;
mod nature_rules;
mod pattern;
mod reaction;
mod table;
mod traits;
mod ty;
mod value;

pub(crate) use self::coverage::ElementCoverage;
pub use self::pattern::logic_pattern::{LogicPattern, PatternFault};
pub use self::{nature::ResolveImpl, traits::impl_table::{BoundOracle, ImplEntry, ImplTable, ImplTarget}, value::aggregate::FieldType, value::enums::EnumTag, value::intrinsic::Intrinsic, value::widths::FloatText, generics::obligations::Obligation, generics::substitution::Substitution, module::param_requirement::ParamRequirement};
pub use self::{generics::generics::{Bound, ConstType, GenericParameter, GenericSignature, ParameterDefault, ParameterKind}, table::{Builtin, Callee, Continuity, Instantiation, Method, Types}, ty::{ConstTerm, GenericValue, Length, Nominal, Ty}};
