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
mod bounds;
mod bundle;
mod bundle_handles;
mod carrier;
mod catalogue;
mod check;
mod pattern;
mod coherence;
mod continuity;

mod coverage;
mod definition;
mod derives;
mod element_target;
mod event;
mod generation;
mod generic_values;
mod generics;
mod handles;
mod inference;

mod model;
mod module;
mod module_bounds;
mod module_generics;
mod nature;
mod nature_rules;
mod obligations;
mod param_requirement;

mod reach;
mod reaction;
mod record;
mod repr;
mod result;
mod substitution;
mod table;
mod template_items;
mod templates;
mod traits;
mod ty;
mod value;

pub(crate) use self::coverage::ElementCoverage;
pub use self::pattern::logic_pattern::{LogicPattern, PatternFault};
pub use self::{nature::ResolveImpl, traits::{BoundOracle, ImplEntry, ImplTable, ImplTarget}, value::aggregate::FieldType, value::enums::EnumTag, value::intrinsic::Intrinsic, value::widths::FloatText, obligations::Obligation, substitution::Substitution, param_requirement::ParamRequirement};
pub use self::{generics::{Bound, ConstType, GenericParameter, GenericSignature, ParameterDefault, ParameterKind}, table::{Builtin, Callee, Continuity, Instantiation, Method, Types}, ty::{ConstTerm, GenericValue, Length, Nominal, Ty}};
