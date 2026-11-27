//! The ratchet: a guard's list of what is wrong today, one file per crate per guard.
//!
//! A guard fails on a violation its allowance does not list **and** on a listed row that no longer
//! exists, so a task that closes a finding deletes the rows it closed and adds none.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::source_tree::SourceTree;

/// What a guard found, per crate: one key per violation (repeated keys count).
#[derive(Default)]
pub struct Findings {
    by_crate: BTreeMap<String, Vec<String>>,
}

impl Findings {
    /// An empty set of findings.
    pub fn new() -> Findings {
        Findings::default()
    }

    /// Records one violation of `crate_name`.
    pub fn add(&mut self, crate_name: &str, key: String) {
        self.by_crate.entry(crate_name.to_string()).or_default().push(key);
    }

    /// How many violations were recorded, over every crate.
    pub fn count(&self) -> usize {
        self.by_crate.values().map(Vec::len).sum()
    }

    /// The crates that have a finding, and the crates the workspace has: every crate is judged,
    /// so a crate with no finding and a stale row still fails.
    fn crates_to_judge(&self) -> Vec<String> {
        let mut crates: Vec<String> = SourceTree::crate_names();
        crates.extend(self.by_crate.keys().cloned());
        crates.sort();
        crates.dedup();
        crates
    }

    /// Judges every crate's findings against `allowances/<crate>/<guard>.tsv`.
    pub fn judge(&self, guard: &str) -> AllowanceVerdict {
        let mut verdict = AllowanceVerdict::default();
        for crate_name in self.crates_to_judge() {
            let found = self.by_crate.get(&crate_name).map(Vec::as_slice).unwrap_or(&[]);
            let crate_verdict = Allowance::read(&crate_name, guard).judge(found);
            verdict.unlisted.extend(crate_verdict.unlisted.into_iter().map(|row| format!("{crate_name}: {row}")));
            verdict.stale.extend(crate_verdict.stale.into_iter().map(|row| format!("{crate_name}: {row}")));
        }
        verdict
    }
}

/// The outcome of judging findings against an allowance.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AllowanceVerdict {
    /// Violations the allowance does not list (or lists with a smaller count), as the row to add.
    pub unlisted: Vec<String>,
    /// Rows the allowance lists that are no longer found (or found fewer times), as the row to delete.
    pub stale: Vec<String>,
}

impl AllowanceVerdict {
    /// Panics with every unlisted violation and every stale row; a clean verdict returns.
    pub fn assert_clean(&self, guard: &str) {
        assert!(
            self.unlisted.is_empty() && self.stale.is_empty(),
            "{guard}: not in allowances/<crate>/{guard}.tsv (fix the code, never add a row):\n{}\nlisted but not found any more (delete or lower the row):\n{}",
            self.unlisted.join("\n"),
            self.stale.join("\n")
        );
    }
}

/// One guard's allowance for one crate: `key<TAB>count` rows.
pub struct Allowance {
    rows: BTreeMap<String, usize>,
}

impl Allowance {
    /// Reads `allowances/<crate_name>/<guard>.tsv` of the guards crate; a missing file is an empty allowance.
    pub fn read(crate_name: &str, guard: &str) -> Allowance {
        let path: PathBuf = SourceTree::guards_directory().join("allowances").join(crate_name).join(format!("{guard}.tsv"));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        Allowance::parse(&path.display().to_string(), &text)
    }

    /// Parses the rows of an allowance file called `origin`; `#` lines and blank lines are comments.
    /// A row that is not `key<TAB>count` is a panic naming the file and the line.
    pub fn parse(origin: &str, text: &str) -> Allowance {
        let mut rows = BTreeMap::new();
        for (index, line) in text.lines().enumerate().filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#')) {
            let parsed = line.rsplit_once('\t').and_then(|(key, count)| Some((key, count.trim().parse::<usize>().ok()?)));
            let Some((key, count)) = parsed else { panic!("{origin}:{}: `{line}` is not `key<TAB>count`", index + 1) };
            rows.insert(key.to_string(), count);
        }
        Allowance { rows }
    }

    /// Compares what was found (one key per violation) with the rows.
    pub fn judge(&self, found: &[String]) -> AllowanceVerdict {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for key in found {
            *counts.entry(key.as_str()).or_default() += 1;
        }
        let mut verdict = AllowanceVerdict::default();
        for (key, &count) in &counts {
            if self.rows.get(*key).copied().unwrap_or(0) < count {
                verdict.unlisted.push(Self::row(key, count));
            }
        }
        for (key, &listed) in &self.rows {
            if counts.get(key.as_str()).copied().unwrap_or(0) < listed {
                verdict.stale.push(Self::row(key, listed));
            }
        }
        verdict
    }

    /// The text of one row, as the file writes it.
    pub fn row(key: &str, count: usize) -> String {
        format!("{key}\t{count}")
    }

    /// The text of a whole allowance file for `found`, sorted: how a file is seeded.
    pub fn render(found: &[String]) -> String {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for key in found {
            *counts.entry(key.as_str()).or_default() += 1;
        }
        counts.iter().map(|(key, count)| format!("{}\n", Self::row(key, *count))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_violation_the_allowance_does_not_list_fails() {
        let allowance = Allowance::parse("test", "a.rs::old\t1\n");
        let verdict = allowance.judge(&["a.rs::old".to_string(), "b.rs::new".to_string()]);
        assert_eq!(verdict.unlisted, vec!["b.rs::new\t1".to_string()]);
        assert!(verdict.stale.is_empty());
    }

    #[test]
    fn a_listed_row_that_no_longer_exists_fails() {
        let allowance = Allowance::parse("test", "a.rs::old\t1\na.rs::closed\t1\n");
        let verdict = allowance.judge(&["a.rs::old".to_string()]);
        assert_eq!(verdict.stale, vec!["a.rs::closed\t1".to_string()]);
        assert!(verdict.unlisted.is_empty());
    }

    #[test]
    fn a_repeated_violation_is_counted_both_ways() {
        let allowance = Allowance::parse("test", "a.rs::dup\t2\n");
        let twice = ["a.rs::dup".to_string(), "a.rs::dup".to_string()];
        assert_eq!(allowance.judge(&twice), AllowanceVerdict::default());
        assert_eq!(allowance.judge(&twice[..1]).stale, vec!["a.rs::dup\t2".to_string()]);
        let three = ["a.rs::dup".to_string(), "a.rs::dup".to_string(), "a.rs::dup".to_string()];
        assert_eq!(allowance.judge(&three).unlisted, vec!["a.rs::dup\t3".to_string()]);
    }

    #[test]
    #[should_panic(expected = "allowance.tsv:2")]
    fn a_malformed_count_fails_loud_naming_the_file_and_line() {
        Allowance::parse("allowance.tsv", "a.rs::one\t1\nb.rs::two\tmany\n");
    }

    #[test]
    fn a_seeded_file_judges_clean() {
        let found = vec!["x::b".to_string(), "x::a".to_string(), "x::a".to_string()];
        let text = Allowance::render(&found);
        assert_eq!(text, "x::a\t2\nx::b\t1\n");
        assert_eq!(Allowance::parse("test", &text).judge(&found), AllowanceVerdict::default());
    }
}
