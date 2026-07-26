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
    scratch_repo_with(name, &[])
}

/// A local git repo with extra committed files (`rel` → content).
fn scratch_repo_with(name: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path().join(name);
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("Piperine.toml"),
        "[project]\nname = \"dep\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
    )
    .unwrap();
    for (rel, content) in files {
        std::fs::write(repo.join(rel), content).unwrap();
    }
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
    piperine_add_env(dir, args, &[])
}

fn piperine_add_env(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_piperine"));
    cmd.arg("add").args(args).current_dir(dir);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn piperine add")
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

// ─── Permissions consent (PLG-23 / D11) ─────────────────────────────────────

/// A plugin repo declaring `[plugin.permissions]`.
fn scratch_permissioned_plugin() -> tempfile::TempDir {
    scratch_repo_with(
        "bjt",
        &[(
            "piperine-plugin.toml",
            "[plugin]\nname = \"bjt\"\n\n[permissions]\nfilesystem = [\"read *.model\"]\nnetwork = true\n",
        )],
    )
}

fn add_plugin(project: &Path, repo: &Path, envs: &[(&str, &str)]) -> Output {
    let url = format!("file://{}", repo.join("bjt").display());
    piperine_add_env(project, &["bjt", "--git", &url, "--branch", "main"], envs)
}

fn combined(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn project_toml(project: &Path) -> String {
    std::fs::read_to_string(project.join("Piperine.toml")).unwrap()
}

#[test]
fn declared_permissions_are_printed_and_a_deny_aborts() {
    // No silent-accept default: the test's stdin is not a tty, so the
    // interactive gate denies — the install aborts with nothing changed.
    let repo = scratch_permissioned_plugin();
    let project = scratch_project();
    let before = project_toml(project.path());
    let out = add_plugin(project.path(), repo.path(), &[]);
    assert!(!out.status.success(), "must abort: {}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("read *.model"), "prints the declared permissions: {text}");
    assert!(text.contains("network"), "prints the declared permissions: {text}");
    assert_eq!(project_toml(project.path()), before, "nothing installed");
    assert!(!project.path().join("Piperine.lock").exists(), "no lock written");
}

#[test]
fn accept_all_mode_grants_and_proceeds() {
    let repo = scratch_permissioned_plugin();
    let project = scratch_project();
    let out = add_plugin(project.path(), repo.path(), &[("PIPERINE_PLUGIN_TRUST", "accept")]);
    assert!(out.status.success(), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("read *.model"), "permissions still printed: {text}");
    assert!(project_toml(project.path()).contains("bjt"), "dep declared");
}

#[test]
fn reject_mode_denies_deterministically() {
    let repo = scratch_permissioned_plugin();
    let project = scratch_project();
    let before = project_toml(project.path());
    let out = add_plugin(project.path(), repo.path(), &[("PIPERINE_PLUGIN_TRUST", "reject")]);
    assert!(!out.status.success(), "must abort");
    assert_eq!(project_toml(project.path()), before, "nothing installed");
}

#[test]
fn a_plugin_declaring_no_permissions_needs_no_consent() {
    let repo = scratch_repo_with("bjt", &[("piperine-plugin.toml", "[plugin]\nname = \"bjt\"\n")]);
    let project = scratch_project();
    let out = add_plugin(project.path(), repo.path(), &[]);
    assert!(out.status.success(), "{}", combined(&out));
    assert!(project_toml(project.path()).contains("bjt"), "dep declared");
}
