//! Scope: `Piperine.lock` (feature `msl-packages`, PKG-13): the lockfile that records the commit
//! of each git package, its fixed format and order, the pin that survives a moved branch, and
//! (with `Options { update }`) the update, the pruning of stale entries, `E0176` for an invalid
//! file, and the offline resolution of a locked checkout. Every repository is a local one in a
//! temporary directory; no test contacts the network.

#[path = "support/mod.rs"]
#[allow(dead_code)]
mod support;

use std::fs;
use std::path::Path;

use piperine_project::{Options, PackageGraph, resolve};
use support::{commit, package, package_files, repo, tag, url, workspace};

fn files(list: &[(String, String)]) -> Vec<(&str, &str)> {
    list.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
}

fn ok(dir: &Path) -> PackageGraph {
    match resolve(dir, Options::default()) {
        Ok(graph) => graph,
        Err(errors) => panic!("resolution failed: {errors:?}"),
    }
}

fn lock_of(app: &Path) -> String {
    fs::read_to_string(app.join("Piperine.lock")).unwrap()
}

/// A repository holding the package `name`, and its first commit.
fn named_repo(dir: &Path, name: &str, lib_text: &str) -> (git2::Repository, git2::Oid) {
    let repo = repo(dir);
    let oid = commit(&repo, &files(&package_files(name, "1.0.0", lib_text)), "first");
    (repo, oid)
}

#[test]
fn the_lockfile_records_each_git_package() {
    let (_tmp, base) = workspace();
    let (util, util_oid) = named_repo(&base.join("util-repo"), "util", "");
    let (cells, cells_oid) = named_repo(&base.join("cells-repo"), "cells", "");
    tag(&cells, "v1", cells_oid);
    let app = package(
        &base.join("app"),
        "app",
        "1.0.0",
        &[&format!("util = {{ git = \"{}\" }}", url(&util)), &format!("cells = {{ git = \"{}\", tag = \"v1\" }}", url(&cells))],
        "",
    );
    ok(&app);
    let expected = format!(
        "version = 1\n\n[[package]]\nname = \"cells\"\nversion = \"1.0.0\"\nsource = \"git+{}?tag=v1\"\ncommit = \"{cells_oid}\"\n\n[[package]]\nname = \"util\"\nversion = \"1.0.0\"\nsource = \"git+{}\"\ncommit = \"{util_oid}\"\n",
        url(&cells),
        url(&util)
    );
    assert_eq!(lock_of(&app), expected);
    assert_eq!(util_oid.to_string().len(), 40);
    // `?branch=` and `?rev=` forms.
    let line_branch = format!("util = {{ git = \"{}\", branch = \"{}\" }}", url(&util), util.head().unwrap().shorthand().unwrap());
    let app = package(&base.join("app2"), "app2", "1.0.0", &[&line_branch], "");
    ok(&app);
    assert!(lock_of(&app).contains(&format!("source = \"git+{}?branch={}\"", url(&util), util.head().unwrap().shorthand().unwrap())));
    let app = package(&base.join("app3"), "app3", "1.0.0", &[&format!("util = {{ git = \"{}\", rev = \"{util_oid}\" }}", url(&util))], "");
    ok(&app);
    assert!(lock_of(&app).contains(&format!("source = \"git+{}?rev={util_oid}\"", url(&util))));
}

#[test]
fn a_locked_commit_is_kept_when_the_branch_moves() {
    let (_tmp, base) = workspace();
    let (util, first) = named_repo(&base.join("util-repo"), "util", "// first\n");
    let app = package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", url(&util))], "");
    ok(&app);
    let second = commit(&util, &[("src/lib.msl", "// second\n")], "second");
    assert_ne!(first, second);
    let graph = ok(&app);
    assert_eq!(graph.packages[1].commit, Some(first.to_string()));
    assert_eq!(fs::read_to_string(&graph.graph.crates()[1].root).unwrap(), "// first\n");
    assert!(lock_of(&app).contains(&first.to_string()) && !lock_of(&app).contains(&second.to_string()));
}

#[test]
fn no_git_package_no_lockfile() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }"], "");
    package(&base.join("util"), "util", "1.0.0", &[], "");
    ok(&app);
    assert!(!app.join("Piperine.lock").exists());
}

#[test]
fn the_lockfile_is_deterministic() {
    let (_tmp, base) = workspace();
    let (util, _) = named_repo(&base.join("util-repo"), "util", "");
    let (basis, _) = named_repo(&base.join("base-repo"), "base", "");
    // `util` is written first; the lockfile still lists `base` first.
    let app = package(
        &base.join("app"),
        "app",
        "1.0.0",
        &[&format!("util = {{ git = \"{}\" }}", url(&util)), &format!("base = {{ git = \"{}\" }}", url(&basis))],
        "",
    );
    ok(&app);
    let first = lock_of(&app);
    ok(&app);
    assert_eq!(lock_of(&app), first);
    let names: Vec<&str> = first.lines().filter(|l| l.starts_with("name = ")).collect();
    assert_eq!(names, ["name = \"base\"", "name = \"util\""]);
    assert!(first.ends_with('\n') && !first.ends_with("\n\n"));
    // The same repository is not listed twice for two dependents.
    package(&base.join("mid"), "mid", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", url(&util))], "");
    let app2 = package(&base.join("app2"), "app2", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", url(&util)), "mid = { path = \"../mid\" }"], "");
    ok(&app2);
    assert_eq!(lock_of(&app2).matches("[[package]]").count(), 1);
}
