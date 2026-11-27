//! The test targets of each package of the workspace, and the `#[test]` functions each one holds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::item_scan::{FreeFunctionPermit, ItemKind, ItemScan};
use crate::source_tree::SourceTree;

/// What a package's `Cargo.toml` declares: its name and its `[[test]]` targets.
struct Manifest {
    package: String,
    explicit_tests: Vec<(String, String)>,
}

impl Manifest {
    /// Reads the few lines of a manifest this crate needs; no TOML parser is involved.
    fn parse(text: &str) -> Manifest {
        let mut package = String::new();
        let mut explicit_tests = Vec::new();
        let mut section = String::new();
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
                _ => {}
            }
        }
        flush(&mut pending, &mut explicit_tests);
        Manifest { package, explicit_tests }
    }
}

/// One integration-test target: its name and the `#[test]` function names in its module tree.
pub struct TestTarget {
    /// The target's name, as `--test <name>` spells it.
    pub name: String,
    /// The names of its `#[test]` functions, without module path.
    pub tests: BTreeSet<String>,
}

/// One package: its integration-test targets and the test functions of the rest of its sources.
pub struct PackageTests {
    /// The integration-test targets.
    pub targets: Vec<TestTarget>,
    /// Every `#[test]` function of the package's `src/`, the unit tests.
    pub unit_tests: BTreeSet<String>,
}

impl PackageTests {
    /// Every test function of the package, unit and integration.
    pub fn all_tests(&self) -> BTreeSet<&str> {
        let integration = self.targets.iter().flat_map(|target| target.tests.iter());
        self.unit_tests.iter().chain(integration).map(String::as_str).collect()
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
        let mut found: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
        for item in scan.items().iter().filter(|i| i.kind == ItemKind::FreeFunction && i.permit == Some(FreeFunctionPermit::TestAttribute)) {
            found.entry(item.file.clone()).or_default().insert(item.name.clone());
        }
        let mut packages = BTreeMap::new();
        for (directory, manifest_path) in manifests {
            let Ok(text) = std::fs::read_to_string(&manifest_path) else { continue };
            let manifest = Manifest::parse(&text);
            packages.insert(manifest.package.clone(), Self::package_tests(tree, &found, &directory, &manifest));
        }
        TestTargets { packages }
    }

    fn package_tests(tree: &SourceTree, found: &BTreeMap<PathBuf, BTreeSet<String>>, directory: &Path, manifest: &Manifest) -> PackageTests {
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
                let tests = tree.reached_from(&root).iter().flat_map(|file| found.get(file.path()).into_iter().flatten().cloned()).collect();
                TestTarget { name, tests }
            })
            .collect();
        let source_directory = directory.join("src");
        let unit_tests = found.iter().filter(|(path, _)| path.starts_with(&source_directory)).flat_map(|(_, names)| names.iter().cloned()).collect();
        PackageTests { targets, unit_tests }
    }

    /// The package of that name, when the workspace has it.
    pub fn package(&self, name: &str) -> Option<&PackageTests> {
        self.packages.get(name)
    }
}
