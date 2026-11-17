//! `CheckCommand`: `piperine check` (CLI-14). Every stage before a backend, for every run;
//! the `ok` line, or every diagnostic of every run and exit 1.

use std::io::Write;
use std::path::PathBuf;

use piperine_api::Project;

use crate::cli::Out;
use crate::commands::Locate;
use crate::exit::Exit;
use crate::render::diagnostic::Report;

/// `piperine check`: the project, or a lone file.
pub struct CheckCommand {
    pub project: Option<PathBuf>,
    pub target: Option<PathBuf>,
}

impl CheckCommand {
    /// Exit 0 with `ok: <package> (<c> crates, <r> runs)` when every stage passes (CLI-14 AC 2,
    /// 4), else exit 1 with every diagnostic on standard error and no `ok` line (AC 3). Runs
    /// that share a source report the same diagnostic once.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        let project = match &self.target {
            Some(file) => Project::file(file),
            None => Locate::package(self.project.as_deref()),
        };
        let checked = match project {
            Ok(project) => project.check(),
            Err(failure) => Err(vec![failure]),
        };
        match checked {
            Ok(checked) => {
                let note = if checked.typed { "" } else { "; type checking needs a run" };
                let line = format!("ok: {} ({} crates, {} runs{note})", checked.package, checked.crates, checked.runs);
                let mut out = out.borrow_mut();
                let wrote = writeln!(out, "{line}").and_then(|()| out.flush());
                match wrote {
                    Ok(()) => Exit::Done,
                    Err(error) => {
                        let _ = writeln!(err, "error: cannot write to standard output: {error}");
                        Exit::Failed
                    }
                }
            }
            Err(failures) => {
                let mut seen: Vec<Vec<u8>> = Vec::new();
                for failure in &failures {
                    let mut text = Vec::new();
                    // a vector takes every write
                    let _ = Report::write(failure, &mut text);
                    if !seen.contains(&text) {
                        // a closed standard error leaves nothing to tell; the command fails either way
                        let _ = err.write_all(&text);
                        seen.push(text);
                    }
                }
                Exit::Failed
            }
        }
    }
}
