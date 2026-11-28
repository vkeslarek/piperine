//! Scope: `Piperine.lock` (feature `msl-packages`, PKG-13): the lockfile that records the commit
//! of each git package, its fixed format and order, the pin that survives a moved branch, and
//! (with `Options { update }`) the update, the pruning of stale entries, `E0176` for an invalid
//! file, and the offline resolution of a locked checkout. Every repository is a local one in a
//! temporary directory; no test contacts the network.

use crate::support;

use std::fs;
use std::path::Path;

use piperine_project::{Options, PackageGraph, Resolver};
use support::ProjectFixture;

/// Reads and builds the lockfile cases.
struct LockProbe;

impl LockProbe {
    fn files(list: &[(String, String)]) -> Vec<(&str, &str)> {
        list.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
    }

    fn ok(dir: &Path) -> PackageGraph {
        match Resolver::resolve(dir, Options::default()) {
            Ok(graph) => graph,
            Err(errors) => panic!("resolution failed: {errors:?}"),
        }
    }

    fn lock_of(app: &Path) -> String {
        fs::read_to_string(app.join("Piperine.lock")).unwrap()
    }

    /// A repository holding the package `name`, and its first commit.
    fn named_repo(dir: &Path, name: &str, lib_text: &str) -> (git2::Repository, git2::Oid) {
        let repo = ProjectFixture::repo(dir);
        let oid = ProjectFixture::commit(&repo, &LockProbe::files(&ProjectFixture::package_files(name, "1.0.0", lib_text)), "first");
        (repo, oid)
    }
}

#[test]
fn the_lockfile_records_each_git_package() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, util_oid) = LockProbe::named_repo(&base.join("util-repo"), "util", "");
    let (cells, cells_oid) = LockProbe::named_repo(&base.join("cells-repo"), "cells", "");
    ProjectFixture::tag(&cells, "v1", cells_oid);
    let app = ProjectFixture::package(
        &base.join("app"),
        "app",
        "1.0.0",
        &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util)), &format!("cells = {{ git = \"{}\", tag = \"v1\" }}", ProjectFixture::url(&cells))],
        "",
    );
    LockProbe::ok(&app);
    let expected = format!(
        "version = 1\n\n[[package]]\nname = \"cells\"\nversion = \"1.0.0\"\nsource = \"git+{}?tag=v1\"\ncommit = \"{cells_oid}\"\n\n[[package]]\nname = \"util\"\nversion = \"1.0.0\"\nsource = \"git+{}\"\ncommit = \"{util_oid}\"\n",
        ProjectFixture::url(&cells),
        ProjectFixture::url(&util)
    );
    assert_eq!(LockProbe::lock_of(&app), expected);
    assert_eq!(util_oid.to_string().len(), 40);
    // `?branch=` and `?rev=` forms.
    let line_branch = format!("util = {{ git = \"{}\", branch = \"{}\" }}", ProjectFixture::url(&util), util.head().unwrap().shorthand().unwrap());
    let app = ProjectFixture::package(&base.join("app2"), "app2", "1.0.0", &[&line_branch], "");
    LockProbe::ok(&app);
    assert!(LockProbe::lock_of(&app).contains(&format!("source = \"git+{}?branch={}\"", ProjectFixture::url(&util), util.head().unwrap().shorthand().unwrap())));
    let app = ProjectFixture::package(&base.join("app3"), "app3", "1.0.0", &[&format!("util = {{ git = \"{}\", rev = \"{util_oid}\" }}", ProjectFixture::url(&util))], "");
    LockProbe::ok(&app);
    assert!(LockProbe::lock_of(&app).contains(&format!("source = \"git+{}?rev={util_oid}\"", ProjectFixture::url(&util))));
}

#[test]
fn a_locked_commit_is_kept_when_the_branch_moves() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, first) = LockProbe::named_repo(&base.join("util-repo"), "util", "// first\n");
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    LockProbe::ok(&app);
    let second = ProjectFixture::commit(&util, &[("src/lib.msl", "// second\n")], "second");
    assert_ne!(first, second);
    let graph = LockProbe::ok(&app);
    assert_eq!(graph.packages[1].commit, Some(first.to_string()));
    assert_eq!(fs::read_to_string(&graph.graph.crates()[1].root).unwrap(), "// first\n");
    assert!(LockProbe::lock_of(&app).contains(&first.to_string()) && !LockProbe::lock_of(&app).contains(&second.to_string()));
}

#[test]
fn no_git_package_no_lockfile() {
    let (_tmp, base) = ProjectFixture::workspace();
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }"], "");
    ProjectFixture::package(&base.join("util"), "util", "1.0.0", &[], "");
    LockProbe::ok(&app);
    assert!(!app.join("Piperine.lock").exists());
}

#[test]
fn the_lockfile_is_deterministic() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, _) = LockProbe::named_repo(&base.join("util-repo"), "util", "");
    let (basis, _) = LockProbe::named_repo(&base.join("base-repo"), "base", "");
    // `util` is written first; the lockfile still lists `base` first.
    let app = ProjectFixture::package(
        &base.join("app"),
        "app",
        "1.0.0",
        &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util)), &format!("base = {{ git = \"{}\" }}", ProjectFixture::url(&basis))],
        "",
    );
    LockProbe::ok(&app);
    let first = LockProbe::lock_of(&app);
    LockProbe::ok(&app);
    assert_eq!(LockProbe::lock_of(&app), first);
    let names: Vec<&str> = first.lines().filter(|l| l.starts_with("name = ")).collect();
    assert_eq!(names, ["name = \"base\"", "name = \"util\""]);
    assert!(first.ends_with('\n') && !first.ends_with("\n\n"));
    // The same repository is not listed twice for two dependents.
    ProjectFixture::package(&base.join("mid"), "mid", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    let app2 = ProjectFixture::package(&base.join("app2"), "app2", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util)), "mid = { path = \"../mid\" }"], "");
    LockProbe::ok(&app2);
    assert_eq!(LockProbe::lock_of(&app2).matches("[[package]]").count(), 1);
}

#[test]
fn update_moves_the_lock_to_the_new_commit() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, first) = LockProbe::named_repo(&base.join("util-repo"), "util", "// first\n");
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    LockProbe::ok(&app);
    let second = ProjectFixture::commit(&util, &[("src/lib.msl", "// second\n")], "second");
    let graph = Resolver::resolve(&app, Options { update: true }).unwrap();
    assert_eq!(graph.packages[1].commit, Some(second.to_string()));
    assert_eq!(fs::read_to_string(&graph.graph.crates()[1].root).unwrap(), "// second\n");
    assert!(LockProbe::lock_of(&app).contains(&second.to_string()) && !LockProbe::lock_of(&app).contains(&first.to_string()));
    // Without the option the new pin holds.
    assert_eq!(LockProbe::ok(&app).packages[1].commit, Some(second.to_string()));
}

#[test]
fn stale_entries_leave_the_lockfile() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, first) = LockProbe::named_repo(&base.join("util-repo"), "util", "// first\n");
    let default = util.head().unwrap().shorthand().unwrap().to_string();
    support::ProjectFixture::branch(&util, "dev", first);
    support::ProjectFixture::switch(&util, "dev");
    let dev = ProjectFixture::commit(&util, &[("src/lib.msl", "// dev\n")], "dev");
    support::ProjectFixture::switch(&util, &default);
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\", branch = \"{default}\" }}", ProjectFixture::url(&util))], "");
    LockProbe::ok(&app);
    assert!(LockProbe::lock_of(&app).contains(&first.to_string()) && LockProbe::lock_of(&app).contains(&format!("?branch={default}")));
    // The `branch` is edited: the entry is replaced, not kept beside the new one.
    ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\", branch = \"dev\" }}", ProjectFixture::url(&util))], "");
    let graph = LockProbe::ok(&app);
    assert_eq!(graph.packages[1].commit, Some(dev.to_string()));
    let text = LockProbe::lock_of(&app);
    assert_eq!(text.matches("[[package]]").count(), 1, "{text}");
    assert!(text.contains(&dev.to_string()) && text.contains("?branch=dev") && !text.contains(&first.to_string()), "{text}");
    // The dependency is removed: the entries go.
    ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[], "");
    LockProbe::ok(&app);
    assert_eq!(LockProbe::lock_of(&app), "version = 1\n");
}

#[test]
fn an_invalid_lockfile_is_e0176() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, _) = LockProbe::named_repo(&base.join("util-repo"), "util", "");
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    for text in ["version = 1\n\n[[package]\nname = \"util", "version = 2\n", "[[package]]\nname = \"util\"\n"] {
        ProjectFixture::write(&app.join("Piperine.lock"), text);
        let mut errors = Resolver::resolve(&app, Options::default()).err().expect("the lockfile was accepted");
        assert_eq!(errors.len(), 1, "{errors:?}");
        let error = errors.remove(0);
        assert_eq!(error.code(), "E0176", "{text}");
        assert_eq!(error.file, app.join("Piperine.lock"));
        assert_eq!(error.text, text);
        assert!(error.diagnostic.span().end <= text.len() + 1);
        assert!(!app.join("target/deps").exists(), "{text}: something was fetched");
    }
    let error = Resolver::resolve(&app, Options { update: true }).err().unwrap().remove(0);
    assert_eq!(error.code(), "E0176");
}

#[test]
fn a_lockfile_commit_must_be_forty_lowercase_hex_digits() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, first) = LockProbe::named_repo(&base.join("util-repo"), "util", "");
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    let source = format!("git+{}", ProjectFixture::url(&util));
    let upper = first.to_string().to_uppercase();
    let short = first.to_string()[..39].to_string();
    for bad in ["zz", "main", "..", "", upper.as_str(), short.as_str()] {
        let text = format!("version = 1\n\n[[package]]\nname = \"util\"\nversion = \"1.0.0\"\nsource = \"{source}\"\ncommit = \"{bad}\"\n");
        ProjectFixture::write(&app.join("Piperine.lock"), &text);
        for options in [Options::default(), Options { update: true }] {
            let mut errors = Resolver::resolve(&app, options).err().expect("the lockfile was accepted");
            assert_eq!(errors.len(), 1, "{bad}: {errors:?}");
            let error = errors.remove(0);
            assert_eq!(error.code(), "E0176", "{bad}");
            assert_eq!(error.file, app.join("Piperine.lock"));
            assert_eq!(&error.text[error.diagnostic.span().range()], format!("\"{bad}\""), "{bad}");
            assert!(!app.join("target/deps").exists(), "{bad}: something was fetched");
        }
    }
    // A good one is accepted.
    let text = format!("version = 1\n\n[[package]]\nname = \"util\"\nversion = \"1.0.0\"\nsource = \"{source}\"\ncommit = \"{first}\"\n");
    ProjectFixture::write(&app.join("Piperine.lock"), &text);
    LockProbe::ok(&app);
}

#[test]
fn a_locked_checkout_needs_no_repository() {
    let (_tmp, base) = ProjectFixture::workspace();
    let (util, first) = LockProbe::named_repo(&base.join("util-repo"), "util", "// kept\n");
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    LockProbe::ok(&app);
    // The repository, and the clone made of it, are gone.
    fs::remove_dir_all(base.join("util-repo")).unwrap();
    for entry in fs::read_dir(app.join("target/deps")).unwrap() {
        fs::remove_dir_all(entry.unwrap().path().join("db")).unwrap();
    }
    let graph = LockProbe::ok(&app);
    assert_eq!(graph.packages[1].commit, Some(first.to_string()));
    assert_eq!(fs::read_to_string(&graph.graph.crates()[1].root).unwrap(), "// kept\n");
}

#[test]
fn only_the_root_lockfile_counts() {
    let (_tmp, base) = ProjectFixture::workspace();
    // `base` has two commits; its head is the second.
    let (base_repo, old) = LockProbe::named_repo(&base.join("base-repo"), "base", "");
    let mut again = ProjectFixture::package_files("base", "1.0.0", "");
    again.push(("marker.txt".to_string(), "newer".to_string()));
    let head = ProjectFixture::commit(&base_repo, &LockProbe::files(&again), "second");
    assert_ne!(old, head);
    // `util`'s own committed lockfile pins `base` at the OLD commit: a resolver that honoured it
    // would check `base` out at `old`.
    let util = ProjectFixture::repo(&base.join("util-repo"));
    let source = format!("git+{}", ProjectFixture::url(&base_repo));
    let mut all = ProjectFixture::package_files("util", "1.0.0", "");
    for (name, text) in all.iter_mut() {
        if name == "Piperine.toml" {
            text.push_str(&format!("\n[dependencies]\nbase = {{ git = \"{}\" }}\n", ProjectFixture::url(&base_repo)));
        }
    }
    all.push(("Piperine.lock".to_string(), format!("version = 1\n\n[[package]]\nname = \"base\"\nversion = \"1.0.0\"\nsource = \"{source}\"\ncommit = \"{old}\"\n")));
    ProjectFixture::commit(&util, &LockProbe::files(&all), "util with a lockfile");
    let app = ProjectFixture::package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", ProjectFixture::url(&util))], "");
    let graph = LockProbe::ok(&app);
    let locked_base = graph.packages.iter().find(|p| p.manifest.package.name == "base").expect("base resolved");
    assert_eq!(locked_base.commit, Some(head.to_string()), "the dependency's lockfile must be ignored");
    assert!(LockProbe::lock_of(&app).contains(&head.to_string()) && !LockProbe::lock_of(&app).contains(&old.to_string()));
}
