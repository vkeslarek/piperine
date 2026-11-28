//! The M1 parity harness: ngspice is run at test time on `tests/e2e/twins/*.cir` and the MSL twins
//! of those fixtures are compared against it under one tolerance contract. The library serves the
//! integration targets of this crate; `compile_cache` (the one compile cache of a test run), `mixed` (a hand-built design
//! compiled and run on the solver) and `sinks` (the record sinks) serve the targets of the host and transpiler
//! crates too.

pub mod backend;
pub mod bench;
pub mod compile_cache;
pub mod contract;
pub mod corpus;
pub mod deviation;
pub mod golden;
pub mod host;
pub mod mixed;
pub mod ngspice;
pub mod operating_point;
pub mod project;
pub mod scenario;
pub mod scratch;
pub mod sinks;
pub mod waveform;
