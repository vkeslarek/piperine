//! Fixture builders shared by the resolver targets (`paths`, `git`, `lock`, `project`): plain
//! functions that write packages and repositories into a temporary directory. Included by
//! `#[path]`; it is not a test target.

use std::fs;
use std::path::{Path, PathBuf};

/// Write the package `name` at `dir`: a manifest with the given `[dependencies]` lines (each one
/// `key = { ... }`) and `src/lib.msl` holding `lib_text`. Returns `dir`.
pub fn package(dir: &Path, name: &str, version: &str, deps: &[&str], lib_text: &str) -> PathBuf {
    let mut manifest = format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\n");
    if !deps.is_empty() {
        manifest.push_str("\n[dependencies]\n");
        for line in deps {
            manifest.push_str(line);
            manifest.push('\n');
        }
    }
    write(&dir.join("Piperine.toml"), &manifest);
    write(&dir.join("src/lib.msl"), lib_text);
    dir.to_path_buf()
}

/// Write `text` at `path`, creating its directories.
pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A temporary directory and its canonical path (the resolver names packages by canonical path).
pub fn workspace() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    (tmp, base)
}
