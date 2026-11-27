//! Scope: git dependencies (feature `msl-packages`, PKG-12): the default branch, a branch, a tag
//! and a revision each checked out under `target/deps`, one crate per commit, `E0172` for what
//! git cannot give, and a git package that resolves its own `path` dependencies. Every
//! repository is a local one in a temporary directory; no test contacts the network.

#[path = "support/mod.rs"]
#[allow(dead_code)]
mod support;

use std::fs;

use piperine_project::{Options, PackageError, PackageGraph, Resolver};
use support::{branch, commit, package, package_files, repo, switch, tag, url, workspace};

fn files(list: &[(String, String)]) -> Vec<(&str, &str)> {
    list.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
}

fn ok(dir: &std::path::Path) -> PackageGraph {
    match Resolver::resolve(dir, Options::default()) {
        Ok(graph) => graph,
        Err(errors) => panic!("resolution failed: {errors:?}"),
    }
}

fn only(dir: &std::path::Path) -> PackageError {
    let mut errors = Resolver::resolve(dir, Options::default()).err().expect("the project was accepted");
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.remove(0)
}

/// A repository holding the package `util` whose crate root is `lib_text`, and its first commit.
fn util_repo(dir: &std::path::Path, lib_text: &str) -> (git2::Repository, git2::Oid) {
    let repo = repo(dir);
    let oid = commit(&repo, &files(&package_files("util", "1.0.0", lib_text)), "util");
    (repo, oid)
}

#[test]
fn a_git_dependency_checks_out_the_default_branch() {
    let (_tmp, base) = workspace();
    let (repo, oid) = util_repo(&base.join("util-repo"), "// head\n");
    let app = package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", url(&repo))], "");
    let graph = ok(&app);
    assert_eq!(graph.graph.crates().len(), 2);
    let root = &graph.graph.crates()[1].root;
    assert!(root.starts_with(app.join("target/deps")), "{root:?}");
    assert_eq!(fs::read_to_string(root).unwrap(), "// head\n");
    assert!(root.to_str().unwrap().contains(&oid.to_string()));
    assert_eq!(graph.packages[1].commit, Some(oid.to_string()));
    assert_eq!(graph.packages[1].manifest.package.name, "util");
}

#[test]
fn branch_tag_and_rev_select_their_commit() {
    let (_tmp, base) = workspace();
    let (repo, first) = util_repo(&base.join("util-repo"), "// first\n");
    tag(&repo, "v1", first);
    let default = repo.head().unwrap().shorthand().unwrap().to_string();
    branch(&repo, "dev", first);
    switch(&repo, "dev");
    let second = commit(&repo, &[("src/lib.msl", "// second\n")], "second");
    switch(&repo, &default);
    let third = commit(&repo, &[("src/lib.msl", "// third\n")], "third");
    let cases = [
        ("branch = \"dev\"".to_string(), "// second\n", second),
        ("tag = \"v1\"".to_string(), "// first\n", first),
        (format!("rev = \"{third}\""), "// third\n", third),
        (String::new(), "// third\n", third),
    ];
    for (i, (reference, text, oid)) in cases.iter().enumerate() {
        let line = format!("util = {{ git = \"{}\", {reference} }}", url(&repo)).replace(", }", " }");
        let app = package(&base.join(format!("app{i}")), "app", "1.0.0", &[&line], "");
        let graph = ok(&app);
        assert_eq!(fs::read_to_string(&graph.graph.crates()[1].root).unwrap(), *text, "{reference}");
        assert_eq!(graph.packages[1].commit, Some(oid.to_string()), "{reference}");
    }
}

#[test]
fn an_unreachable_repository_or_ref_is_e0172() {
    let (_tmp, base) = workspace();
    let (repo, _) = util_repo(&base.join("util-repo"), "");
    let missing = base.join("no-repo").display().to_string();
    for (line, what) in [
        (format!("util = {{ git = \"{missing}\" }}"), "a path that is no repository"),
        (format!("util = {{ git = \"{}\", branch = \"nope\" }}", url(&repo)), "a missing branch"),
        (format!("util = {{ git = \"{}\", tag = \"nope\" }}", url(&repo)), "a missing tag"),
        (format!("util = {{ git = \"{}\", rev = \"{}\" }}", url(&repo), "0123456789abcdef0123456789abcdef01234567"), "a missing commit"),
    ] {
        let app = package(&base.join("app"), "app", "1.0.0", &[&line], "");
        let error = only(&app);
        assert_eq!(error.code(), "E0172", "{what}");
        assert_eq!(error.file, app.join("Piperine.toml"));
        assert_eq!(&error.text[error.diagnostic.span().range()], "util", "{what}");
        assert!(error.diagnostic.message.len() > "dependency `util`".len() + 10, "{what}: {}", error.diagnostic.message);
        // Nothing half checked out: at most the repository clone, never a commit directory.
        let deps = app.join("target/deps");
        if deps.exists() {
            for entry in fs::read_dir(&deps).unwrap() {
                let names: Vec<_> = fs::read_dir(entry.unwrap().path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
                assert!(names.iter().all(|n| n == "db"), "{what}: {names:?}");
            }
        }
        fs::remove_dir_all(&app).unwrap();
    }
}

#[test]
fn two_commits_of_one_repository_are_two_crates() {
    let (_tmp, base) = workspace();
    let (repo, first) = util_repo(&base.join("util-repo"), "// one\n");
    tag(&repo, "v1", first);
    let second = commit(&repo, &[("src/lib.msl", "// two\n")], "two");
    tag(&repo, "v2", second);
    let lines = [format!("u1 = {{ git = \"{}\", tag = \"v1\", package = \"util\" }}", url(&repo)), format!("u2 = {{ git = \"{}\", tag = \"v2\", package = \"util\" }}", url(&repo))];
    let app = package(&base.join("app"), "app", "1.0.0", &[&lines[0], &lines[1]], "");
    let graph = ok(&app);
    let crates = graph.graph.crates();
    assert_eq!(crates.len(), 3);
    assert_ne!(crates[1].root, crates[2].root);
    assert_eq!(fs::read_to_string(&crates[1].root).unwrap(), "// one\n");
    assert_eq!(fs::read_to_string(&crates[2].root).unwrap(), "// two\n");
    let edges: Vec<&str> = crates[0].dependencies.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(edges, ["u1", "u2"]);
}

#[test]
fn one_commit_of_one_repository_is_one_crate() {
    let (_tmp, base) = workspace();
    let (repo, first) = util_repo(&base.join("util-repo"), "");
    tag(&repo, "v1", first);
    let line = format!("util = {{ git = \"{}\", tag = \"v1\" }}", url(&repo));
    // `mid` reaches the same repository and tag, and a second spelling of the reference: the
    // revision the tag names.
    let rev_line = format!("util = {{ git = \"{}\", rev = \"{first}\" }}", url(&repo));
    package(&base.join("mid"), "mid", "1.0.0", &[&line], "");
    let app = package(&base.join("app"), "app", "1.0.0", &["mid = { path = \"../mid\" }", &line], "");
    let graph = ok(&app);
    let crates = graph.graph.crates();
    assert_eq!(crates.len(), 2 + 1);
    let util_of = |i: usize| crates[i].dependencies.iter().find(|d| d.name == "util").unwrap().krate;
    assert_eq!(util_of(0), util_of(1));
    // The same commit through a `rev`: still one crate.
    package(&base.join("mid"), "mid", "1.0.0", &[&rev_line], "");
    let graph = ok(&app);
    assert_eq!(graph.graph.crates().len(), 3);
}

#[test]
fn one_repository_spelled_two_ways_is_one_crate() {
    let (_tmp, base) = workspace();
    let (repo, _) = util_repo(&base.join("util-repo.git"), "");
    let plain = url(&repo);
    let spellings = [plain.clone(), format!("{plain}/")];
    package(&base.join("mid"), "mid", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", spellings[1])], "");
    let app = package(&base.join("app"), "app", "1.0.0", &["mid = { path = \"../mid\" }", &format!("util = {{ git = \"{}\" }}", spellings[0])], "");
    let graph = ok(&app);
    // app, mid and one util.
    assert_eq!(graph.graph.crates().len(), 3, "{:?}", graph.graph.crates().iter().map(|c| &c.name).collect::<Vec<_>>());
    // Every spelling of the repository is the same lockfile source: no `/`; a local path keeps its `.git`.
    let lock = fs::read_to_string(app.join("Piperine.lock")).unwrap();
    assert_eq!(lock.matches("[[package]]").count(), 1, "{lock}");
    assert!(lock.contains(&format!("source = \"git+{}\"", plain)), "{lock}");
}

#[test]
fn local_repositories_util_and_util_dot_git_stay_two_crates() {
    let (_tmp, base) = workspace();
    let (plain, _) = util_repo(&base.join("util"), "// plain\n");
    let (dotgit, _) = util_repo(&base.join("util.git"), "// dotgit\n");
    let line = |name: &str, repo: &git2::Repository| format!("{name} = {{ git = \"{}\", package = \"util\" }}", url(repo));
    let app = package(&base.join("app"), "app", "1.0.0", &[&line("u1", &plain), &line("u2", &dotgit)], "");
    let graph = ok(&app);
    assert_eq!(graph.graph.crates().len(), 3, "{:?}", graph.graph.crates().iter().map(|c| &c.name).collect::<Vec<_>>());
    let texts: Vec<String> = graph.graph.crates()[1..].iter().map(|c| fs::read_to_string(&c.root).unwrap()).collect();
    assert!(texts.contains(&"// plain\n".to_string()) && texts.contains(&"// dotgit\n".to_string()), "{texts:?}");
}

#[test]
fn a_trailing_dot_git_is_stripped_only_from_remote_urls() {
    use piperine_project::GitCheckout;
    for remote in ["https://h.org/a/util", "http://h.org/a/util", "ssh://git@h.org/a/util", "git://h.org/a/util", "git@h.org:a/util"] {
        assert_eq!(GitCheckout::normalize_url(&format!("{remote}.git")), remote);
        assert_eq!(GitCheckout::normalize_url(&format!("{remote}.git/")), remote);
    }
    for local in ["/srv/util.git", "../util.git", "file:///srv/util.git", "util.git"] {
        assert_eq!(GitCheckout::normalize_url(local), local);
    }
}

#[test]
fn a_git_package_resolves_its_own_dependencies() {
    let (_tmp, base) = workspace();
    let repo = repo(&base.join("util-repo"));
    let mut all = package_files("util", "1.0.0", "");
    all[0].1.push_str("\n[dependencies]\nbase = { path = \"base\" }\n");
    all.extend(package_files("base", "0.1.0", "// base\n").into_iter().map(|(p, t)| (format!("base/{p}"), t)));
    commit(&repo, &files(&all), "util with base");
    let app = package(&base.join("app"), "app", "1.0.0", &[&format!("util = {{ git = \"{}\" }}", url(&repo))], "");
    let graph = ok(&app);
    let names: Vec<&str> = graph.graph.crates().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["app", "util", "base"]);
    let util_root = &graph.graph.crates()[1].root;
    let base_root = &graph.graph.crates()[2].root;
    assert_eq!(*base_root, util_root.parent().unwrap().parent().unwrap().join("base/src/lib.msl"));
    assert_eq!(fs::read_to_string(base_root).unwrap(), "// base\n");
}
