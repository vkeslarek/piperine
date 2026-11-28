//! What the number tests of `piperine-numbers` and `piperine-parity` check against, apart from the devices: an exact
//! oracle and a generator of operands. A dev-only crate: no product crate depends on it.
//!
//! - `exact`: `ExactRational`, rational arithmetic the numeric oracles compare against; `oracle`: `ExactOracle` over it.
//! - `random`: `ShiftGenerator`, the deterministic generator of the numeric tests.

#![deny(missing_docs)]

pub mod exact;
pub mod oracle;
pub mod random;
