//! The proof ledger: every proof the feature folders name, checked against the tests that exist.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::cargo_command::CargoTestCommand;
use crate::feature_file::{FeatureFile, ProofLine, RunLine};
use crate::item_scan::ItemScan;
use crate::source_tree::SourceTree;
use crate::test_targets::TestTargets;

/// A proof, a `Run` line or an inventory row that names something that does not exist.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DanglingReference {
    /// The feature folder's name, or `tests/book/inventory.tsv`.
    pub owner: String,
    /// The check id (`C12`), the run (`Run docs`), or the inventory row's id.
    pub subject: String,
    /// What does not exist.
    pub reason: String,
}

impl DanglingReference {
    /// The row of `allowances/proof_ledger.tsv` that lists it.
    pub fn row(&self) -> String {
        format!("{}\t{}\t{}", self.owner, self.subject, self.reason)
    }
}

/// Everything the ledger reads: the feature files, the inventory, the targets and tests that exist.
pub struct ProofLedger {
    root: PathBuf,
    features: Vec<FeatureFile>,
    targets: TestTargets,
    inventory_rows: Vec<(String, String)>,
}

impl ProofLedger {
    /// Reads the repository this crate sits in.
    pub fn of_workspace() -> ProofLedger {
        let tree = SourceTree::of_workspace();
        let scan = ItemScan::of(&tree);
        ProofLedger::over(SourceTree::workspace_root(), &tree, &scan)
    }

    /// Reads the feature files and the inventory under `root`, over a tree and its scan.
    pub fn over(root: PathBuf, tree: &SourceTree, scan: &ItemScan) -> ProofLedger {
        ProofLedger { features: FeatureFile::read_all(&root), inventory_rows: Self::read_inventory(&root), targets: TestTargets::of_workspace(tree, scan), root }
    }

    /// The `(id, covered_by)` cells of the Book's inventory rows.
    fn read_inventory(root: &Path) -> Vec<(String, String)> {
        let Ok(text) = std::fs::read_to_string(root.join("tests/book/inventory.tsv")) else { return Vec::new() };
        text.lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .filter_map(|line| {
                let cells: Vec<&str> = line.split('\t').collect();
                Some((cells.get(1)?.to_string(), cells.get(4)?.to_string()))
            })
            .collect()
    }

    /// The repository root the ledger read.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The feature files read, in path order.
    pub fn features(&self) -> &[FeatureFile] {
        &self.features
    }

    /// Why a `cargo test` command names nothing that exists, or `None` when it does (or is no
    /// package-specific test run).
    pub fn command_problem(&self, command: &str) -> Option<String> {
        let parsed = CargoTestCommand::parse(command)?;
        let package = parsed.package()?;
        let Some(tests) = self.targets.package(package) else { return Some(format!("no package {package}")) };
        parsed.targets().into_iter().find(|target| !tests.targets.iter().any(|t| t.name == *target)).map(|target| format!("no test target {package}/{target}"))
    }

    /// Whether a `#[test]` function of that name exists where a command runs: its target, or the
    /// whole package when the command names no target.
    pub fn test_exists(&self, command: &str, test: &str) -> bool {
        let Some(parsed) = CargoTestCommand::parse(command) else { return true };
        let Some(package) = parsed.package() else { return true };
        let Some(tests) = self.targets.package(package) else { return false };
        let targets = parsed.targets();
        if targets.is_empty() {
            return tests.all_tests().contains(test);
        }
        tests.targets.iter().any(|t| targets.contains(&t.name.as_str()) && t.tests.contains(test))
    }

    /// Whether a function of that name is a test of the package.
    pub fn package_has_test(&self, package: &str, test: &str) -> bool {
        self.targets.package(package).is_some_and(|tests| tests.all_tests().contains(test))
    }

    /// Whether the package has an integration-test target of that name.
    pub fn package_has_target(&self, package: &str, target: &str) -> bool {
        self.targets.package(package).is_some_and(|tests| tests.targets.iter().any(|t| t.name == target))
    }

    /// Every dangling reference of the feature files: a `Run` line naming no package or target,
    /// and a proof naming a test that is not there.
    pub fn dangling_proofs(&self) -> Vec<DanglingReference> {
        let mut found = BTreeSet::new();
        for file in &self.features {
            let runs = file.runs();
            for run in &runs {
                if let Some(reason) = self.command_problem(&run.command) {
                    found.insert(DanglingReference { owner: file.feature().to_string(), subject: format!("Run {}", run.name), reason });
                }
            }
            for proof in file.proofs() {
                if let Some(reference) = self.dangling_proof(file, &runs, &proof) {
                    found.insert(reference);
                }
            }
        }
        found.into_iter().collect()
    }

    /// The reference a proof leaves dangling, when its run names no test that exists.
    fn dangling_proof(&self, file: &FeatureFile, runs: &[RunLine], proof: &ProofLine) -> Option<DanglingReference> {
        let owner = file.feature().to_string();
        let command = runs.iter().find(|run| run.name == proof.run_name).map(|run| run.command.as_str())?;
        let test = proof.test_name.as_deref()?;
        if self.test_exists(command, test) {
            return None;
        }
        Some(DanglingReference { owner, subject: proof.check_id.clone(), reason: format!("run {} names no test {test}", proof.run_name) })
    }

    /// Every inventory row whose `covered_by` names a test that is not there.
    pub fn dangling_inventory_rows(&self) -> Vec<DanglingReference> {
        let mut found = BTreeSet::new();
        for (id, covered_by) in &self.inventory_rows {
            let Some((target_path, test)) = covered_by.split_once("::") else { continue };
            let Some((package, target)) = target_path.split_once('/') else { continue };
            let test_name = test.rsplit("::").next().unwrap_or(test);
            let reason = if !self.package_has_target(package, target) {
                format!("no test target {package}/{target}")
            } else if !self.targets.package(package).is_some_and(|p| p.targets.iter().any(|t| t.name == target && t.tests.contains(test_name))) {
                format!("{package}/{target} has no test {test_name}")
            } else {
                continue;
            };
            found.insert(DanglingReference { owner: "tests/book/inventory.tsv".to_string(), subject: id.clone(), reason });
        }
        found.into_iter().collect()
    }

    /// The parity oracles that exist now, as `<package>::<test>`: every test of `piperine-parity`,
    /// the root `scenarios` and `buck_scenarios` targets and the command line's `twins` target.
    pub fn parity_oracles(&self) -> BTreeSet<String> {
        let mut oracles = BTreeSet::new();
        let selections: [(&str, Option<&str>); 4] = [("piperine-parity", None), ("piperine", Some("scenarios")), ("piperine", Some("buck_scenarios")), ("piperine-cli", Some("twins"))];
        for (package, target) in selections {
            let Some(tests) = self.targets.package(package) else { continue };
            match target {
                None => oracles.extend(tests.all_tests().into_iter().map(|test| format!("{package}::{test}"))),
                Some(name) => {
                    for found in tests.targets.iter().filter(|t| t.name == name) {
                        oracles.extend(found.tests.iter().map(|test| format!("{package}::{test}")));
                    }
                }
            }
        }
        oracles
    }

    /// Whether a `<package>::<test>` oracle row still names a test of its package.
    pub fn oracle_exists(&self, row: &str) -> bool {
        row.split_once("::").is_some_and(|(package, test)| self.package_has_test(package, test))
    }
}
