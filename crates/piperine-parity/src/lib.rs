//! The M1 parity harness: ngspice is run at test time on `tests/e2e/twins/*.cir` and the MSL twins
//! of those fixtures are compared against it under one tolerance contract. The library serves the
//! integration targets of this crate; `compile_cache` (the one compile cache of a test run), `buck_fixture` (a project read, built and run, and the changes of a net read back), `probe` (the output node of a design read at each analysis), `panic_message` (the text a panic carries), `tree_copy` (a directory tree copied for a run), `table_columns` (the columns of a printed table), `source` (the model-file runners), `mixed` (a hand-built design
//! compiled and run on the solver) and `sinks` (the record sinks) and `compiled` (one body compiled and called through the ABI) serve the targets of the host and transpiler
//! crates too.

pub mod backend;
pub mod bench;
pub mod buck_fixture;
pub mod compile_cache;
pub mod compiled;
pub mod contract;
pub mod tree_copy;
pub mod corpus;
pub mod deviation;
pub mod golden;
pub mod host;
pub mod mixed;
pub mod ngspice;
pub mod panic_message;
pub mod operating_point;
pub mod probe;
pub mod project;
pub mod scenario;
pub mod scratch;
pub mod source;
pub mod sinks;
pub mod table_columns;
pub mod waveform;
