//! `CleanCommand`: `piperine clean` (CLI-18). Removes `<project>/target`, and with `--cache`
//! the entries of the cache of compiled designs that the transpiler's settings name: only what
//! the transpiler's layout writes under `<cache>/rust`, and only from a directory that holds no
//! project, working directory or home directory (CLI-18 AC 5). Nothing else.

use std::io::Write;
use std::path::{Path, PathBuf};

use piperine_transpile::{Cache, Settings};

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
        // the cache is judged before anything is removed: a refusal removes nothing
        let cache = if self.cache { Self::owned_cache(dir)? } else { None };
        let mut removed = false;
        let target = dir.join(TARGET);
        if Self::remove(&target)? {
            removed = true;
            writeln!(out.borrow_mut(), "removed {TARGET}").map_err(|e| format!("cannot write to standard output: {e}"))?;
        }
        if let Some(cache) = cache {
            if Self::clear(&cache)? {
                removed = true;
                let _ = writeln!(err, "removed the cache of compiled designs: {}", cache.display());
            }
        }
        if !removed {
            writeln!(out.borrow_mut(), "nothing to remove").map_err(|e| format!("cannot write to standard output: {e}"))?;
        }
        out.borrow_mut().flush().map_err(|e| format!("cannot write to standard output: {e}"))
    }

    /// The cache directory the settings name, canonical; `None` when there is none. Refused when
    /// it is `/`, or is or holds the project `dir`, the working directory or the home directory,
    /// or holds no `rust/` of the transpiler's layout.
    fn owned_cache(dir: &Path) -> Result<Option<PathBuf>, String> {
        let settings = Settings::from_environment().map_err(|e| e.to_string())?;
        let named = settings.cache.display().to_string();
        let cache = match std::fs::canonicalize(&settings.cache) {
            Ok(cache) => cache,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{named}: {e}")),
        };
        let refused = |why: &str| Err(format!("refused to clean the cache {named}: {why}; nothing was removed"));
        if cache.parent().is_none() {
            return refused("it is the root directory");
        }
        let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
        let cwd = std::env::current_dir().ok();
        let guarded = [("the project", Some(dir.to_path_buf())), ("the working directory", cwd), ("the home directory", home)];
        for (what, path) in guarded {
            let Some(path) = path.and_then(|p| std::fs::canonicalize(p).ok()) else { continue };
            if path.starts_with(&cache) {
                return refused(&format!("it is or holds {what}"));
            }
        }
        if !cache.join("rust").is_dir() {
            return refused("it holds no `rust/` directory, so it is no cache of compiled designs");
        }
        Ok(Some(cache))
    }

    /// Removes from `cache` what the transpiler's layout wrote under `rust/`, then `rust/` and
    /// `cache` when that left them empty; whether anything was removed.
    fn clear(cache: &Path) -> Result<bool, String> {
        let rust = cache.join("rust");
        let failed = |path: &Path, e: std::io::Error| format!("{}: {e}", path.display());
        let mut removed = false;
        for entry in std::fs::read_dir(&rust).map_err(|e| failed(&rust, e))? {
            let entry = entry.map_err(|e| failed(&rust, e))?;
            if entry.file_name().to_str().is_some_and(Cache::owns) && Self::remove(&entry.path())? {
                removed = true;
            }
        }
        for dir in [&rust, cache] {
            // a directory that still holds a file of the user's stays; that is no failure
            if std::fs::remove_dir(dir).is_ok() {
                removed = true;
            }
        }
        Ok(removed)
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
