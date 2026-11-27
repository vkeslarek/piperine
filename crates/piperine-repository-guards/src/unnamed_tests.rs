//! The unnamed tests: the `#[test]` functions that no proof, task check, inventory row, `GAPS.md`
//! line or Book page names, and the file of verdicts that holds one row for each of them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::item_scan::{FreeFunctionPermit, ItemScan};
use crate::source_tree::SourceTree;

/// The crates whose uncited tests carry a verdict (code-clarity T12 and T13, CC-12 AC 2): every
/// crate with tests except the guards crate itself, whose tests are the guards.
const JUDGED_CRATES: [&str; 15] = [
    "piperine-abi",
    "piperine-analog-solver",
    "piperine-api",
    "piperine-cli",
    "piperine-digital-solver",
    "piperine-interp",
    "piperine-ir",
    "piperine-lang",
    "piperine-numbers",
    "piperine-parity",
    "piperine-plan",
    "piperine-project",
    "piperine-solver",
    "piperine-transpile",
    "piperine",
];

/// The verdict file, relative to the repository root.
const VERDICT_FILE: &str = ".specs/features/code-clarity/uncited_tests.tsv";

/// The uncited tests of the repository and their verdicts.
pub struct UnnamedTests;

impl UnnamedTests {
    /// The words of every file that can cite a test: the proofs, tasks and checks of the feature
    /// folders, the Book's inventory, `GAPS.md` and the Book's pages.
    fn named_words(root: &Path) -> BTreeSet<String> {
        let mut texts = Vec::new();
        for path in SourceTree::files_under(&root.join(".specs/features")) {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
            let is_record = ["proofs", "tasks", "checks"].iter().any(|prefix| name.starts_with(prefix)) && name.ends_with(".md");
            if is_record {
                texts.push(path);
            }
        }
        texts.push(root.join("tests/book/inventory.tsv"));
        texts.push(root.join("GAPS.md"));
        texts.extend(SourceTree::files_under(&root.join("docs/book")));
        let mut words = BTreeSet::new();
        for path in texts {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            words.extend(text.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|word| !word.is_empty()).map(str::to_string));
        }
        words
    }

    /// The verdict key of a test: its crate, its target and its name, joined by tabs. A test under
    /// `tests/` of a crate has that file's (or directory's) name as target, a unit test is
    /// `(unit tests) <path under src>`, and a test of the root package is `tests/<first name>`.
    fn key(file: &Path, crate_name: &str, test: &str) -> Option<String> {
        let parts: Vec<String> = file.components().map(|part| part.as_os_str().to_string_lossy().to_string()).collect();
        let (crate_name, target) = if parts.first().is_some_and(|first| first == "tests") {
            ("piperine".to_string(), format!("tests/{}", parts.get(1)?.trim_end_matches(".rs")))
        } else if parts.get(2).is_some_and(|part| part == "tests") {
            (crate_name.to_string(), parts.get(3)?.trim_end_matches(".rs").to_string())
        } else if parts.get(2).is_some_and(|part| part == "src") {
            (crate_name.to_string(), format!("(unit tests) {}", parts[3..].join("/")))
        } else {
            return None;
        };
        JUDGED_CRATES.contains(&crate_name.as_str()).then(|| format!("{crate_name}\t{target}\t{test}"))
    }

    /// The key of every `#[test]` of the judged crates whose name no record cites.
    pub fn inventory(scan: &ItemScan, root: &Path) -> BTreeSet<String> {
        let named = Self::named_words(root);
        scan.items()
            .iter()
            .filter(|item| item.permit == Some(FreeFunctionPermit::TestAttribute) && !named.contains(&item.name))
            .filter_map(|item| Self::key(&item.file, &item.crate_name, &item.name))
            .collect()
    }

    /// The path of the verdict file.
    pub fn verdict_path(root: &Path) -> PathBuf {
        root.join(VERDICT_FILE)
    }

    /// Whether a verdict cell is a judgement: `kept: ` followed by the rule the test alone proves,
    /// not an empty text, `-` or `TODO`.
    pub fn is_judged(verdict: &str) -> bool {
        let Some(rule) = verdict.strip_prefix("kept: ") else { return false };
        let rule = rule.trim();
        !rule.is_empty() && rule != "-" && !rule.eq_ignore_ascii_case("todo")
    }

    /// The rows of the verdict file that are not `crate<TAB>target<TAB>test<TAB>kept: <rule>`.
    pub fn rows_without_a_verdict(root: &Path) -> Vec<String> {
        let text = std::fs::read_to_string(Self::verdict_path(root)).unwrap_or_default();
        text.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter(|line| {
                let cells: Vec<&str> = line.split('\t').collect();
                cells.len() != 4 || !Self::is_judged(cells[3])
            })
            .map(str::to_string)
            .collect()
    }

    /// The rows of the verdict file as keys, with how many times each stands: `(key, count)`.
    pub fn verdict_rows(root: &Path) -> Vec<(String, usize)> {
        let text = std::fs::read_to_string(Self::verdict_path(root)).unwrap_or_default();
        let mut counts: Vec<(String, usize)> = Vec::new();
        for line in text.lines().filter(|line| !line.is_empty() && !line.starts_with('#')) {
            let cells: Vec<&str> = line.split('\t').collect();
            let key = cells.iter().take(3).copied().collect::<Vec<_>>().join("\t");
            match counts.iter_mut().find(|(known, _)| *known == key) {
                Some((_, count)) => *count += 1,
                None => counts.push((key, 1)),
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_is_a_kept_rule_and_nothing_else() {
        assert!(UnnamedTests::is_judged("kept: asserts the queue keeps its order"));
        for unjudged in ["", "-", "TODO", "kept: ", "kept: -", "kept: TODO", "asserts the queue keeps its order", "kept:no space"] {
            assert!(!UnnamedTests::is_judged(unjudged), "{unjudged:?} passed as a verdict");
        }
    }
}
