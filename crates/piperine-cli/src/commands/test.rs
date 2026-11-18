//! `TestCommand`: `piperine test` (CLI-08, CLI-16). Every run of the project, or the runs
//! named, is executed and judged by its `checks` records: a `PASS` or `FAIL` line each, the
//! failing records under a `FAIL`, then a line per target margin (which never fails a run), no
//! table and no other record, then the count line. A run that called `fatal` is
//! `FAIL <name> (fatal: <msg>)`.

use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use piperine_api::{Cell, Failure, Project, RunSpec, Session, Verdict};
use piperine_transpile::Settings;

use crate::cli::Out;
use crate::commands::run::{RunCommand, Stop};
use crate::commands::{Locate, Select};
use crate::exit::Exit;
use crate::render::diagnostic::Report;
use crate::render::number::Number;
use crate::render::record::RecordLine;

/// `piperine test`: the project and the runs named (all when none).
pub struct TestCommand {
    pub project: Option<PathBuf>,
    pub runs: Vec<String>,
}

impl TestCommand {
    /// Exit 0 when every run passed (and when there is none), 1 when a run failed or did not
    /// run, 2 for a run name that is none.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        let selected = Locate::package(self.project.as_deref()).and_then(|project| Select::runs(&project, &self.runs).map(|specs| (project, specs)));
        let (project, specs) = match selected {
            Ok(found) => found,
            Err(failure) => {
                // a closed standard error leaves nothing to tell; the command fails either way
                let _ = Report::write(&failure, err);
                return if matches!(failure, Failure::NoRun { .. }) { Exit::Usage } else { Exit::Failed };
            }
        };
        let settings = Settings::from_environment().map_err(|e| Failure::Backend(e.to_string()));
        let (mut passed, mut failed) = (0usize, 0usize);
        for spec in &specs {
            let verdict = settings.as_ref().map_err(|e| Failure::Backend(e.to_string())).and_then(|settings| Self::judge(&project, spec, settings, err));
            let text = match verdict {
                Ok(verdict) => {
                    let mut text = Vec::new();
                    if let Some(fatal) = verdict.fatal() {
                        failed += 1;
                        let _ = writeln!(text, "FAIL {} ({})", spec.name, RunCommand::fatal_line(fatal));
                        for record in verdict.failed() {
                            let _ = RecordLine::write(record, &mut text);
                        }
                    } else if verdict.passed() {
                        passed += 1;
                        let _ = writeln!(text, "PASS {} ({} checks)", spec.name, verdict.checks());
                    } else {
                        failed += 1;
                        match verdict.fatal() {
                            Some(msg) => {
                                let _ = writeln!(text, "FAIL {} (fatal: {msg})", spec.name);
                            }
                            None => {
                                let _ = writeln!(text, "FAIL {} ({} of {} checks failed)", spec.name, verdict.failed().len(), verdict.checks());
                            }
                        }
                        for record in verdict.failed() {
                            let _ = RecordLine::write(record, &mut text);
                        }
                    }
                    // HII-16 AC 2: a target only informs, whatever its sign (decision X3)
                    for (name, margin) in verdict.targets() {
                        let _ = writeln!(text, "  target {name} margin={}", Number::text(&Cell::Real(*margin)));
                    }
                    text
                }
                Err(failure) => {
                    failed += 1;
                    let _ = Report::write(&failure, err);
                    format!("FAIL {} (did not run)\n", spec.name).into_bytes()
                }
            };
            if let Err(error) = Self::print(out, &text) {
                let _ = writeln!(err, "error: cannot write to standard output: {error}");
                return Exit::Failed;
            }
        }
        let summary = format!("{} runs, {passed} passed, {failed} failed\n", specs.len());
        if let Err(error) = Self::print(out, summary.as_bytes()) {
            let _ = writeln!(err, "error: cannot write to standard output: {error}");
            return Exit::Failed;
        }
        if failed == 0 { Exit::Done } else { Exit::Failed }
    }

    fn print(out: &Out, text: &[u8]) -> std::io::Result<()> {
        let mut out = out.borrow_mut();
        out.write_all(text)?;
        out.flush()
    }

    /// One run executed and judged. The run's own output keys do not apply: no file is written,
    /// and the stream filter is cleared so that every check reaches the verdict.
    fn judge(project: &Project, spec: &RunSpec, settings: &Settings, err: &mut dyn Write) -> Result<Verdict, Failure> {
        let mut spec = spec.clone();
        spec.csv = None;
        spec.records = None;
        spec.plot = None;
        spec.streams = Vec::new();
        let backend = match RunCommand::backend(None, &spec) {
            Ok(backend) => backend,
            Err(Stop::Failed(failure) | Stop::Usage(failure)) => return Err(failure),
            Err(Stop::Closed(error)) => return Err(Failure::Backend(error.to_string())),
        };
        spec.backend = Some(backend.name().into());
        let mut session = Session::open_announced(project, &spec, settings, &mut |top| RunCommand::announce(err, &spec.name, top))?;
        let verdict = Rc::new(RefCell::new(Verdict::new()));
        let ran = session.run(verdict.clone());
        let verdict = verdict.borrow().clone();
        // a `fatal` ends the run with a verdict of its own; any other failure is a run that did not run
        if verdict.fatal().is_none() {
            ran?;
        }
        Ok(verdict)
    }
}
