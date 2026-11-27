//! Host scenarios of an example (MD-73): a scenario package under `tests/e2e/<example>/` is run
//! by a Rust host that schedules its stimulus and judges what the run printed.
//!
//! - `column`: [`NamedColumn`], a table column found by its label or a dotted suffix of it.
//! - `trace`: [`Trace`], one real column measured over a window.
//! - `discrete`: [`DiscreteTrace`], one discrete column read over a window.
//! - `stimulus`: [`Stimulus`] and [`Ramp`], the host's schedule for one run.
//! - `planted` (tests only): a one-module package the harness tests plant on disk.
//! - `run`: [`ScenarioRun`] and [`ScenarioBatch`], runs of a package executed once each, at once.

pub mod column;
pub mod discrete;
#[cfg(test)]
mod planted;
pub mod run;
pub mod stimulus;
pub mod trace;

pub use column::NamedColumn;
pub use discrete::{Departure, DiscreteSample, DiscreteTrace};
pub use run::{CheckTally, CompletedRun, ScenarioBatch, ScenarioRun};
pub use stimulus::{Ramp, ActionRefusal, Stimulus};
pub use trace::{Crossing, Sample, Trace};
