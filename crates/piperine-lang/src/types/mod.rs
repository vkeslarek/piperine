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
mod callee;
mod carrier;
mod catalogue;
mod check;
mod generics;
mod module;
mod pattern;
mod result_table;
mod traits;
mod ty;
mod value;
mod whole_model;

pub(crate) use self::carrier::coverage::ElementCoverage;
pub use self::pattern::logic_pattern::{LogicPattern, PatternFault};
pub use self::{analog::nature::ResolveImpl, traits::impl_table::{BoundOracle, ImplEntry, ImplTable, ImplTarget}, value::aggregate::FieldType, value::enums::EnumTag, value::intrinsic::Intrinsic, value::widths::FloatText, generics::obligations::Obligation, generics::substitution::Substitution, module::param_requirement::ParamRequirement};
pub use self::{generics::generics::{Bound, ConstType, GenericParameter, GenericSignature, ParameterDefault, ParameterKind}, callee::{Builtin, Callee, Method}, result_table::{Continuity, Instantiation, Types}, ty::{ConstTerm, GenericValue, Length, Nominal, Ty}};
