//! The host's session surface, split by role: [`Session`] (the compiled entry
//! object) with its [`SessionBuilder`], the sweep drivers over a compiled
//! session, the run configuration an analysis reads, and the build plumbing
//! both share.
//!
//! [`SimSession`] is the pre-`Session` staged shape — one elaborate-and-JIT
//! per analysis. It is being retired in favour of "compile a `Session` per
//! staged workflow" (`SessionBuilder`); every capability it had now lives on
//! `Session` (`.specs/features/p6-cleanup-architecture/session-equivalence.md`).

mod build;
mod config;
mod entry;
mod sim;
mod sweep;

pub use config::{Scale, SolverConfig};
pub use entry::{Session, SessionBuilder};
pub use sim::SimSession;
pub use sweep::{Grid, Nested, Sweep, SweepPoint};
