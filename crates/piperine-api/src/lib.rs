//! # piperine-api
//!
//! The host library (MD-20): it turns a project and a run request into a result. It prints
//! nothing and never exits; the command line (`piperine-cli`) is a shell over it, and the root
//! package `piperine` re-exports it.
//!
//! - [`failure`]: every way a request fails, typed, with what a host needs to report it.
//! - [`backend`]: which backend runs a design, and the order of the places that may name it.
//! - [`probe`]: what a table column shows.
//! - [`project`]: the package a command works on, its runs and the crate graph of a run.
//! - [`plot`]: the geometry of a plot, as numbers.
//! - [`session`]: a run request from source to a circuit bound on a backend, and its run.
//! - [`table`]: the result of an analysis as columns of typed cells.
//! - [`record`]: an `emit!` record, its decoding, and where a session hands it.
//! - [`request`]: a run request, read from a run table or from flags, validated.

pub mod backend;
pub mod failure;
pub mod prelude;
pub mod plot;
pub mod probe;
pub mod project;
pub mod record;
pub mod request;
pub mod session;
pub mod table;

pub use backend::Backend;
pub use failure::Failure;
pub use plot::Plot;
pub use probe::Probe;
pub use project::Project;
pub use record::{Field, Record, RecordSink, Streams};
pub use request::{Analysis, Grid, Overrides, RunSpec};
pub use session::{Built, Session};
pub use table::{Cell, Table};
