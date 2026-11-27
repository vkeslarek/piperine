//! A `tasks*.md` or `checks*.md` file of a feature folder, read the way `check_proofs.py` reads it.

use std::path::{Path, PathBuf};

/// A `**Run <name>**: <command>` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunLine {
    /// The run's name, as a proof spells it after `run`.
    pub name: String,
    /// The command, with the backticks removed.
    pub command: String,
    /// The line number, from 1.
    pub line_number: usize,
}

/// A `**C<n>** - <claim>` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckLine {
    /// The check id, `C12`.
    pub id: String,
    /// The claim's text.
    pub claim: String,
    /// The line number, from 1.
    pub line_number: usize,
}

/// A `Proof: run <name> | <expected line>` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofLine {
    /// The check the proof belongs to.
    pub check_id: String,
    /// The run it names.
    pub run_name: String,
    /// The words after the bar.
    pub expected: String,
    /// The test function the expected line names (`test <name> ... ok`), module path removed.
    pub test_name: Option<String>,
    /// The line number, from 1.
    pub line_number: usize,
}

/// A `Proof: \`<command>\`` line: a command run alone, judged by its exit code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoneProofLine {
    /// The check the proof belongs to.
    pub check_id: String,
    /// The command.
    pub command: String,
    /// The line number, from 1.
    pub line_number: usize,
}

/// One file of a feature folder holding runs, checks and proofs.
pub struct FeatureFile {
    path: PathBuf,
    feature: String,
    text: String,
}

impl FeatureFile {
    /// A file over its text, for the folder `feature`.
    pub fn new(path: PathBuf, feature: &str, text: String) -> FeatureFile {
        FeatureFile { path, feature: feature.to_string(), text }
    }

    /// Every `tasks*.md` and `checks*.md` of `.specs/features/*/` under `root`, in path order.
    pub fn read_all(root: &Path) -> Vec<FeatureFile> {
        let mut files = Vec::new();
        let Ok(features) = std::fs::read_dir(root.join(".specs/features")) else { return files };
        let mut folders: Vec<PathBuf> = features.flatten().map(|entry| entry.path()).collect();
        folders.sort();
        for folder in folders {
            let Ok(entries) = std::fs::read_dir(&folder) else { continue };
            let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            paths.sort();
            for path in paths {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !(name.ends_with(".md") && (name.starts_with("tasks") || name.starts_with("checks"))) {
                    continue;
                }
                if let (Ok(text), Some(feature)) = (std::fs::read_to_string(&path), folder.file_name().and_then(|n| n.to_str())) {
                    files.push(FeatureFile::new(path.clone(), feature, text));
                }
            }
        }
        files
    }

    /// The file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The feature folder's name.
    pub fn feature(&self) -> &str {
        &self.feature
    }

    /// The file's text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The lines outside fenced blocks, with their numbers from 1, left and right trimmed.
    pub fn unfenced_lines(&self) -> Vec<(usize, &str)> {
        let mut inside = false;
        let mut lines = Vec::new();
        for (index, raw) in self.text.lines().enumerate() {
            let line = raw.trim();
            if line.starts_with("```") {
                inside = !inside;
            } else if !inside {
                lines.push((index + 1, line));
            }
        }
        lines
    }

    fn unquoted(text: &str) -> String {
        text.trim().trim_matches('`').trim().to_string()
    }

    /// The label before a colon, `**Run docs**` style: returns the text after the colon when the
    /// line, stripped of leading `*` and spaces, starts with `word` then `separator_free` characters.
    fn after_label(line: &str, word: &str) -> Option<String> {
        let trimmed = line.trim_start_matches(|c: char| c == '*' || c.is_whitespace());
        let head = trimmed.get(..word.len())?;
        if !head.eq_ignore_ascii_case(word) {
            return None;
        }
        Some(trimmed[word.len()..].to_string())
    }

    /// Parses a `Run <name>: <command>` line.
    pub fn parse_run(line: &str) -> Option<(String, String)> {
        let rest = Self::after_label(line, "Run")?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let rest = rest.trim_start();
        let name_end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).unwrap_or(rest.len());
        if name_end == 0 {
            return None;
        }
        let after = rest[name_end..].trim_start_matches(|c: char| c == '*' || c.is_whitespace());
        let command = after.strip_prefix(':')?;
        Some((rest[..name_end].to_string(), Self::unquoted(command)))
    }

    /// Parses a `C<n> - <claim>` line.
    pub fn parse_check(line: &str) -> Option<(String, String)> {
        let trimmed = line.trim_start_matches(|c: char| c == '*' || c.is_whitespace());
        let rest = trimmed.strip_prefix('C')?;
        let digits_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        if digits_end == 0 {
            return None;
        }
        let after = rest[digits_end..].trim_start_matches(|c: char| c == '*' || c.is_whitespace());
        let claim = after.strip_prefix(['-', '\u{2013}', '\u{2014}', ':'])?;
        Some((format!("C{}", &rest[..digits_end]), claim.trim().to_string()))
    }

    /// Parses a `Proof: <proof>` line into its text.
    pub fn parse_proof(line: &str) -> Option<String> {
        let rest = Self::after_label(line, "Proof")?;
        let after = rest.trim_start_matches(|c: char| c == '*' || c.is_whitespace());
        let proof = after.strip_prefix(':')?;
        Some(Self::unquoted(proof))
    }

    /// Every run line.
    pub fn runs(&self) -> Vec<RunLine> {
        self.unfenced_lines().into_iter().filter_map(|(number, line)| Self::parse_run(line).map(|(name, command)| RunLine { name, command, line_number: number })).collect()
    }

    /// Every check line.
    pub fn checks(&self) -> Vec<CheckLine> {
        self.unfenced_lines().into_iter().filter_map(|(number, line)| Self::parse_check(line).map(|(id, claim)| CheckLine { id, claim, line_number: number })).collect()
    }

    /// Every `run <name> | ...` proof, each with the check it follows.
    pub fn proofs(&self) -> Vec<ProofLine> {
        let mut current = String::new();
        let mut proofs = Vec::new();
        for (number, line) in self.unfenced_lines() {
            if let Some((id, _)) = Self::parse_check(line) {
                current = id;
            } else if let Some(proof) = Self::parse_proof(line) {
                if let Some((run_name, expected)) = Self::parse_run_proof(&proof) {
                    let test_name = Self::test_named_by(&expected);
                    proofs.push(ProofLine { check_id: current.clone(), run_name, expected, test_name, line_number: number });
                }
            }
        }
        proofs
    }

    /// Every proof that is a bare command.
    pub fn lone_proofs(&self) -> Vec<LoneProofLine> {
        let mut current = String::new();
        let mut proofs = Vec::new();
        for (number, line) in self.unfenced_lines() {
            if let Some((id, _)) = Self::parse_check(line) {
                current = id;
            } else if let Some(proof) = Self::parse_proof(line) {
                let lowered = proof.to_lowercase();
                if Self::parse_run_proof(&proof).is_none() && !lowered.starts_with("browser") && !lowered.starts_with("walk") && !proof.is_empty() {
                    proofs.push(LoneProofLine { check_id: current.clone(), command: proof, line_number: number });
                }
            }
        }
        proofs
    }

    /// Splits `run <name> | <expected>`.
    pub fn parse_run_proof(proof: &str) -> Option<(String, String)> {
        let head = proof.get(..3)?;
        if !head.eq_ignore_ascii_case("run") {
            return None;
        }
        let rest = proof[3..].strip_prefix(char::is_whitespace)?.trim_start();
        let (name, expected) = rest.split_once('|')?;
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return None;
        }
        Some((name.to_string(), expected.trim().to_string()))
    }

    /// The test function a `test <name> ... ok` expected line names, module path removed; `None`
    /// when the line names none or only a module prefix (`inventory_guard::`).
    pub fn test_named_by(expected: &str) -> Option<String> {
        let words: Vec<&str> = expected.split_whitespace().collect();
        let at = words.iter().position(|w| *w == "test")?;
        let name = words.get(at + 1)?;
        let last = name.rsplit("::").next()?;
        if last.is_empty() || *name == "..." { None } else { Some(last.to_string()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One feature file over a small text.
    struct Scenario;

    impl Scenario {
        fn file(text: &str) -> FeatureFile {
            FeatureFile::new(PathBuf::from("tasks.md"), "demo", text.to_string())
        }
    }

    #[test]
    fn runs_checks_and_proofs_are_read_outside_fences() {
        let file = Scenario::file(
            "**Run docs**: `cargo test -p a --test docs`\n```\n**Run fenced**: `cargo test -p z`\n```\n**C3** - It holds.\nProof: run docs | test it_holds ... ok\nProof: `cargo test -p a --lib`\nProof: browser x.png\n",
        );
        assert_eq!(file.runs(), vec![RunLine { name: "docs".into(), command: "cargo test -p a --test docs".into(), line_number: 1 }]);
        assert_eq!(file.checks(), vec![CheckLine { id: "C3".into(), claim: "It holds.".into(), line_number: 5 }]);
        let proofs = file.proofs();
        assert_eq!((proofs[0].check_id.as_str(), proofs[0].run_name.as_str(), proofs[0].test_name.as_deref()), ("C3", "docs", Some("it_holds")));
        assert_eq!(file.lone_proofs().iter().map(|p| p.command.as_str()).collect::<Vec<_>>(), vec!["cargo test -p a --lib"]);
    }

    #[test]
    fn a_module_path_is_removed_and_a_prefix_names_no_test() {
        assert_eq!(FeatureFile::test_named_by("test merged::inner::it_holds ... ok").as_deref(), Some("it_holds"));
        assert_eq!(FeatureFile::test_named_by("test inventory_guard:: ... ok"), None);
        assert_eq!(FeatureFile::test_named_by("result ok"), None);
    }

    #[test]
    fn a_prose_line_with_run_in_it_is_no_run() {
        assert_eq!(FeatureFile::parse_run("**Run at HEAD (targeted):** `cargo test`"), None);
        assert_eq!(FeatureFile::parse_run("**Run** (PIPERINE_NGSPICE unset):"), None);
    }
}
