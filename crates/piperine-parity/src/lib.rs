//! The M1 parity harness: ngspice is run at test time on `tests/ngspice/*.cir` and the MSL twins
//! of those fixtures are compared against it under one tolerance contract. The library serves the
//! integration targets of this crate only.
pub mod contract;
