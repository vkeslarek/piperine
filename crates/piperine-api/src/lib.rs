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
//! - [`verdict`]: pass or fail of a run, from its `checks` records.
//! - [`request`]: a run request, read from a run table or from flags, validated.

pub mod backend;
pub mod composite;
pub mod failure;
mod held;
mod host;
pub mod prelude;
pub mod metadata;
pub mod params;
pub mod plot;
pub mod probe;
pub mod project;
pub mod record;
mod recorder;
pub mod request;
pub mod session;
mod source;
pub mod table;
pub mod verdict;

pub use backend::Backend;
pub use composite::{CellShape, CompositeCell, VariantShape};
pub use failure::Failure;
pub use held::Held;
pub use metadata::{Metadata, ParamInfo, PortInfo, StreamInfo};
pub use plot::Plot;
pub use probe::{Column, Probe};
pub use project::{Checked, Project};
pub use record::{Field, Record, RecordSink, Streams};
pub use request::{Analysis, Grid, Overrides, RunSpec};
pub use session::{Built, ParamPath, ParamTarget, Session};
pub use source::NodeSource;
pub use params::{Changed, Derive, Params};
pub use piperine_solver::prelude::ParamValue;
pub use table::{Cell, Table};
pub use verdict::Verdict;
