//! The project directories the command tests run in.

use std::path::{Path, PathBuf};

/// An entry of the cache, under the cache directory, as the transpiler lays it out.
pub(crate) const ENTRY: &str = "rust/0123456789abcdef0123456789abcdef";

/// The project directories the command tests run in: the shipped examples, the fixtures under `tests/projects/`, and copies a test may write into.
pub(crate) struct ProjectDirectory;

impl ProjectDirectory {
    /// `examples/rc`.
    pub(crate) fn rc() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc")
    }

    /// A parity twin of `tests/e2e/twins/`.
    pub(crate) fn twin(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins").join(name)
    }

    /// A copy of `examples/rc` in a fresh directory, so a test may write beside its manifest.
    pub(crate) fn rc_copy() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tmp");
        ProjectDirectory::copy(&ProjectDirectory::rc(), dir.path());
        dir
    }

    pub(crate) fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("mkdir");
        for entry in std::fs::read_dir(from).expect("read_dir") {
            let entry = entry.expect("entry");
            let target = to.join(entry.file_name());
            if entry.path().is_dir() { ProjectDirectory::copy(&entry.path(), &target) } else { std::fs::copy(entry.path(), &target).expect("copy"); }
        }
    }

    /// The fixture project `name` under `tests/projects/`.
    pub(crate) fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects").join(name)
    }

    /// Writes `files` (relative path, text) under a new temporary directory and returns it.
    pub(crate) fn written(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tmp");
        for (path, text) in files {
            let at = dir.path().join(path);
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("mkdir");
            std::fs::write(at, text).expect("write");
        }
        dir
    }

    /// Copies the directory `from` into `to` (a project is plain files).
    pub(crate) fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("a directory");
        for entry in std::fs::read_dir(from).expect("a readable directory") {
            let entry = entry.expect("an entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("a type").is_dir() {
                ProjectDirectory::copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("a copy");
            }
        }
    }

    /// A temporary copy of `source`, named `name` inside its own temporary directory.
    pub(crate) fn copy_of(source: &Path, name: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tmp");
        let dir = tmp.path().join(name);
        ProjectDirectory::copy_tree(source, &dir);
        (tmp, dir)
    }

    // ---- test (CLI-08, CLI-16) ------------------------------------------------------------------

    /// The files of a project, by path under `dir`, as (path, text), sorted.
    pub(crate) fn files_under(dir: &Path) -> Vec<(String, String)> {
        let mut found = Vec::new();
        ProjectDirectory::walk(dir, dir, &mut found);
        found.sort();
        found
    }

    // ---- clean (CLI-18) -------------------------------------------------------------------------
    // Every `clean` here works inside a temporary directory: the project is a copy, and the cache
    // is a directory of the test's own named by `PIPERINE_CACHE_DIR`.

    pub(crate) fn walk(root: &Path, at: &Path, found: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(at).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                ProjectDirectory::walk(root, &path, found);
            } else {
                found.push((path.strip_prefix(root).expect("under root").to_string_lossy().into_owned(), std::fs::read_to_string(&path).expect("text")));
            }
        }
    }

    pub(crate) fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// A copy of `rc` in a temporary directory, with a `target/` tree and a separate cache
    /// directory holding a file; returns the temporary directory, the project and the cache.
    pub(crate) fn project_with_target() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let (tmp, dir) = ProjectDirectory::copy_of(&ProjectDirectory::rc(), "rc");
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(dir.join("target/debug")).expect("a target");
        std::fs::write(dir.join("target/debug/out.bin"), "built").expect("a file");
        // the transpiler's layout: an entry `<cache>/rust/<key>/`
        std::fs::create_dir_all(cache.join(ENTRY)).expect("a cache");
        std::fs::write(cache.join(ENTRY).join("design.so"), "compiled").expect("a file");
        (tmp, dir, cache)
    }

    // ---- fmt (CLI-24) ---------------------------------------------------------------------------
    // Every `fmt` here works on a copy of `examples/rc` (committed formatted, CLI-24 AC 6) in a
    // temporary directory; no test formats a file of the repository in place.
}
