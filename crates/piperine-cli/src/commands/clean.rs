//! `CleanCommand`: `piperine clean` (CLI-18). Removes `<project>/target`, and with `--cache`
//! the directory of compiled designs that the transpiler's settings name. Nothing else.

use std::io::Write;
use std::path::{Path, PathBuf};

use piperine_transpile::Settings;

use crate::cli::Out;
use crate::commands::Locate;
use crate::exit::Exit;
use crate::render::diagnostic::Report;

/// What a project's build writes, under the package directory.
const TARGET: &str = "target";

/// `piperine clean`: the project, and whether the shared cache goes too.
pub struct CleanCommand {
    pub project: Option<PathBuf>,
    pub cache: bool,
}

impl CleanCommand {
    /// Exit 0 with `removed target` (and, for the cache, a line on standard error), or
    /// `nothing to remove` when there was nothing; exit 1 when a directory cannot be removed.
    pub fn execute(&self, out: &Out, err: &mut dyn Write) -> Exit {
        let project = match Locate::package(self.project.as_deref()) {
            Ok(project) => project,
            Err(failure) => {
                // a closed standard error leaves nothing to tell; the command fails either way
                let _ = Report::write(&failure, err);
                return Exit::Failed;
            }
        };
        match self.attempt(project.dir(), out, err) {
            Ok(()) => Exit::Done,
            Err(message) => {
                let _ = writeln!(err, "error: {message}");
                Exit::Failed
            }
        }
    }

    fn attempt(&self, dir: &Path, out: &Out, err: &mut dyn Write) -> Result<(), String> {
        let mut removed = false;
        let target = dir.join(TARGET);
        if Self::remove(&target)? {
            removed = true;
            writeln!(out.borrow_mut(), "removed {TARGET}").map_err(|e| format!("cannot write to standard output: {e}"))?;
        }
        if self.cache {
            let settings = Settings::from_environment().map_err(|e| e.to_string())?;
            if Self::remove(&settings.cache)? {
                removed = true;
                let _ = writeln!(err, "removed the cache of compiled designs: {}", settings.cache.display());
            }
        }
        if !removed {
            writeln!(out.borrow_mut(), "nothing to remove").map_err(|e| format!("cannot write to standard output: {e}"))?;
        }
        out.borrow_mut().flush().map_err(|e| format!("cannot write to standard output: {e}"))
    }

    /// Removes the directory `path` with everything in it; whether there was one.
    fn remove(path: &Path) -> Result<bool, String> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path).map(|()| true).map_err(|e| format!("{}: {e}", path.display())),
            Ok(_) => Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}
