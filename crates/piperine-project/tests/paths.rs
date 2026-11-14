//! Scope: resolving `path` dependencies into a crate graph (feature `msl-packages`, PKG-11):
//! each path relative to the manifest that names it, the graph that mirrors the manifests, a
//! shared dependency as one crate, and the located package that is not there (`E0172`, `E0175`).

#[path = "support/mod.rs"]
#[allow(dead_code)]
mod support;

use piperine_project::{Manifest, Options, PackageError, PackageGraph, Requirement, Version, resolve};
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

#[test]
fn a_package_of_another_name_is_e0173() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }"], "");
    package(&base.join("util"), "utils", "1.0.0", &[], "");
    let error = only(&app);
    assert_eq!(error.code(), "E0173");
    assert_eq!(error.file, app.join("Piperine.toml"));
    assert_eq!(&error.text[error.diagnostic.span().range()], "util");
    let message = &error.diagnostic.message;
    assert!(message.contains("`util`") && message.contains("`utils`"), "{message}");
}

#[test]
fn package_key_renames_a_dependency() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["u2 = { path = \"../util\", package = \"util\" }"], "");
    package(&base.join("util"), "util", "1.0.0", &[], "");
    let graph = ok(&app);
    let crates = graph.graph.crates();
    assert_eq!(crates[1].name, "util");
    let edge = &crates[0].dependencies[0];
    assert_eq!(edge.name, "u2");
    assert_eq!(edge.krate, graph.packages[1].krate);
    // With `package` given, the key is no longer the name asked for.
    let app = package(&base.join("app3"), "app3", "1.0.0", &["util = { path = \"../util\", package = \"other\" }"], "");
    let error = only(&app);
    assert_eq!(error.code(), "E0173");
    assert!(error.diagnostic.message.contains("`other`") && error.diagnostic.message.contains("`util`"), "{}", error.diagnostic.message);
}

#[test]
fn version_requirements_follow_the_caret_table() {
    let v = |major, minor, patch| Version { major, minor, patch };
    let max = u64::MAX;
    // requirement, lower bound, last admitted, the one below, the first past.
    let rows = [
        ("1.2.3", v(1, 2, 3), v(1, max, max), v(1, 2, 2), v(2, 0, 0)),
        ("1.2", v(1, 2, 0), v(1, max, max), v(1, 1, max), v(2, 0, 0)),
        ("1", v(1, 0, 0), v(1, max, max), v(0, max, max), v(2, 0, 0)),
        ("0.2.3", v(0, 2, 3), v(0, 2, max), v(0, 2, 2), v(0, 3, 0)),
        ("0.2", v(0, 2, 0), v(0, 2, max), v(0, 1, max), v(0, 3, 0)),
        ("0.0.3", v(0, 0, 3), v(0, 0, 3), v(0, 0, 2), v(0, 0, 4)),
        ("0.0", v(0, 0, 0), v(0, 0, max), v(0, 0, 0), v(0, 1, 0)),
        ("0", v(0, 0, 0), v(0, max, max), v(0, 0, 0), v(1, 0, 0)),
    ];
    for (text, lower, last, below, past) in rows {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Piperine.toml"), format!("[package]\nname = \"a\"\nversion = \"1.0.0\"\n[dependencies]\nx = {{ path = \".\", version = \"{text}\" }}\n")).unwrap();
        let manifest = Manifest::load(dir.path()).unwrap();
        let requirement = manifest.dependencies[0].requirement.expect("a requirement");
        assert!(requirement.admits(lower), "{text} must admit its lower bound {lower:?}");
        assert!(requirement.admits(last), "{text} must admit {last:?}");
        // `0.0` and `0` have nothing below their lower bound: `0.0.0` is the lower bound itself.
        if text != "0.0" && text != "0" {
            assert!(!requirement.admits(below), "{text} must reject {below:?}");
        }
        assert!(!requirement.admits(past), "{text} must reject {past:?}");
    }
}

#[test]
fn a_requirement_at_the_integer_limit_does_not_overflow() {
    let max = u64::MAX;
    let v = |major, minor, patch| Version { major, minor, patch };
    // The caret's upper bound would overflow: no upper bound.
    let major = Requirement { major: max, minor: None, patch: None };
    assert!(major.admits(v(max, 0, 0)) && major.admits(v(max, max, max)));
    assert!(!major.admits(v(1, 0, 0)));
    let minor = Requirement { major: 0, minor: Some(max), patch: None };
    assert!(minor.admits(v(0, max, max)) && !minor.admits(v(0, 1, 0)));
    let patch = Requirement { major: 0, minor: Some(0), patch: Some(max) };
    assert!(patch.admits(v(0, 0, max)) && !patch.admits(v(0, 0, 1)));
    // Through the manifest reader, the resolver's path.
    for text in ["18446744073709551615", "0.18446744073709551615", "0.0.18446744073709551615"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Piperine.toml"), format!("[package]\nname = \"a\"\nversion = \"1.0.0\"\n[dependencies]\nx = {{ path = \".\", version = \"{text}\" }}\n")).unwrap();
        let requirement = Manifest::load(dir.path()).unwrap().dependencies[0].requirement.expect("a requirement");
        assert!(requirement.admits(v(max, max, max)), "{text}");
        assert!(!requirement.admits(v(0, 0, 0)), "{text}");
    }
}

#[test]
fn an_unsatisfied_requirement_is_e0173() {
    let (_tmp, base) = workspace();
    let app = package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\", version = \"1.2\" }"], "");
    package(&base.join("util"), "util", "2.0.0", &[], "");
    let error = only(&app);
    assert_eq!(error.code(), "E0173");
    assert_eq!(&error.text[error.diagnostic.span().range()], "util");
    let message = &error.diagnostic.message;
    assert!(message.contains("1.2") && message.contains("2.0.0"), "{message}");
    // A satisfied requirement resolves.
    package(&base.join("util"), "util", "1.9.0", &[], "");
    ok(&app);
}

#[test]
fn a_dependency_cycle_is_e0174_with_its_chain() {
    let (_tmp, base) = workspace();
    let a = package(&base.join("a"), "a", "1.0.0", &["b = { path = \"../b\" }"], "");
    package(&base.join("b"), "b", "1.0.0", &["c = { path = \"../c\" }"], "");
    package(&base.join("c"), "c", "1.0.0", &["a = { path = \"../a\" }"], "");
    let error = only(&a);
    assert_eq!(error.code(), "E0174");
    assert_eq!(error.file, base.join("c/Piperine.toml"));
    assert_eq!(&error.text[error.diagnostic.span().range()], "a");
    assert!(error.diagnostic.message.contains("a -> b -> c -> a"), "{}", error.diagnostic.message);
    // Two packages.
    let x = package(&base.join("x"), "x", "1.0.0", &["y = { path = \"../y\" }"], "");
    package(&base.join("y"), "y", "1.0.0", &["x = { path = \"../x\" }"], "");
    let error = only(&x);
    assert_eq!(error.code(), "E0174");
    assert!(error.diagnostic.message.contains("x -> y -> x"), "{}", error.diagnostic.message);
    // A cycle that does not pass through the root names only the cycle.
    let r = package(&base.join("r"), "r", "1.0.0", &["p = { path = \"../p\" }"], "");
    package(&base.join("p"), "p", "1.0.0", &["q = { path = \"../q\" }"], "");
    package(&base.join("q"), "q", "1.0.0", &["p = { path = \"../p\" }"], "");
    let error = only(&r);
    assert_eq!(error.code(), "E0174");
    assert!(error.diagnostic.message.contains("p -> q -> p") && !error.diagnostic.message.contains("r ->"), "{}", error.diagnostic.message);
}

#[test]
fn a_package_that_depends_on_itself_is_e0174() {
    let (_tmp, base) = workspace();
    let me = package(&base.join("me"), "me", "1.0.0", &["me = { path = \".\" }"], "");
    let error = only(&me);
    assert_eq!(error.code(), "E0174");
    assert_eq!(&error.text[error.diagnostic.span().range()], "me");
    assert!(error.diagnostic.message.contains("me -> me"), "{}", error.diagnostic.message);
}
