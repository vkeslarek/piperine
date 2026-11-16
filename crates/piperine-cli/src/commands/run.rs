//! `RunCommand`: `piperine run` (CLI-06, CLI-07). Discover the project, take the run, open the
//! session, print the header, every record as it arrives, an empty line and the result table.
//! Standard output is data and deterministic; a first compile is announced on standard error.

use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use piperine_api::{Backend, Built, Failure, Overrides, Project, Record, RecordSink, RunSpec, Session};
use piperine_transpile::Settings;

use crate::cli::Out;
use crate::exit::Exit;
use crate::render::diagnostic::Report;
use crate::render::record::RecordLine;
use crate::render::table::TextTable;

/// The variable that names a backend when the flag does not (CLI-04 AC 7).
const BACKEND_VARIABLE: &str = "PIPERINE_BACKEND";

/// `piperine run`: the project, the run asked for, and the settings the flags gave.
pub struct RunCommand {
    pub project: Option<PathBuf>,
    pub target: Option<String>,
    pub overrides: Overrides,
}

/// Why a run ended before its table: a failure of the request (a usage error when the command
/// line is wrong), or a standard output that cannot be written.
enum Stop {
    Failed(Failure),
    Usage(Failure),
    Closed(io::Error),
}

impl From<Failure> for Stop {
    /// A run that cannot be chosen is the command line's fault: exit 2 (CLI-03 AC 10, 11).
    fn from(failure: Failure) -> Stop {
        match failure {
            Failure::NoRun { .. } => Stop::Usage(failure),
            other => Stop::Failed(other),
        }
    }
}

impl From<io::Error> for Stop {
    fn from(error: io::Error) -> Stop {
        Stop::Closed(error)
    }
}

/// The records of a run, written to standard output the moment the solver delivers them. A
/// write that fails is kept, for the command to report once the solve ends.
struct LiveRecords {
    out: Out,
    closed: Option<io::Error>,
}

impl RecordSink for LiveRecords {
    fn record(&mut self, record: &Record) {
        if self.closed.is_some() {
            return;
        }
        let mut out = self.out.borrow_mut();
        if let Err(error) = RecordLine::write(record, &mut *out).and_then(|()| out.flush()) {
            self.closed = Some(error);
        }
    }
}

impl RunCommand {
    /// Runs the command: exit 0 when the run completed, 1 for a failed design or run, 2 for a
    /// run that cannot be chosen or a backend the flag does not name.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        match self.attempt(out, err) {
            Ok(()) => Exit::Done,
            Err(Stop::Failed(failure)) => {
                // a closed standard error leaves nothing to tell; the command fails either way
                let _ = Report::write(&failure, err);
                Exit::Failed
            }
            Err(Stop::Usage(failure)) => {
                let _ = Report::write(&failure, err);
                Exit::Usage
            }
            Err(Stop::Closed(error)) => {
                let _ = writeln!(err, "error: cannot write to standard output: {error}");
                Exit::Failed
            }
        }
    }

    fn attempt(&self, out: &Out, err: &mut dyn Write) -> Result<(), Stop> {
        let project = self.discover()?;
        let name = if self.lone().is_some() { None } else { self.target.as_deref() };
        let spec = project.request(name, &Overrides { backend: None, ..self.overrides.clone() })?;
        let mut spec = spec;
        spec.backend = Some(self.backend(&spec)?.name().into());
        let settings = Settings::from_environment().map_err(|e| Failure::Backend(e.to_string()))?;
        let mut session = Session::open(&project, &spec, &settings)?;
        if let Built::Compiled { .. } = session.built() {
            // the library compiles inside `open`, so the line is written once it has
            let _ = writeln!(err, "compiled run `{}` (top {}) with rustc, and cached it", spec.name, session.top())?;
        }
        {
            let mut out = out.borrow_mut();
            writeln!(out, "run {}: {}, top {}, backend {}", spec.name, spec.analysis.name(), session.top(), session.backend().name())?;
            out.flush()?;
        }
        let live = Rc::new(RefCell::new(LiveRecords { out: out.clone(), closed: None }));
        let table = session.run(live.clone());
        if let Some(error) = live.borrow_mut().closed.take() {
            return Err(error.into());
        }
        let table = table?;
        let mut out = out.borrow_mut();
        writeln!(out)?;
        TextTable::write(&table, &mut *out)?;
        out.flush()?;
        Ok(())
    }

    /// The project: a lone file when the target ends in `.msl` (its run is made from the flags,
    /// named by the file's stem), else the package at `--project`, else the nearest one upward
    /// from here.
    fn discover(&self) -> Result<Project, Failure> {
        if let Some(file) = self.lone() {
            return Project::file(file);
        }
        match &self.project {
            Some(dir) => Project::at(dir),
            None => {
                let here = std::env::current_dir().map_err(|error| Failure::Io { path: PathBuf::from("."), error })?;
                Project::discover(&here)
            }
        }
    }

    /// The target, when it names a file.
    fn lone(&self) -> Option<&Path> {
        self.target.as_deref().map(Path::new).filter(|p| p.extension().is_some_and(|e| e == "msl"))
    }

    /// CLI-04 AC 7 and 8: the flag, then `PIPERINE_BACKEND`, then the run's `backend` key, then
    /// `rust`. A flag that names no backend is a usage error; the variable or the key, a failure.
    fn backend(&self, spec: &RunSpec) -> Result<Backend, Stop> {
        let flag = self.overrides.backend.as_deref();
        if let Some(flag) = flag {
            Backend::parse(flag).map_err(Stop::Usage)?;
        }
        let variable = std::env::var(BACKEND_VARIABLE).ok().filter(|v| !v.is_empty());
        Ok(Backend::choose(flag, variable.as_deref(), spec.backend.as_deref())?)
    }
}
