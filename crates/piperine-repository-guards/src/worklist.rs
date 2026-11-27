//! A worklist: the inventory of one finding, each row with the verdict a task gave it.
//!
//! A row is `key<TAB>verdict`; the verdict is `split`, `removed`, `kept: <reason>`, or `-` while the
//! row is open. The inventory is held to the file both ways: a new item that is not a row and a row
//! whose item is gone both fail, so closing a row means doing what its verdict says.

use std::collections::{BTreeMap, BTreeSet};

use crate::allowance::AllowanceVerdict;
use crate::source_tree::SourceTree;

/// The rows of `worklists/<name>.tsv` of the guards crate.
pub struct Worklist {
    rows: BTreeMap<String, String>,
}

impl Worklist {
    /// Reads a worklist; a missing file is an empty one.
    pub fn read(name: &str) -> Worklist {
        let path = SourceTree::guards_directory().join("worklists").join(format!("{name}.tsv"));
        Worklist::parse(&std::fs::read_to_string(path).unwrap_or_default())
    }

    /// Parses worklist text; `#` lines and blank lines are comments.
    pub fn parse(text: &str) -> Worklist {
        let mut rows = BTreeMap::new();
        for line in text.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')) {
            let (key, verdict) = line.rsplit_once('\t').unwrap_or((line, "-"));
            rows.insert(key.to_string(), verdict.trim().to_string());
        }
        Worklist { rows }
    }

    /// The rows with no verdict yet.
    pub fn open_rows(&self) -> Vec<&str> {
        self.rows.iter().filter(|(_, verdict)| verdict.as_str() == "-").map(|(key, _)| key.as_str()).collect()
    }

    /// Compares the inventory with the rows: items with no row, and rows with no item.
    pub fn judge(&self, inventory: &BTreeSet<String>) -> AllowanceVerdict {
        AllowanceVerdict {
            unlisted: inventory.iter().filter(|key| !self.rows.contains_key(*key)).map(|key| format!("{key}\t-")).collect(),
            stale: self.rows.iter().filter(|(key, _)| !inventory.contains(*key)).map(|(key, verdict)| format!("{key}\t{verdict}")).collect(),
        }
    }

    /// The text of a worklist file over an inventory, every row open: how a worklist is seeded.
    pub fn render(inventory: &BTreeSet<String>) -> String {
        inventory.iter().map(|key| format!("{key}\t-\n")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds inventories for the tests below.
    struct Scenario;

    impl Scenario {
        fn inventory(keys: &[&str]) -> BTreeSet<String> {
            keys.iter().map(|key| key.to_string()).collect()
        }
    }

    #[test]
    fn an_item_with_no_row_and_a_row_with_no_item_both_fail() {
        let worklist = Worklist::parse("a.rs::one\t-\nb.rs::gone\tremoved\n");
        let verdict = worklist.judge(&Scenario::inventory(&["a.rs::one", "c.rs::new"]));
        assert_eq!(verdict.unlisted, vec!["c.rs::new\t-".to_string()]);
        assert_eq!(verdict.stale, vec!["b.rs::gone\tremoved".to_string()]);
    }

    #[test]
    fn open_rows_are_the_rows_with_no_verdict() {
        let worklist = Worklist::parse("# comment\na.rs::one\t-\nb.rs::two\tkept: an exhaustive match\n");
        assert_eq!(worklist.open_rows(), vec!["a.rs::one"]);
    }

    #[test]
    fn a_seeded_worklist_judges_clean() {
        let inventory = Scenario::inventory(&["x::b", "x::a"]);
        let text = Worklist::render(&inventory);
        assert_eq!(text, "x::a\t-\nx::b\t-\n");
        assert_eq!(Worklist::parse(&text).judge(&inventory), AllowanceVerdict::default());
    }
}
