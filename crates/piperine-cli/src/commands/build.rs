//! `BuildCommand`: `piperine build` (CLI-15). Open a session for every run, which compiles its
//! top on its backend, and say what it did; a run that fails is reported and the others go on.

use std::io::Write;
use std::path::PathBuf;

use piperine_api::{Built, Failure, Project, RunSpec, Session};
use piperine_transpile::Settings;

use crate::cli::Out;
use crate::commands::run::{RunCommand, Stop};
use crate::commands::{Locate, Select};
use crate::exit::Exit;
use crate::render::diagnostic::Report;

/// `piperine build`: the project, the runs named (all when none) and `--verbose`.
pub struct BuildCommand {
    pub project: Option<PathBuf>,
    pub runs: Vec<String>,
    pub verbose: bool,
}

impl BuildCommand {
    /// Exit 0 when every run built, 1 when any failed (its error on standard error, the other
    /// runs still built), 2 for a run name that is none.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        let report = |failure: &Failure, err: &mut dyn Write| {
            // a closed standard error leaves nothing to tell; the command fails either way
            let _ = Report::write(failure, err);
        };
        let selected = Locate::package(self.project.as_deref()).and_then(|project| Select::runs(&project, &self.runs).map(|specs| (project, specs)));
        let (project, specs) = match selected {
            Ok(found) => found,
            Err(failure) => {
                report(&failure, err);
                return if matches!(failure, Failure::NoRun { .. }) { Exit::Usage } else { Exit::Failed };
            }
        };
        let settings = match Settings::from_environment() {
            Ok(settings) => settings,
            Err(error) => {
                report(&Failure::Backend(error.to_string()), err);
                return Exit::Failed;
            }
        };
        let mut exit = Exit::Done;
        for spec in &specs {
            match self.build(&project, spec, &settings) {
                Ok(lines) => {
                    let mut out = out.borrow_mut();
                    if let Err(error) = out.write_all(lines.as_bytes()).and_then(|()| out.flush()) {
                        let _ = writeln!(err, "error: cannot write to standard output: {error}");
                        return Exit::Failed;
                    }
                }
                Err(failure) => {
                    report(&failure, err);
                    exit = Exit::Failed;
                }
            }
        }
        exit
    }

    /// The lines one run prints: what the session did, and with `--verbose` the generated
    /// source after a compiled or cached line.
    fn build(&self, project: &Project, spec: &RunSpec, settings: &Settings) -> Result<String, Failure> {
        let mut spec = spec.clone();
        let backend = match RunCommand::backend(None, &spec) {
            Ok(backend) => backend,
            Err(Stop::Failed(failure) | Stop::Usage(failure)) => return Err(failure),
            Err(Stop::Closed(error)) => return Err(Failure::Backend(error.to_string())),
        };
        spec.backend = Some(backend.name().into());
        let session = Session::open(project, &spec, settings)?;
        Ok(match session.built() {
            Built::Compiled { source } => self.listed(&spec, "compiled", Some(source)),
            Built::Cached { source } => self.listed(&spec, "cached", Some(source)),
            Built::Interpreted => self.listed(&spec, "interp, nothing to compile", None),
        })
    }

    fn listed(&self, spec: &RunSpec, what: &str, source: Option<PathBuf>) -> String {
        let mut text = format!("built {}: {what}\n", spec.name);
        if let (true, Some(source)) = (self.verbose, source) {
            text.push_str(&format!("{}\n", source.display()));
        }
        text
    }
}
