//! The test targets of each package of the workspace, and the `#[test]` functions each one holds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::item_scan::{FreeFunctionPermit, ScannedKind, ItemScan};
use crate::source_tree::SourceTree;

/// What a package's `Cargo.toml` declares: its name and its `[[test]]` targets.
struct ManifestText {
    package: String,
    explicit_tests: Vec<(String, String)>,
    /// The names of the `[[test]]` targets declared `test = false`: run by name, not by `cargo test`.
    on_demand: Vec<String>,
}

impl ManifestText {
    /// Reads the few lines of a manifest this crate needs; no TOML parser is involved.
    fn parse(text: &str) -> ManifestText {
        let mut package = String::new();
        let mut explicit_tests = Vec::new();
        let mut section = String::new();
        let mut on_demand = Vec::new();
        let mut pending: (Option<String>, Option<String>) = (None, None);
        let flush = |pending: &mut (Option<String>, Option<String>), tests: &mut Vec<(String, String)>| {
            if let (Some(name), Some(path)) = pending.clone() {
                tests.push((name, path));
            }
            *pending = (None, None);
        };
        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with('[') {
                flush(&mut pending, &mut explicit_tests);
                section = line.to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim().trim_matches('"').to_string();
            match (section.as_str(), key.trim()) {
                ("[package]", "name") => package = value,
                ("[[test]]", "name") => pending.0 = Some(value),
                ("[[test]]", "path") => pending.1 = Some(value),
                ("[[test]]", "test") if value == "false" => on_demand.push(pending.0.clone().unwrap_or_default()),
                _ => {}
            }
        }
        flush(&mut pending, &mut explicit_tests);
        ManifestText { package, explicit_tests, on_demand }
    }
}

/// One integration-test target: its name and the `#[test]` function names in its module tree.
pub struct TestTarget {
    /// The target's name, as `--test <name>` spells it.
    pub name: String,
    /// The names of its `#[test]` functions, without module path.
    pub tests: BTreeSet<String>,
    /// The paths of its `#[test]` functions as cargo prints them, module path from the target's root.
    pub paths: BTreeSet<String>,
}

impl TestTarget {
    /// Whether a test path as a proof writes it names the test at `full`: equal to it, or the end
    /// of it after a `::` (a proof may leave out leading modules, never write a wrong one).
    pub fn path_names(full: &str, written: &str) -> bool {
        full == written || full.strip_suffix(written).is_some_and(|start| start.ends_with("::"))
    }

    /// Whether the target holds a test that the written path names.
    pub fn holds_path(&self, written: &str) -> bool {
        self.paths.iter().any(|full| Self::path_names(full, written))
    }
}

/// One package: its integration-test targets and the test functions of the rest of its sources.
pub struct PackageTests {
    /// The integration-test targets.
    pub targets: Vec<TestTarget>,
    /// The names of the targets run by name, not by `cargo test`.
    pub on_demand: Vec<String>,
    /// Every `#[test]` function of the package's `src/`, the unit tests.
    pub unit_tests: BTreeSet<String>,
    /// The paths of the unit tests as cargo prints them, from the library, main or binary root.
    pub unit_paths: BTreeSet<String>,
}

impl PackageTests {
    /// Every test function of the package, unit and integration.
    pub fn all_tests(&self) -> BTreeSet<&str> {
        let integration = self.targets.iter().flat_map(|target| target.tests.iter());
        self.unit_tests.iter().chain(integration).map(String::as_str).collect()
    }

    /// Whether a unit or integration test of the package is the one the written path names.
    pub fn holds_path(&self, written: &str) -> bool {
        self.unit_paths.iter().any(|full| TestTarget::path_names(full, written)) || self.targets.iter().any(|target| target.holds_path(written))
    }
}

/// The test targets of every package, by package name.
pub struct TestTargets {
    packages: BTreeMap<String, PackageTests>,
}

impl TestTargets {
    /// Reads every `Cargo.toml` of the repository and resolves each package's targets over the tree.
    pub fn of_workspace(tree: &SourceTree, scan: &ItemScan) -> TestTargets {
        let root = SourceTree::workspace_root();
        let mut manifests = vec![(PathBuf::new(), root.join("Cargo.toml"))];
        if let Ok(entries) = std::fs::read_dir(root.join("crates")) {
            let mut directories: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            directories.sort();
            for directory in directories {
                if let Some(name) = directory.file_name() {
                    manifests.push((Path::new("crates").join(name), directory.join("Cargo.toml")));
                }
            }
        }
        let mut found: BTreeMap<PathBuf, BTreeSet<(String, String)>> = BTreeMap::new();
        for item in scan.items().iter().filter(|i| i.kind == ScannedKind::FreeFunction && i.permit == Some(FreeFunctionPermit::TestAttribute)) {
            found.entry(item.file.clone()).or_default().insert((item.scope.clone(), item.name.clone()));
        }
        let mut packages = BTreeMap::new();
        for (directory, manifest_path) in manifests {
            let Ok(text) = std::fs::read_to_string(&manifest_path) else { continue };
            let manifest = ManifestText::parse(&text);
            packages.insert(manifest.package.clone(), Self::package_tests(tree, &found, &directory, &manifest));
        }
        TestTargets { packages }
    }

    /// The targets and tests of one package.
    fn package_tests(tree: &SourceTree, found: &BTreeMap<PathBuf, BTreeSet<(String, String)>>, directory: &Path, manifest: &ManifestText) -> PackageTests {
        let mut roots: Vec<(String, PathBuf)> = manifest.explicit_tests.iter().map(|(name, path)| (name.clone(), directory.join(path))).collect();
        let tests_directory = directory.join("tests");
        for file in tree.files() {
            let Ok(relative) = file.path().strip_prefix(&tests_directory) else { continue };
            let parts: Vec<&str> = relative.iter().filter_map(|p| p.to_str()).collect();
            let name = match parts.as_slice() {
                [single] => single.strip_suffix(".rs").map(str::to_string),
                [folder, "main.rs"] => Some(folder.to_string()),
                _ => None,
            };
            if let Some(name) = name {
                if !roots.iter().any(|(existing, _)| *existing == name) {
                    roots.push((name, file.path().to_path_buf()));
                }
            }
        }
        let targets = roots
            .into_iter()
            .map(|(name, root)| {
                let paths = Self::test_paths(tree, found, &root);
                let tests = paths.iter().map(|path| path.rsplit("::").next().unwrap_or(path).to_string()).collect();
                TestTarget { name, tests, paths }
            })
            .collect();
        let source_directory = directory.join("src");
        let unit_tests = found.iter().filter(|(path, _)| path.starts_with(&source_directory)).flat_map(|(_, names)| names.iter().map(|(_, name)| name.clone())).collect();
        let mut unit_roots = vec![source_directory.join("lib.rs"), source_directory.join("main.rs")];
        unit_roots.extend(tree.files().iter().map(|file| file.path().to_path_buf()).filter(|path| path.parent() == Some(source_directory.join("bin").as_path())));
        let mut unit_paths: BTreeSet<String> = unit_roots.iter().flat_map(|root| Self::test_paths(tree, found, root)).collect();
        // a source file no root reaches (a `#[path]` the reader does not follow) keeps its inline path
        let reached: BTreeSet<PathBuf> = unit_roots.iter().flat_map(|root| tree.reached_from(root)).map(|file| file.path().to_path_buf()).collect();
        for (_, names) in found.iter().filter(|(path, _)| path.starts_with(&source_directory) && !reached.contains(*path)) {
            unit_paths.extend(names.iter().map(|(scope, name)| if scope.is_empty() { name.clone() } else { format!("{scope}::{name}") }));
        }
        PackageTests { targets, on_demand: manifest.on_demand.clone(), unit_tests, unit_paths }
    }

    /// The paths, as cargo prints them, of the `#[test]` functions reached from the crate root `root`:
    /// the file's module path, then the inline modules the function sits in, then its name.
    fn test_paths(tree: &SourceTree, found: &BTreeMap<PathBuf, BTreeSet<(String, String)>>, root: &Path) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();
        for (file, module_path) in tree.modules_reached_from(root) {
            for (scope, name) in found.get(file.path()).into_iter().flatten() {
                let segments = [module_path.as_str(), scope.as_str(), name.as_str()];
                paths.insert(segments.iter().filter(|segment| !segment.is_empty()).copied().collect::<Vec<_>>().join("::"));
            }
        }
        paths
    }

    /// The package of that name, when the workspace has it.
    pub fn package(&self, name: &str) -> Option<&PackageTests> {
        self.packages.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_path_names_a_test_only_with_its_own_modules() {
        let full = "workspace_shape::examples_hold_only_package_sources";
        assert!(TestTarget::path_names(full, "examples_hold_only_package_sources"));
        assert!(TestTarget::path_names(full, full));
        assert!(!TestTarget::path_names(full, "workspace_shape::workspace_shape::examples_hold_only_package_sources"));
        assert!(!TestTarget::path_names(full, "crate_boundaries::examples_hold_only_package_sources"));
        assert!(!TestTarget::path_names(full, "package_sources"));
    }

    #[test]
    fn a_test_path_follows_the_module_declarations_from_the_root() {
        let tree = SourceTree::of_sources(&[
            ("crates/demo/tests/merged/main.rs", "mod shape;\nmod inline { mod deep; }\n#[test]\nfn at_root() {}\n"),
            ("crates/demo/tests/merged/shape.rs", "#[test]\nfn in_shape() {}\nmod tests {\n    #[test]\n    fn nested() {}\n}\n"),
            ("crates/demo/tests/merged/inline/deep.rs", "#[test]\nfn in_deep() {}\n"),
        ]);
        let scan = ItemScan::of(&tree);
        let mut found: BTreeMap<PathBuf, BTreeSet<(String, String)>> = BTreeMap::new();
        for item in scan.items().iter().filter(|i| i.permit == Some(FreeFunctionPermit::TestAttribute)) {
            found.entry(item.file.clone()).or_default().insert((item.scope.clone(), item.name.clone()));
        }
        let paths = TestTargets::test_paths(&tree, &found, Path::new("crates/demo/tests/merged/main.rs"));
        let expected: BTreeSet<String> = ["at_root", "shape::in_shape", "shape::tests::nested", "inline::deep::in_deep"].into_iter().map(str::to_string).collect();
        assert_eq!(paths, expected);
    }
}
