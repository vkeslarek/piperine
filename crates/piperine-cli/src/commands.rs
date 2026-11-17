//! The commands: one struct per command, each calling `piperine-api` and writing to the two
//! streams it is given.
//!
//! - [`run`]: `piperine run`.
//! - [`check`]: `piperine check`.

pub mod check;
pub mod run;

use std::path::Path;

use piperine_api::{Failure, Project};

/// Where a command finds its project.
pub struct Locate;

impl Locate {
    /// The package at `dir`, else the nearest one upward from the working directory (CLI-02).
    pub fn package(dir: Option<&Path>) -> Result<Project, Failure> {
        match dir {
            Some(dir) => Project::at(dir),
            None => {
                let here = std::env::current_dir().map_err(|error| Failure::Io { path: ".".into(), error })?;
                Project::discover(&here)
            }
        }
    }
}
