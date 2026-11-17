//! `FmtCommand`: `piperine fmt` (CLI-24). Formats every `.msl` file under the project's `src/`
//! and `benches/`, or the files named, with `piperine_lang::msl::format::Formatter`, which keeps
//! every comment and refuses a text it cannot keep them in. A file is rewritten through a
//! temporary file in its own directory and a rename, so an interrupted run leaves the original.
//! `--check` writes nothing.

use std::io::Write;
use std::path::{Path, PathBuf};

use piperine_lang::msl::diag;
use piperine_lang::msl::format::Formatter;

use crate::cli::Out;
use crate::commands::Locate;
use crate::exit::Exit;
use crate::render::diagnostic::Report;

/// The directories of a project whose files `fmt` formats when none is named.
const SOURCES: [&str; 2] = ["src", "benches"];

/// `piperine fmt`: the project, the files named, and whether only to check.
pub struct FmtCommand {
    pub project: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub check: bool,
}

/// What became of one file.
enum Outcome {
    /// Already formatted: nothing printed.
    Kept,
    /// Rewritten, or, with `--check`, would be.
    Differs,
    /// Not formatted: it does not parse, cannot be read or written, or the formatter refused it.
    Failed,
}

impl FmtCommand {
    /// Exit 0 when every file is formatted (printing `formatted <path>` for each one rewritten,
    /// nothing when none was); exit 1 when a file failed, or, with `--check`, when one would
    /// change (printing `would format <path>`).
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        let Some(files) = self.files(err) else { return Exit::Failed };
        let mut failed = false;
        let mut differs = false;
        for (path, shown) in &files {
            match self.one(path, shown, out, err) {
                Outcome::Kept => {}
                Outcome::Differs => differs = true,
                Outcome::Failed => failed = true,
            }
        }
        if failed || (self.check && differs) { Exit::Failed } else { Exit::Done }
    }

    /// The files to format, each with the path a line names it by: the files named, as given;
    /// else every `.msl` under the project's `src/`, then under its `benches/`, each in path
    /// order, relative to the project. `None`, the failure written to `err`, when the project cannot be read.
    fn files(&self, err: &mut dyn Write) -> Option<Vec<(PathBuf, String)>> {
        if !self.files.is_empty() {
            return Some(self.files.iter().map(|f| (f.clone(), f.display().to_string())).collect());
        }
        let project = match Locate::package(self.project.as_deref()) {
            Ok(project) => project,
            Err(failure) => {
                // a closed standard error leaves nothing to tell; the command fails either way
                let _ = Report::write(&failure, err);
                return None;
            }
        };
        let mut files = Vec::new();
        for dir in SOURCES {
            let mut found = Vec::new();
            if let Err(message) = FmtCommand::walk(&project.dir().join(dir), &mut found) {
                let _ = writeln!(err, "error: {message}");
                return None;
            }
            found.sort();
            files.extend(found);
        }
        Some(
            files
                .into_iter()
                .map(|path| {
                    let relative = path.strip_prefix(project.dir()).unwrap_or(&path);
                    let shown = relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                    (path, shown)
                })
                .collect(),
        )
    }

    /// Every `.msl` file under `dir`, recursively; nothing when `dir` does not exist.
    fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| format!("{}: {e}", path.display()))?;
            if kind.is_dir() {
                FmtCommand::walk(&path, files)?;
            } else if path.extension().is_some_and(|e| e == "msl") {
                files.push(path);
            }
        }
        Ok(())
    }

    /// Formats the file at `path`, named `shown` on every line about it.
    fn one(&self, path: &Path, shown: &str, out: &Out, err: &mut dyn Write) -> Outcome {
        let text = match std::fs::read(path).map(String::from_utf8) {
            Ok(Ok(text)) => text,
            Ok(Err(_)) => return FmtCommand::fail(err, &format!("{shown}: not UTF-8 text")),
            Err(e) => return FmtCommand::fail(err, &format!("{shown}: {e}")),
        };
        let formatted = match Formatter::format(&text) {
            Ok(formatted) => formatted,
            Err(diagnostics) => {
                for d in &diagnostics {
                    let _ = writeln!(err, "{}", diag::render(d, shown, &text));
                }
                return FmtCommand::fail(err, &format!("{shown} is left as it is: {} diagnostic(s)", diagnostics.len()));
            }
        };
        if formatted == text {
            return Outcome::Kept;
        }
        let line = if self.check {
            format!("would format {shown}")
        } else {
            if let Err(e) = FmtCommand::replace(path, &formatted) {
                return FmtCommand::fail(err, &format!("{shown}: {e}"));
            }
            format!("formatted {shown}")
        };
        let mut out = out.borrow_mut();
        match writeln!(out, "{line}").and_then(|()| out.flush()) {
            Ok(()) => Outcome::Differs,
            Err(e) => FmtCommand::fail(err, &format!("cannot write to standard output: {e}")),
        }
    }

    /// Writes `text` to a temporary file beside `path`, with `path`'s permissions, and renames it
    /// over `path`: a reader sees the old file or the new one, never a part.
    fn replace(path: &Path, text: &str) -> std::io::Result<()> {
        let name = path.file_name().map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
        let temporary = path.with_file_name(format!(".{name}.fmt-{}.tmp", std::process::id()));
        let written = std::fs::metadata(path).and_then(|meta| {
            std::fs::write(&temporary, text)?;
            std::fs::set_permissions(&temporary, meta.permissions())?;
            std::fs::rename(&temporary, path)
        });
        if written.is_err() {
            // the original is untouched; what is left of the temporary file is removed if it can be
            let _ = std::fs::remove_file(&temporary);
        }
        written
    }

    fn fail(err: &mut dyn Write, message: &str) -> Outcome {
        // a closed standard error leaves nothing to tell; the file fails either way
        let _ = writeln!(err, "error: {message}");
        Outcome::Failed
    }
}
