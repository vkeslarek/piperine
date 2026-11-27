//! The census of the test suite: how many targets and tests each crate holds, for the before and
//! after report and for the guard that holds each crate to its plan of targets.

use std::collections::BTreeMap;

use crate::item_scan::ItemScan;
use crate::source_tree::SourceTree;
use crate::test_targets::TestTargets;

/// What one crate holds of the suite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateCensus {
    /// The crate directory name, or `piperine` for the root package.
    pub crate_name: String,
    /// The integration-test targets `cargo test` runs.
    pub targets: usize,
    /// The `#[test]` functions of those targets.
    pub integration_tests: usize,
    /// The `#[test]` functions under `src/`.
    pub unit_tests: usize,
    /// The targets holding three tests or fewer.
    pub small_targets: usize,
    /// The lines of the Rust files under `tests/`.
    pub test_lines: usize,
}

/// The census of every crate, in name order.
pub struct SuiteCensus {
    crates: Vec<CrateCensus>,
}

impl SuiteCensus {
    /// Counts the workspace this crate sits in.
    pub fn take() -> SuiteCensus {
        let tree = SourceTree::of_workspace();
        let scan = ItemScan::of(&tree);
        let targets = TestTargets::of_workspace(&tree, &scan);
        let mut lines: BTreeMap<String, usize> = BTreeMap::new();
        for file in tree.files().iter().filter(|file| file.path().components().any(|part| part.as_os_str() == "tests")) {
            *lines.entry(file.crate_name().to_string()).or_default() += file.line_count();
        }
        let mut crates = Vec::new();
        for crate_name in SourceTree::crate_names() {
            let package = if crate_name == "piperine" { "piperine".to_string() } else { crate_name.clone() };
            let Some(tests) = targets.package(&package) else { continue };
            let run: Vec<_> = tests.targets.iter().filter(|target| !tests.on_demand.contains(&target.name)).collect();
            crates.push(CrateCensus {
                targets: run.len(),
                integration_tests: run.iter().map(|target| target.tests.len()).sum(),
                unit_tests: tests.unit_tests.len(),
                small_targets: run.iter().filter(|target| target.tests.len() <= 3).count(),
                test_lines: lines.get(&crate_name).copied().unwrap_or(0),
                crate_name,
            });
        }
        SuiteCensus { crates }
    }

    /// The crates, in name order.
    pub fn crates(&self) -> &[CrateCensus] {
        &self.crates
    }

    /// The census of one crate.
    pub fn of(&self, crate_name: &str) -> Option<&CrateCensus> {
        self.crates.iter().find(|census| census.crate_name == crate_name)
    }

    /// The table of findings.md 3.1 without its two timing columns, as Markdown rows plus the total.
    pub fn render_rows(&self) -> String {
        let mut text = String::new();
        let mut total = CrateCensus { crate_name: "**total**".to_string(), targets: 0, integration_tests: 0, unit_tests: 0, small_targets: 0, test_lines: 0 };
        for census in &self.crates {
            text.push_str(&format!("| `{}` | {} | {} | {} | {} | {} |\n", census.crate_name, census.targets, census.integration_tests, census.unit_tests, census.small_targets, census.test_lines));
            total.targets += census.targets;
            total.integration_tests += census.integration_tests;
            total.unit_tests += census.unit_tests;
            total.small_targets += census.small_targets;
            total.test_lines += census.test_lines;
        }
        text.push_str(&format!("| {} | **{}** | **{}** | **{}** | **{}** | **{}** |\n", total.crate_name, total.targets, total.integration_tests, total.unit_tests, total.small_targets, total.test_lines));
        text
    }
}
