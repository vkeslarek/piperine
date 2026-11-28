//! Fixture builders shared by the resolver targets (`paths`, `git`, `lock`, `project`): plain
//! functions that write packages and repositories into a temporary directory. Included by
//! `#[path]`; it is not a test target.

use std::fs;
use std::path::{Path, PathBuf};

/// Builds throwaway packages, workspaces and git repositories for the project tests.
pub struct ProjectFixture;

impl ProjectFixture {
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
        ProjectFixture::write(&dir.join("Piperine.toml"), &manifest);
        ProjectFixture::write(&dir.join("src/lib.msl"), lib_text);
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

    /// A new repository at `dir` (created, with its directories). Every repository of the tests is
    /// a local one, named to the resolver by its filesystem path: no test contacts the network.
    pub fn repo(dir: &Path) -> git2::Repository {
        fs::create_dir_all(dir).unwrap();
        git2::Repository::init(dir).unwrap()
    }

    /// Write `files` into the work tree and commit everything on `HEAD`.
    pub fn commit(repo: &git2::Repository, files: &[(&str, &str)], message: &str) -> git2::Oid {
        let workdir = repo.workdir().unwrap().to_path_buf();
        for (path, text) in files {
            ProjectFixture::write(&workdir.join(path), text);
        }
        let mut index = repo.index().unwrap();
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = git2::Signature::now("tester", "tester@example.com").unwrap();
        let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parents).unwrap()
    }

    /// A branch `name` at `oid`.
    pub fn branch(repo: &git2::Repository, name: &str, oid: git2::Oid) {
        repo.branch(name, &repo.find_commit(oid).unwrap(), true).unwrap();
    }

    /// Make `name` the checked-out branch, so that the next `commit` lands on it.
    pub fn switch(repo: &git2::Repository, name: &str) {
        repo.set_head(&format!("refs/heads/{name}")).unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force())).unwrap();
    }

    /// A lightweight tag `name` at `oid`.
    pub fn tag(repo: &git2::Repository, name: &str, oid: git2::Oid) {
        repo.tag_lightweight(name, &repo.find_commit(oid).unwrap().into_object(), true).unwrap();
    }

    /// The path of a repository's work tree, as the manifest writes it.
    pub fn url(repo: &git2::Repository) -> String {
        repo.workdir().unwrap().to_str().unwrap().trim_end_matches('/').to_string()
    }

    /// The files of a package `name` (manifest and a crate root holding `lib_text`), for `commit`.
    pub fn package_files(name: &str, version: &str, lib_text: &str) -> Vec<(String, String)> {
        vec![
            ("Piperine.toml".to_string(), format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\n")),
            ("src/lib.msl".to_string(), lib_text.to_string()),
        ]
    }
}

