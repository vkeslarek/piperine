//! Scope: resolving `path` dependencies into a crate graph (feature `msl-packages`, PKG-11):
//! each path relative to the manifest that names it, the graph that mirrors the manifests, a
//! shared dependency as one crate, and the located package that is not there (`E0172`, `E0175`).

#[path = "support/mod.rs"]
#[allow(dead_code)]
mod support;

use piperine_project::{Options, PackageError, PackageGraph, resolve};
use support::{package, workspace};

fn ok(dir: &std::path::Path) -> PackageGraph {
    match resolve(dir, Options::default()) {
        Ok(graph) => graph,
        Err(errors) => panic!("resolution failed: {errors:?}"),
    }
}

fn only(dir: &std::path::Path) -> PackageError {
    let mut errors = resolve(dir, Options::default()).err().expect("the project was accepted");
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.remove(0)
}

#[test]
fn path_dependencies_are_relative_to_their_manifest() {
    let (_tmp, base) = workspace();
    // From `app`, `../base` would be `ws/base`, which does not exist: only `util`'s own
    // manifest reaches `ws/libs/base`.
    let app = package(&base.join("ws/app"), "app", "1.0.0", &["util = { path = \"../libs/util\" }"], "");
    package(&base.join("ws/libs/util"), "util", "1.0.0", &["base = { path = \"../base\" }"], "");
    package(&base.join("ws/libs/base"), "base", "1.0.0", &[], "");
    let graph = ok(&app);
    let names: Vec<&str> = graph.graph.crates().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["app", "util", "base"]);
    assert_eq!(graph.graph.crates()[2].root, base.join("ws/libs/base/src/lib.msl"));
}

#[test]
fn the_graph_mirrors_the_manifests() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }"], "");
    package(&base.join("util"), "util", "2.3.4", &["base = { path = \"../base\" }"], "");
    package(&base.join("base"), "base", "0.1.0", &[], "");
    let graph = ok(&app);
    let crates = graph.graph.crates();
    assert_eq!(crates.len(), 3);
    for (node, name) in crates.iter().zip(["app", "util", "base"]) {
        assert_eq!(node.name, name);
        assert_eq!(node.root, base.join(name).join("src/lib.msl"));
    }
    let edges = |i: usize| crates[i].dependencies.iter().map(|d| (d.name.clone(), d.krate)).collect::<Vec<_>>();
    assert_eq!(edges(0), [("util".to_string(), graph.packages[1].krate)]);
    assert_eq!(edges(1), [("base".to_string(), graph.packages[2].krate)]);
    assert!(edges(2).is_empty());
    // The first crate is the root package.
    assert_eq!(graph.packages[0].dir, app);
    assert_eq!(graph.packages[0].manifest.package.name, "app");
    assert_eq!(graph.packages[1].manifest.package.version.minor, 3);
    assert_eq!(graph.packages[0].commit, None);
}

#[test]
fn a_shared_dependency_is_one_crate() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }", "base = { path = \"../base\" }"], "");
    // `util` spells the shared directory another way.
    package(&base.join("util"), "util", "1.0.0", &["base = { path = \"../util/../base\" }"], "");
    package(&base.join("base"), "base", "1.0.0", &[], "");
    let graph = ok(&app);
    let crates = graph.graph.crates();
    assert_eq!(crates.len(), 3);
    let base_id = |i: usize| crates[i].dependencies.iter().find(|d| d.name == "base").unwrap().krate;
    assert_eq!(base_id(0), base_id(1));
    assert_eq!(crates[base_id(0).index()].name, "base");
}

#[test]
fn a_path_without_a_package_is_e0172() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../nowhere\" }"], "");
    let error = only(&app);
    assert_eq!(error.code(), "E0172");
    assert_eq!(error.file, app.join("Piperine.toml"));
    assert_eq!(&error.text[error.diagnostic.span().range()], "util");
    // A directory that exists but holds no manifest is the same fault.
    std::fs::create_dir_all(base.join("empty")).unwrap();
    let app = package(&base.join("app2"), "app2", "1.0.0", &["util = { path = \"../empty\" }"], "");
    assert_eq!(only(&app).code(), "E0172");
}

#[test]
fn a_package_without_a_crate_root_is_e0175() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }"], "");
    package(&base.join("util"), "util", "1.0.0", &[], "");
    std::fs::remove_file(base.join("util/src/lib.msl")).unwrap();
    let error = only(&app);
    assert_eq!(error.code(), "E0175");
    assert_eq!(error.file, app.join("Piperine.toml"));
    assert_eq!(&error.text[error.diagnostic.span().range()], "util");
    assert!(error.diagnostic.message.contains(&base.join("util/src/lib.msl").display().to_string()), "{}", error.diagnostic.message);
    // The root package: a zero span in its own manifest.
    std::fs::remove_file(app.join("src/lib.msl")).unwrap();
    let error = only(&app);
    assert_eq!(error.code(), "E0175");
    assert_eq!(error.file, app.join("Piperine.toml"));
    assert!(error.diagnostic.span().is_empty());
    assert!(error.diagnostic.message.contains(&app.join("src/lib.msl").display().to_string()));
}
