//! The host's session surface, split by role: [`Session`] (the one host entry
//! object) with its [`SessionBuilder`], the sweep drivers over a compiled
//! session, the run configuration an analysis reads, and the build plumbing
//! they share.
//!
//! There is exactly one session type. A staged workflow is "configure a
//! [`SessionBuilder`], compile a `Session`" — staging precedes the build, so it
//! lives on the builder; a live workflow is `set`/`schedule_set` on the
//! compiled session (MD-18: restamp, never re-JIT).

mod build;
mod config;
mod entry;
mod sweep;

pub use config::{Scale, SolverConfig};
pub use entry::{Session, SessionBuilder};
pub use sweep::{Grid, Nested, Sweep, SweepPoint};
