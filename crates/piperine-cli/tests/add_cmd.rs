//! `piperine add <source>` (plugin-interface v2, PLG-22 / D9): the
//! positional source is resolved Go-style — a full git URL is used
//! verbatim and the package name derives from its last segment; the
//! dependency is written to `Piperine.toml` and resolved.

use std::path::Path;
use std::process::{Command, Output};

/// A scratch project: just the `Piperine.toml` marker.
fn scratch_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("Piperine.toml"),
        "[project]\nname = \"scratch\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
    )
    .unwrap();
    dir
}

/// A local git repo holding one committed file, clonable via `file://`.
fn scratch_repo(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path().join(name);
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("Piperine.toml"),
        "[project]\nname = \"dep\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
    )
    .unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(&repo).output().expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["add", "."]);
    git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "init"]);
    dir
}

fn piperine_add(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_piperine"))
        .arg("add")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn piperine add")
}

#[test]
fn positional_full_git_url_is_verbatim_and_names_the_package() {
    let repo = scratch_repo("bjt-models");
    let project = scratch_project();
    let url = format!("file://{}", repo.path().join("bjt-models").display());
    let out = piperine_add(project.path(), &[&url, "--branch", "main"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let toml = std::fs::read_to_string(project.path().join("Piperine.toml")).unwrap();
    assert!(
        toml.contains("bjt-models") && toml.contains(&format!("git = \"{url}\"")),
        "verbatim URL + derived name in Piperine.toml: {toml}"
    );
}

#[test]
fn malformed_positional_source_fails_loud() {
    let project = scratch_project();
    let out = piperine_add(project.path(), &["justoneword"]);
    assert!(!out.status.success(), "must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("owner/repo"), "names the expected form: {stderr}");
    let toml = std::fs::read_to_string(project.path().join("Piperine.toml")).unwrap();
    assert!(!toml.contains("dependencies"), "nothing installed: {toml}");
}
