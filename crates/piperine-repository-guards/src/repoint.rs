//! The repoint map: the changes of a task to tests and crates, and the rewrite that keeps every
//! proof of every feature pointing at a test that exists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cargo_command::CargoTestCommand;
use crate::feature_file::FeatureFile;

/// The proof a retired check carries instead of its removed test.
const RETIRED_PROOF: &str = "Proof: run ledger | test every_retired_check_is_recorded ... ok";

/// The `Run` line a retired check's proof names.
const LEDGER_RUN: &str = "**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`";

/// A test as the maps spell it: `<package>/<target>::<test>`, the test with its module path when it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestReference {
    /// The package.
    pub package: String,
    /// The integration-test target.
    pub target: String,
    /// The test function, with the module path the target's merge gave it, when there is one.
    pub test: String,
}

impl TestReference {
    /// Parses `<package>/<target>::<test>`.
    fn parse(text: &str) -> Result<TestReference, String> {
        let (target_path, test) = text.split_once("::").ok_or_else(|| format!("`{text}` is not <package>/<target>::<test>"))?;
        let (package, target) = target_path.split_once('/').ok_or_else(|| format!("`{text}` is not <package>/<target>::<test>"))?;
        Ok(TestReference { package: package.to_string(), target: target.to_string(), test: test.to_string() })
    }

    /// The test function's own name, module path removed.
    fn function(&self) -> &str {
        self.test.rsplit("::").next().unwrap_or(&self.test)
    }

    /// The reference as the maps spell it.
    fn spelled(&self) -> String {
        format!("{}/{}::{}", self.package, self.target, self.test)
    }
}

/// One row of a repoint map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepointRow {
    /// A target became a module of a merged target.
    Move {
        /// The package and target that left.
        from_package: String,
        /// The target that left.
        from_target: String,
        /// The package of the merged target.
        to_package: String,
        /// The merged target.
        to_target: String,
        /// The module the old target became.
        module: String,
    },
    /// A crate was renamed.
    RenameCrate {
        /// The old name.
        old: String,
        /// The new name.
        new: String,
    },
    /// A duplicate was removed; the survivor proves the same rule.
    Replace {
        /// The removed test.
        from: TestReference,
        /// The test that survives.
        to: TestReference,
    },
    /// A prose pin was removed and no test replaces it.
    Retire {
        /// The removed test.
        test: TestReference,
        /// Why no test replaces it.
        reason: String,
    },
}

/// The rows of one task's map, and the task they belong to.
pub struct RepointMap {
    task: String,
    rows: Vec<RepointRow>,
}

impl RepointMap {
    /// Parses a map file's text; `task` is the id the retired rows cite (`T14`).
    pub fn parse(task: &str, text: &str) -> Result<RepointMap, String> {
        let mut rows = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            rows.push(Self::parse_row(line).map_err(|message| format!("line {}: {message}", index + 1))?);
        }
        Ok(RepointMap { task: task.to_string(), rows })
    }

    /// One row of a map.
    fn parse_row(line: &str) -> Result<RepointRow, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["move", from, to] => {
                let (from_package, from_target) = from.split_once('/').ok_or("move wants <package>/<target>")?;
                let (to_path, module) = to.split_once("::").ok_or("move wants <package>/<target>::<module>")?;
                let (to_package, to_target) = to_path.split_once('/').ok_or("move wants <package>/<target>::<module>")?;
                Ok(RepointRow::Move { from_package: from_package.into(), from_target: from_target.into(), to_package: to_package.into(), to_target: to_target.into(), module: module.into() })
            }
            ["rename-crate", old, new] => Ok(RepointRow::RenameCrate { old: old.to_string(), new: new.to_string() }),
            ["replace", from, to] => Ok(RepointRow::Replace { from: TestReference::parse(from)?, to: TestReference::parse(to)? }),
            ["retire", test, reason @ ..] if !reason.is_empty() => Ok(RepointRow::Retire { test: TestReference::parse(test)?, reason: reason.join(" ") }),
            _ => Err(format!("`{line}` is no move, rename-crate, replace or retire row")),
        }
    }

    /// Reads `.specs/features/code-clarity/repoints/T<n>.tsv`-style files; the file stem is the task id.
    pub fn read(path: &Path) -> Result<RepointMap, String> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let task = path.file_stem().and_then(|s| s.to_str()).unwrap_or("T0");
        Self::parse(task, &text)
    }

    /// The task id.
    pub fn task(&self) -> &str {
        &self.task
    }

    /// The rows, in order.
    pub fn rows(&self) -> &[RepointRow] {
        &self.rows
    }
}

/// A row of `retired.tsv`: a check whose test was removed with no replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetiredRow {
    /// The feature folder.
    pub feature: String,
    /// The check id.
    pub check: String,
    /// The removed test, `<package>/<target>::<test>`.
    pub test: String,
    /// Why no test replaces it.
    pub reason: String,
}

impl RetiredRow {
    /// The row as the file writes it.
    pub fn line(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.feature, self.check, self.test, self.reason)
    }
}

/// Applies a map to the feature files, the Book's inventory and the two prose files that name tests.
pub struct Repointer {
    root: PathBuf,
}

impl Repointer {
    /// A repointer over the repository at `root`.
    pub fn new(root: PathBuf) -> Repointer {
        Repointer { root }
    }

    /// Rewrites every file the map touches and appends the retired rows.
    pub fn apply(&self, map: &RepointMap) -> Result<(), String> {
        let mut retired = Vec::new();
        for file in FeatureFile::read_all(&self.root) {
            let (text, rows) = FeatureRewriter::rewrite(&file, map)?;
            retired.extend(rows);
            if text != file.text() {
                std::fs::write(file.path(), text).map_err(|error| error.to_string())?;
            }
        }
        self.rewrite_file("tests/book/inventory.tsv", |text| InventoryRewriter::rewrite(text, map))?;
        for prose in ["GAPS.md", "examples/buck/README.md"] {
            self.rewrite_file(prose, |text| Ok(ProseRewriter::rewrite(text, map)))?;
        }
        self.append_retired(&retired)
    }

    /// Rewrites a file of the repository in place when `rewrite` changes it; a missing file is left alone.
    fn rewrite_file(&self, relative: &str, rewrite: impl Fn(&str) -> Result<String, String>) -> Result<(), String> {
        let path = self.root.join(relative);
        let Ok(text) = std::fs::read_to_string(&path) else { return Ok(()) };
        let rewritten = rewrite(&text)?;
        if rewritten != text {
            std::fs::write(&path, rewritten).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Appends the retired rows to `retired.tsv`.
    fn append_retired(&self, rows: &[RetiredRow]) -> Result<(), String> {
        if rows.is_empty() {
            return Ok(());
        }
        let path = self.root.join(".specs/features/code-clarity/retired.tsv");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        for row in rows {
            text.push_str(&row.line());
            text.push('\n');
        }
        std::fs::write(&path, text).map_err(|error| error.to_string())
    }
}

/// Rewrites one feature file.
struct FeatureRewriter {
    lines: Vec<String>,
    retired: Vec<RetiredRow>,
    feature: String,
    task: String,
}

impl FeatureRewriter {
    /// The file's text after every row of the map, and the checks the map retired.
    fn rewrite(file: &FeatureFile, map: &RepointMap) -> Result<(String, Vec<RetiredRow>), String> {
        let mut rewriter = FeatureRewriter { lines: file.text().lines().map(str::to_string).collect(), retired: Vec::new(), feature: file.feature().to_string(), task: map.task().to_string() };
        for row in map.rows() {
            match row {
                RepointRow::Move { from_package, from_target, to_package, to_target, module } => rewriter.move_commands(from_package, from_target, to_package, to_target, module),
                RepointRow::RenameCrate { old, new } => rewriter.rename_commands(old, new),
                RepointRow::Replace { from, to } => rewriter.replace_proofs(from, to),
                RepointRow::Retire { test, reason } => rewriter.retire_proofs(test, reason),
            }
        }
        let mut text = rewriter.lines.join("\n");
        if file.text().ends_with('\n') {
            text.push('\n');
        }
        Ok((text, rewriter.retired))
    }

    /// The indexes of the lines outside fenced blocks.
    fn unfenced(&self) -> Vec<usize> {
        let mut inside = false;
        let mut indexes = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            if line.trim().starts_with("```") {
                inside = !inside;
            } else if !inside {
                indexes.push(index);
            }
        }
        indexes
    }

    /// The command a line carries, when it is a `Run` line or a lone `Proof:` command.
    fn command_of(line: &str) -> Option<String> {
        if let Some((_, command)) = FeatureFile::parse_run(line) {
            return Some(command);
        }
        let proof = FeatureFile::parse_proof(line)?;
        if FeatureFile::parse_run_proof(&proof).is_some() { None } else { Some(proof) }
    }

    /// Replaces every unfenced command that `rewrite` gives a new text for.
    fn rewrite_commands(&mut self, rewrite: impl Fn(&CargoTestCommand) -> Option<String>) {
        for index in self.unfenced() {
            let commands = match FeatureFile::parse_build(&self.lines[index]) {
                Some(commands) => commands,
                None => Self::command_of(&self.lines[index]).into_iter().collect(),
            };
            for command in commands {
                let Some(parsed) = CargoTestCommand::parse(&command) else { continue };
                if let Some(new_command) = rewrite(&parsed) {
                    self.lines[index] = self.lines[index].replacen(&command, &new_command, 1);
                }
            }
        }
    }

    /// Points every command that ran the moved target at its merged target.
    fn move_commands(&mut self, from_package: &str, from_target: &str, to_package: &str, to_target: &str, module: &str) {
        self.rewrite_commands(|command| {
            ((command.package() == Some(from_package) || command.package() == Some(to_package)) && command.targets().contains(&from_target)).then(|| command.moved(from_target, to_package, to_target, module))
        });
    }

    /// Renames the package of every command that names the old crate.
    fn rename_commands(&mut self, old: &str, new: &str) {
        self.rewrite_commands(|command| (command.package() == Some(old)).then(|| command.with_package(new)));
    }

    /// The file's runs, by name.
    fn runs(&self) -> BTreeMap<String, String> {
        self.unfenced().into_iter().filter_map(|index| FeatureFile::parse_run(&self.lines[index])).collect()
    }

    /// The index of the last unfenced `Run` line, where a new one goes after.
    fn last_run_index(&self) -> Option<usize> {
        self.unfenced().into_iter().rev().find(|&index| FeatureFile::parse_run(&self.lines[index]).is_some())
    }

    /// Adds the `Run` line after the last one when the file has no run called `name`.
    fn ensure_run(&mut self, name: &str, line: &str) {
        if self.runs().contains_key(name) {
            return;
        }
        if let Some(at) = self.last_run_index() {
            self.lines.insert(at + 1, line.to_string());
        }
    }

    /// The name of a run that targets `package/target`, creating one when the file has none.
    fn run_for(&mut self, package: &str, target: &str) -> String {
        let runs = self.runs();
        let existing = runs.iter().find(|(_, command)| CargoTestCommand::parse(command).is_some_and(|c| c.package() == Some(package) && c.targets() == [target]));
        if let Some((name, _)) = existing {
            return name.clone();
        }
        let mut name = target.replace('-', "_");
        while runs.contains_key(&name) {
            name.push('x');
        }
        self.ensure_run(&name, &format!("**Run {name}**: `cargo test -p {package} --test {target}`"));
        name
    }

    /// The proofs whose run targets the reference's package and target and whose test is its function:
    /// `(line index, run name, check id)`.
    fn proofs_of(&self, test: &TestReference) -> Vec<(usize, String, String)> {
        let runs = self.runs();
        let mut check = String::new();
        let mut found = Vec::new();
        for index in self.unfenced() {
            let line = &self.lines[index];
            if let Some((id, _)) = FeatureFile::parse_check(line) {
                check = id;
                continue;
            }
            let Some(proof) = FeatureFile::parse_proof(line) else { continue };
            let Some((run_name, expected)) = FeatureFile::parse_run_proof(&proof) else { continue };
            let targets_it = runs.get(&run_name).and_then(|command| CargoTestCommand::parse(command)).is_some_and(|c| c.package() == Some(&test.package) && (c.targets().is_empty() || c.targets().contains(&test.target.as_str())));
            if targets_it && FeatureFile::test_named_by(&expected).as_deref() == Some(test.function()) {
                found.push((index, run_name, check.clone()));
            }
        }
        found
    }

    /// Points the proofs of `from` at `to`, naming a run for the survivor's target when the file has none.
    fn replace_proofs(&mut self, from: &TestReference, to: &TestReference) {
        if from == to {
            return;
        }
        while let Some((index, run_name, _)) = self.proofs_of(from).into_iter().next() {
            let before = self.lines.len();
            let new_run = self.run_for(&to.package, &to.target);
            // a run line inserted above the proof shifts its index
            let at = index + (self.lines.len() - before);
            self.lines[at] = self.lines[at].replacen(&format!("run {run_name}"), &format!("run {new_run}"), 1).replacen(from.function(), &to.test, 1);
        }
    }

    /// Turns the proofs of `test` into the ledger's retirement proof and records each retired check.
    fn retire_proofs(&mut self, test: &TestReference, reason: &str) {
        while let Some((_, _, check)) = self.proofs_of(test).into_iter().next() {
            self.ensure_run("ledger", LEDGER_RUN);
            // the inserted run line moved the proof: find it again
            let Some((index, _, _)) = self.proofs_of(test).into_iter().find(|(_, _, id)| *id == check) else { break };
            let leading: String = self.lines[index].chars().take_while(|c| c.is_whitespace()).collect();
            self.lines[index] = format!("{leading}{RETIRED_PROOF}");
            self.annotate_check(&check, reason);
            self.retired.push(RetiredRow { feature: self.feature.clone(), check, test: test.spelled(), reason: reason.to_string() });
        }
    }

    /// Appends the retirement note to the claim line of `check`, once.
    fn annotate_check(&mut self, check: &str, reason: &str) {
        let note = format!(" (retired by code-clarity {}: {reason})", self.task);
        for index in self.unfenced() {
            if FeatureFile::parse_check(&self.lines[index]).is_some_and(|(id, _)| id == check) && !self.lines[index].contains("retired by code-clarity") {
                self.lines[index].push_str(&note);
            }
        }
    }
}

/// Rewrites the Book's inventory `covered_by` cells.
struct InventoryRewriter;

impl InventoryRewriter {
    /// The inventory's text after every row of the map; a retired test an inventory row cites is an error.
    fn rewrite(text: &str, map: &RepointMap) -> Result<String, String> {
        let mut rewritten: Vec<String> = Vec::new();
        for line in text.lines() {
            let mut cells: Vec<String> = line.split('\t').map(str::to_string).collect();
            if !line.starts_with('#') && cells.len() > 4 {
                for row in map.rows() {
                    cells[4] = Self::cell(&cells[4], row)?;
                }
            }
            rewritten.push(cells.join("\t"));
        }
        let mut result = rewritten.join("\n");
        if text.ends_with('\n') {
            result.push('\n');
        }
        Ok(result)
    }

    /// One `covered_by` cell after one row.
    fn cell(cell: &str, row: &RepointRow) -> Result<String, String> {
        Ok(match row {
            RepointRow::Move { from_package, from_target, to_package, to_target, module } => match cell.strip_prefix(&format!("{from_package}/{from_target}::")) {
                Some(rest) => format!("{to_package}/{to_target}::{module}::{rest}"),
                None => cell.to_string(),
            },
            RepointRow::RenameCrate { old, new } => match cell.strip_prefix(&format!("{old}/")) {
                Some(rest) => format!("{new}/{rest}"),
                None => cell.to_string(),
            },
            RepointRow::Replace { from, to } => match TestReference::parse(cell) {
                Ok(held) if held.package == from.package && held.target == from.target && held.function() == from.function() => to.spelled(),
                _ => cell.to_string(),
            },
            RepointRow::Retire { test, .. } => match TestReference::parse(cell) {
                Ok(held) if held.package == test.package && held.target == test.target && held.function() == test.function() => {
                    return Err(format!("an inventory row of the Book is covered by `{}`, which is retired: replace it instead", test.spelled()));
                }
                _ => cell.to_string(),
            },
        })
    }
}

/// Rewrites the `<package>/<target>` names the prose files carry.
struct ProseRewriter;

impl ProseRewriter {
    /// The prose's text after every row of the map.
    fn rewrite(text: &str, map: &RepointMap) -> String {
        let mut result = text.to_string();
        for row in map.rows() {
            result = match row {
                RepointRow::Move { from_package, from_target, to_package, to_target, module } => Self::replace_token(&result, &format!("{from_package}/{from_target}"), &format!("{to_package}/{to_target}::{module}"), false),
                RepointRow::RenameCrate { old, new } => Self::replace_token(&result, &format!("{old}/"), &format!("{new}/"), true),
                RepointRow::Replace { from, to } => Self::replace_token(&result, &from.spelled(), &to.spelled(), false),
                RepointRow::Retire { .. } => result,
            };
        }
        result
    }

    /// Replaces `from` where it is not the tail of a longer word (and, unless `after_slash`, of a path).
    fn replace_token(text: &str, from: &str, to: &str, after_slash: bool) -> String {
        let mut result = String::new();
        let mut rest = text;
        let mut previous = '\n';
        while let Some(at) = rest.find(from) {
            let before = rest[..at].chars().next_back().unwrap_or(previous);
            let after = rest[at + from.len()..].chars().next();
            let before_ok = !(before.is_alphanumeric() || before == '_' || before == '-' || (before == '/' && !after_slash));
            let after_ok = from.ends_with('/') || !after.is_some_and(|c| c.is_alphanumeric() || c == '_');
            result.push_str(&rest[..at]);
            result.push_str(if before_ok && after_ok { to } else { from });
            previous = from.chars().next_back().unwrap_or(previous);
            rest = &rest[at + from.len()..];
        }
        result.push_str(rest);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixture repository: one feature folder, the inventory and a prose file.
    struct Fixture {
        directory: tempfile::TempDir,
    }

    impl Fixture {
        /// A feature file with a build line, runs, checks and proofs.
        const TASKS: &'static str = "# Demo\n\n**Build**: `cargo build -p alpha`; `cargo test -p alpha --test old_target`\n**Run old**: `cargo test -p alpha --test old_target`\n**Run other**: `cargo test -p alpha --test other_target`\n\n**C1** - It holds.\nProof: run old | test it_holds ... ok\n\n**C2** - A prose pin.\nProof: run old | test a_pin ... ok\n\n**C3** - A duplicate.\nProof: run old | test a_copy ... ok\nProof: `cargo test -p alpha --test old_target`\n";
        /// Two inventory rows covered by tests of the old target.
        const INVENTORY: &'static str = "# header\nclaim\tc#1\tff\trun\talpha/old_target::it_holds\t-\t-\nclaim\tc#2\tff\trun\talpha/old_target::a_copy\t-\t-\n";
        /// A prose line naming the old target and a longer name that must stay.
        const GAPS: &'static str = "see alpha/old_target and crates/alpha/tests, not alpha/old_target_two\n";

        /// Writes the fixture repository into a temporary directory.
        fn new() -> Fixture {
            let directory = tempfile::tempdir().expect("a temporary directory");
            let feature = directory.path().join(".specs/features/demo");
            std::fs::create_dir_all(&feature).expect("the feature folder");
            std::fs::create_dir_all(directory.path().join("tests/book")).expect("the book folder");
            std::fs::create_dir_all(directory.path().join(".specs/features/code-clarity")).expect("the code-clarity folder");
            std::fs::write(feature.join("tasks.md"), Self::TASKS).expect("tasks.md");
            std::fs::write(directory.path().join("tests/book/inventory.tsv"), Self::INVENTORY).expect("inventory");
            std::fs::write(directory.path().join("GAPS.md"), Self::GAPS).expect("GAPS.md");
            Fixture { directory }
        }

        /// Applies a map of the given rows to the fixture.
        fn apply(&self, task: &str, rows: &str) {
            let map = RepointMap::parse(task, rows).expect("a valid map");
            Repointer::new(self.directory.path().to_path_buf()).apply(&map).expect("the map applies");
        }

        /// The text of a file of the fixture.
        fn read(&self, relative: &str) -> String {
            std::fs::read_to_string(self.directory.path().join(relative)).expect("a file of the fixture")
        }
    }

    #[test]
    fn a_move_retargets_runs_the_inventory_and_prose() {
        let fixture = Fixture::new();
        fixture.apply("T5", "move alpha/old_target beta/merged::old_target\n");
        assert_eq!(
            fixture.read(".specs/features/demo/tasks.md"),
            Fixture::TASKS.replace("**Build**: `cargo build -p alpha`; `cargo test -p alpha --test old_target`", "**Build**: `cargo build -p alpha`; `cargo test -p beta --test merged old_target::`").replace("**Run old**: `cargo test -p alpha --test old_target`", "**Run old**: `cargo test -p beta --test merged old_target::`").replace("Proof: `cargo test -p alpha --test old_target`", "Proof: `cargo test -p beta --test merged old_target::`")
        );
        assert_eq!(fixture.read("tests/book/inventory.tsv"), Fixture::INVENTORY.replace("alpha/old_target::", "beta/merged::old_target::"));
        assert_eq!(fixture.read("GAPS.md"), "see beta/merged::old_target and crates/alpha/tests, not alpha/old_target_two\n");
    }

    #[test]
    fn a_crate_rename_changes_every_package_flag_and_prefix() {
        let fixture = Fixture::new();
        fixture.apply("T139", "rename-crate alpha gamma\n");
        assert_eq!(fixture.read(".specs/features/demo/tasks.md"), Fixture::TASKS.replace("-p alpha", "-p gamma").replace("cargo build -p gamma", "cargo build -p alpha"));
        assert_eq!(fixture.read("tests/book/inventory.tsv"), Fixture::INVENTORY.replace("alpha/", "gamma/"));
        assert_eq!(fixture.read("GAPS.md"), "see gamma/old_target and crates/gamma/tests, not gamma/old_target_two\n");
    }

    #[test]
    fn a_replace_points_the_proof_and_the_inventory_at_the_survivor() {
        let fixture = Fixture::new();
        fixture.apply("T12", "replace alpha/old_target::a_copy alpha/other_target::it_holds_too\n");
        assert_eq!(fixture.read(".specs/features/demo/tasks.md"), Fixture::TASKS.replace("run old | test a_copy", "run other | test it_holds_too"));
        assert_eq!(fixture.read("tests/book/inventory.tsv"), Fixture::INVENTORY.replace("alpha/old_target::a_copy", "alpha/other_target::it_holds_too"));
    }

    #[test]
    fn a_replace_names_a_new_run_when_the_survivor_has_none() {
        let fixture = Fixture::new();
        fixture.apply("T12", "replace alpha/old_target::a_copy alpha/third::it_holds_too\n");
        let expected = Fixture::TASKS
            .replace("**Run other**: `cargo test -p alpha --test other_target`\n", "**Run other**: `cargo test -p alpha --test other_target`\n**Run third**: `cargo test -p alpha --test third`\n")
            .replace("run old | test a_copy", "run third | test it_holds_too");
        assert_eq!(fixture.read(".specs/features/demo/tasks.md"), expected);
    }

    #[test]
    fn a_retire_records_the_check_and_points_it_at_the_ledger() {
        let fixture = Fixture::new();
        fixture.apply("T10", "retire alpha/old_target::a_pin the claim was a wording pin\n");
        let expected = Fixture::TASKS
            .replace("**Run other**: `cargo test -p alpha --test other_target`\n", "**Run other**: `cargo test -p alpha --test other_target`\n**Run ledger**: `cargo test -p piperine-repository-guards --test proof_ledger`\n")
            .replace("**C2** - A prose pin.", "**C2** - A prose pin. (retired by code-clarity T10: the claim was a wording pin)")
            .replace("Proof: run old | test a_pin ... ok", "Proof: run ledger | test every_retired_check_is_recorded ... ok");
        assert_eq!(fixture.read(".specs/features/demo/tasks.md"), expected);
        assert_eq!(fixture.read(".specs/features/code-clarity/retired.tsv"), "demo\tC2\talpha/old_target::a_pin\tthe claim was a wording pin\n");
    }

    #[test]
    fn retiring_a_test_an_inventory_row_cites_is_refused() {
        let fixture = Fixture::new();
        let map = RepointMap::parse("T10", "retire alpha/old_target::it_holds a pin\n").expect("a valid map");
        let error = Repointer::new(fixture.directory.path().to_path_buf()).apply(&map).expect_err("the inventory cites it");
        assert!(error.contains("retired: replace it instead"), "{error}");
    }

    #[test]
    fn a_row_that_is_no_row_is_an_error_with_its_line() {
        let error = RepointMap::parse("T1", "# a comment\nmove only-one\n").err().expect("a bad row");
        assert!(error.starts_with("line 2:"), "{error}");
    }
}
