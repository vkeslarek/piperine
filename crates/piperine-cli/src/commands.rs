//! The commands: one struct per command, each calling `piperine-api` and writing to the two
//! streams it is given.
//!
//! - [`run`]: `piperine run`.
//! - [`check`]: `piperine check`.
//! - [`build`]: `piperine build`.

pub mod build;
pub mod check;
pub mod run;

use std::path::Path;

use piperine_api::{Failure, Overrides, Project, RunSpec};

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

/// Which runs of a project a command works on.
pub struct Select;

impl Select {
    /// The runs named, in the order given, or every run in manifest order when none is named.
    /// A name that is no run is `Failure::NoRun` (a usage error); a malformed run table is its
    /// first fault.
    pub fn runs(project: &Project, names: &[String]) -> Result<Vec<RunSpec>, Failure> {
        if names.is_empty() {
            if let Some(first) = project.faults().into_iter().next() {
                return Err(first);
            }
            return Ok(project.runs().to_vec());
        }
        names.iter().map(|name| project.request(Some(name), &Overrides::default())).collect()
    }
}
