//! `RunCommand`: `piperine run` (CLI-06, CLI-07). Discover the project, take the run, open the
//! session, print the header, every record as it arrives, an empty line and the result table.
//! Standard output is data and deterministic; a first compile is announced on standard error.

use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use piperine_api::{Backend, Cell, Failure, Field, Overrides, Project, Record, RecordSink, RunSpec, Session};
use piperine_transpile::Settings;

use crate::cli::Out;
use crate::commands::Locate;
use crate::exit::Exit;
use crate::render::csv::CsvTable;
use crate::render::diagnostic::Report;
use crate::render::json::JsonRecord;
use crate::render::number::Number;
use crate::render::record::RecordLine;
use crate::render::svg::SvgPlot;
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
pub(crate) enum Stop {
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

/// Where the records of a run go the moment the solver delivers them.
enum Destination {
    /// Standard output, as text lines.
    Terminal { out: Out, all_checks: bool },
    /// A file, as JSON Lines; `path` is the name the user gave it.
    File { file: io::BufWriter<std::fs::File>, path: PathBuf },
}

/// The records of a run, written as the solver delivers them. A write that fails is kept, for
/// the command to report once the solve ends.
struct LiveRecords {
    to: Destination,
    closed: Option<io::Error>,
    /// The first `sim` record whose `failed` is known true: a `fatal` of the model.
    fatal: Option<Record>,
}

impl RecordSink for LiveRecords {
    fn record(&mut self, record: &Record) {
        if self.fatal.is_none() && record.stream == "sim" && record.fields.iter().any(|(name, field)| name == "failed" && *field == Field::Bool(true)) {
            self.fatal = Some(record.clone());
        }
        if self.closed.is_some() {
            return;
        }
        let written = match &mut self.to {
            // CLI-06 AC 7: the terminal hides a passing check unless asked; a file keeps every record
            Destination::Terminal { all_checks: false, .. } if record.is_passing_check() => return,
            Destination::Terminal { out, .. } => {
                let mut out = out.borrow_mut();
                RecordLine::write(record, &mut *out).and_then(|()| out.flush())
            }
            Destination::File { file, .. } => JsonRecord::write(record, file),
        };
        if let Err(error) = written {
            self.closed = Some(error);
        }
    }
}

impl LiveRecords {
    /// Writes out what a file has buffered; the error of a failed write, whenever it happened,
    /// is the command's to report.
    fn finish(&mut self) -> Result<(), Stop> {
        if let Destination::File { file, path } = &mut self.to {
            if let Err(error) = file.flush() {
                self.closed.get_or_insert(error);
            }
            if let Some(error) = self.closed.take() {
                return Err(Stop::Failed(Failure::Io { path: path.clone(), error }));
            }
            return Ok(());
        }
        match self.closed.take() {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}

impl RunCommand {
    /// `fatal at <t:e>: <msg>`: what a run says of the `fatal` record that ended it (FIX-02).
    pub(crate) fn fatal_line(record: &Record) -> String {
        let msg = record.fields.iter().find_map(|(name, field)| match field {
            Field::Text(text) if name == "msg" => Some(text.as_str()),
            _ => None,
        });
        format!("fatal at {}: {}", Number::text(&Cell::Real(record.time)), msg.unwrap_or(""))
    }

    /// Runs the command: exit 0 when the run completed, 1 for a failed design or run or a
    /// `fatal` of the model, 2 for a run that cannot be chosen or a backend the flag does not
    /// name.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        match self.attempt(out, err) {
            Ok(None) => Exit::Done,
            Ok(Some(fatal)) => {
                // a closed standard error leaves nothing to tell; the command fails either way
                let _ = writeln!(err, "{}", Self::fatal_line(&fatal));
                Exit::Failed
            }
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

    fn attempt(&self, out: &Out, err: &mut dyn Write) -> Result<Option<Record>, Stop> {
        if self.lone().is_some() && self.overrides.analysis.is_none() {
            // CLI-03 AC 12: a lone file has no run table, so the flags must say what to run
            let message = "a lone file has no run table: name the analysis with --analysis (op, dc, ac, transient, tf, pz, sens or pss)".to_string();
            return Err(Stop::Usage(Failure::Host { code: "P0010", message }));
        }
        let project = self.discover()?;
        let name = if self.lone().is_some() { None } else { self.target.as_deref() };
        let spec = project.request(name, &Overrides { backend: None, ..self.overrides.clone() })?;
        let mut spec = spec;
        spec.backend = Some(Self::backend(self.overrides.backend.as_deref(), &spec)?.name().into());
        let settings = Settings::from_environment().map_err(|e| Failure::Backend(e.to_string()))?;
        // CLI-07 AC 8: said before `rustc` runs, while the user waits for it
        let mut session = Session::open_announced(&project, &spec, &settings, &mut |top| Self::announce(err, &spec.name, top))?;
        let base = self.base(&project)?;
        let to = match &spec.records {
            Some(path) => {
                let file = std::fs::File::create(base.join(path)).map_err(|error| Failure::Io { path: path.clone(), error })?;
                Destination::File { file: io::BufWriter::new(file), path: path.clone() }
            }
            None => Destination::Terminal { out: out.clone(), all_checks: spec.all_checks },
        };
        {
            let mut out = out.borrow_mut();
            writeln!(out, "run {}: {}, top {}, backend {}", spec.name, spec.analysis.name(), session.top(), session.backend().name())?;
            out.flush()?;
        }
        let live = Rc::new(RefCell::new(LiveRecords { to, closed: None, fatal: None }));
        let table = session.run(live.clone());
        live.borrow_mut().finish()?;
        let table = table?;
        let fatal = live.borrow().fatal.clone();
        let mut out = out.borrow_mut();
        writeln!(out)?;
        match &spec.csv {
            Some(file) => {
                let mut text = Vec::new();
                CsvTable::write(&table, &mut text)?;
                Self::replace(&base, file, &text)?;
                writeln!(out, "wrote {} ({} rows)", file.display(), table.rows.len())?;
            }
            None => TextTable::write(&table, &mut *out)?,
        }
        if let Some(file) = &spec.plot {
            let mut text = Vec::new();
            SvgPlot::write(&session.plot()?, &mut text)?;
            Self::replace(&base, file, &text)?;
            writeln!(out, "wrote {}", file.display())?;
        }
        out.flush()?;
        Ok(fatal)
    }

    /// The line on standard error that says the run's design is being compiled, flushed before
    /// the compiler starts; a closed standard error leaves nothing to tell.
    pub(crate) fn announce(err: &mut dyn Write, run: &str, top: &str) {
        let _ = writeln!(err, "compiling run `{run}` (top {top}) with rustc").and_then(|()| err.flush());
    }

    /// What a relative output path is under (CLI-11 AC 6): the project directory, the working
    /// directory for a lone file.
    fn base(&self, project: &Project) -> Result<PathBuf, Failure> {
        if self.lone().is_some() {
            return std::env::current_dir().map_err(|error| Failure::Io { path: PathBuf::from("."), error });
        }
        Ok(project.dir().to_path_buf())
    }

    /// Writes `text` to `file` (under `base` when relative), replacing whatever was there.
    fn replace(base: &Path, file: &Path, text: &[u8]) -> Result<(), Failure> {
        let target = base.join(file);
        std::fs::write(&target, text).map_err(|error| Failure::Io { path: file.to_path_buf(), error })
    }

    /// The project: a lone file when the target ends in `.msl` (its run is made from the flags,
    /// named by the file's stem), else the package at `--project`, else the nearest one upward
    /// from here.
    fn discover(&self) -> Result<Project, Failure> {
        if let Some(file) = self.lone() {
            return Project::file(file);
        }
        Locate::package(self.project.as_deref())
    }

    /// The target, when it names a file.
    fn lone(&self) -> Option<&Path> {
        self.target.as_deref().map(Path::new).filter(|p| p.extension().is_some_and(|e| e == "msl"))
    }

    /// CLI-04 AC 7 and 8: the flag, then `PIPERINE_BACKEND`, then the run's `backend` key, then
    /// `rust`. A flag that names no backend is a usage error; the variable or the key, a failure.
    pub(crate) fn backend(flag: Option<&str>, spec: &RunSpec) -> Result<Backend, Stop> {
        if let Some(flag) = flag {
            Backend::parse(flag).map_err(Stop::Usage)?;
        }
        let variable = std::env::var(BACKEND_VARIABLE).ok().filter(|v| !v.is_empty());
        Ok(Backend::choose(flag, variable.as_deref(), spec.backend.as_deref())?)
    }
}
