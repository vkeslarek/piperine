//! The intrinsic catalog — one declaration per intrinsic (MD-24: the
//! declared language surface, one place, read by both backends and the
//! interpreter). A trait ([`Intrinsic`]) carries the five questions
//! ([`Signature`], [`StateShape`], [`DerivativeRule`], [`Breakpoint`],
//! [`LegalDomains`]) and a data table ([`Catalog::entries`]) lists the
//! entries — table + trait, never a macro (MD-13 rule 5). The family
//! modules below are the implementation behind one or more entries.

mod table;
mod types;
#[cfg(test)]
mod tests;

pub use table::{Catalog, Family, Intrinsic};
pub use types::{Breakpoint, DerivativeRule, LegalDomains, Signature, StateShape};

pub mod analog_ops;
pub mod convergence;
pub mod detectors;
pub mod logic_ops;
pub mod math;
pub mod noise;
pub mod system;
