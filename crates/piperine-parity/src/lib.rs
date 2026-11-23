//! The M1 parity harness: ngspice is run at test time on `tests/e2e/twins/*.cir` and the MSL twins
//! of those fixtures are compared against it under one tolerance contract. The library serves the
//! integration targets of this crate only.

pub mod backend;
pub mod bench;
pub mod contract;
pub mod corpus;
pub mod deviation;
pub mod golden;
pub mod host;
pub mod ngspice;
pub mod operating_point;
pub mod project;
pub mod scenario;
pub mod scratch;
pub mod waveform;
