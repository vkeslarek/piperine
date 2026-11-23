//! Scope: the guard of the parity corpus (requirements PAR-07 and PAR-05 AC 5; feature `root-e2e`,
//! RE-04 and RE-05): every twin under `tests/e2e/` has a `.cir`, a `.command`, its expected
//! `.stdout` and a test function named after it in a `parity_*` target; a library `.msl` is
//! loaded by a twin of its scenario directory; a scenario that holds `benches/` holds
//! `Piperine.toml`; `tests/e2e/twins/` does not exist; and no `parity_*` target compares an
//! absolute difference with a numeric literal. Each check is a method returning its failures:
//! empty on the tree, non-empty on text or a tree planted by a test.

use std::path::{Path, PathBuf};

/// The checks, over text or a scenario handed in.
struct Guard;

impl Guard {
    /// The twin names (every `<name>.msl` but `devices.msl`) that have no `.cir` among `cirs` or no
    /// `fn <name>()` in a target of `targets` (`(file, text)`), each as one message.
    fn unmatched_twins(twins: &[String], cirs: &[String], targets: &[(String, String)]) -> Vec<String> {
        let mut failures = Vec::new();
        for twin in twins.iter().filter(|name| name.as_str() != "devices") {
            if !cirs.contains(twin) {
                failures.push(format!("tests/e2e/twins/{twin}.msl has no tests/e2e/twins/{twin}.cir"));
            }
            let signature = format!("fn {twin}()");
            if !targets.iter().any(|(_, text)| text.contains(&signature)) {
                failures.push(format!("tests/e2e/twins/{twin}.msl has no test function `{twin}` in a parity_* target"));
            }
        }
        failures
    }

    /// The twins of `scenario` (an `.msl` beside a `.cir`) that have no `<name>.stdout`.
    fn twins_without_expected_output(scenario: &Scenario) -> Vec<String> {
        scenario.twin_names().into_iter().filter(|name| !scenario.has_file(&format!("{name}.stdout"))).map(|name| format!("{}/{name}.stdout is missing: the twin has no expected output", scenario.name)).collect()
    }

    /// The twins of `scenario` that have no `<name>.command`.
    fn twins_without_command(scenario: &Scenario) -> Vec<String> {
        scenario.twin_names().into_iter().filter(|name| !scenario.has_file(&format!("{name}.command"))).map(|name| format!("{}/{name}.command is missing: the twin has no command", scenario.name)).collect()
    }

    /// The libraries of `scenario` (an `.msl` with no `.cir`) that no twin of it loads with
    /// `mod <library>;`.
    fn libraries_no_twin_loads(scenario: &Scenario) -> Vec<String> {
        let twins = scenario.twin_names();
        let mut failures = Vec::new();
        for library in scenario.library_names() {
            let declaration = format!("mod {library};");
            let loaded = twins.iter().any(|twin| scenario.source(twin).is_some_and(|text| text.contains(&declaration)));
            if !loaded {
                failures.push(format!("{}/{library}.msl has no .cir and no twin of its scenario loads it with `{declaration}`", scenario.name));
            }
        }
        failures
    }

    /// `scenario`, when it holds `benches/` but no `Piperine.toml`.
    fn benches_without_manifest(scenario: &Scenario) -> Vec<String> {
        if scenario.has_benches && !scenario.has_file("Piperine.toml") {
            return vec![format!("{} holds benches/ but no Piperine.toml", scenario.name)];
        }
        Vec::new()
    }

    /// `tests/e2e/twins` under `tests_directory`, when it exists: the twins have one home.
    fn the_old_home_exists(tests_directory: &Path) -> Vec<String> {
        if tests_directory.join("ngspice").exists() {
            return vec!["tests/e2e/twins exists: the twins live under tests/e2e/".to_string()];
        }
        Vec::new()
    }

    /// Each line of `text` that compares an absolute difference (`.abs()`) with a numeric
    /// literal, as `file:line: text`. Every `.abs()` of a line is read, in both operand orders:
    /// `x.abs() < 1e-3` (a comparison operator then a number after it) and `1e-3 > x.abs()` (a
    /// number then a comparison operator before it).
    fn literal_comparisons(file: &str, text: &str) -> Vec<String> {
        let mut failures = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let hit = line.match_indices(".abs()").any(|(at, call)| Self::literal_after(&line[at + call.len()..]) || Self::literal_before(&line[..at]));
            if hit {
                failures.push(format!("{file}:{}: {}", index + 1, line.trim()));
            }
        }
        failures
    }

    /// Whether `rest` starts with a comparison operator followed by a number.
    fn literal_after(rest: &str) -> bool {
        let rest = rest.trim_start();
        let after = rest.strip_prefix("<=").or_else(|| rest.strip_prefix(">=")).or_else(|| rest.strip_prefix('<')).or_else(|| rest.strip_prefix('>'));
        after.is_some_and(|after| after.trim_start().starts_with(|c: char| c.is_ascii_digit()))
    }

    /// Whether `head` (the text before a `.abs()`) has a number as the operand of the last
    /// comparison operator in it: `1e-3 > (a - b)` ends so. An arrow (`->`, `=>`) is no comparison.
    fn literal_before(head: &str) -> bool {
        let bytes = head.as_bytes();
        let Some(at) = (0..bytes.len()).rev().find(|&i| matches!(bytes[i], b'<' | b'>') && !(i > 0 && matches!(bytes[i - 1], b'-' | b'='))) else { return false };
        let operand = head[..at].trim_end().trim_end_matches(['<', '>', '=']).trim_end();
        let token_start = operand.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).map_or(0, |i| i + 1);
        operand[token_start..].starts_with(|c: char| c.is_ascii_digit())
    }
}

/// One scenario directory of `tests/e2e/`: its file names, the text of its `.msl` files and
/// whether it holds `benches/`.
struct Scenario {
    name: String,
    files: Vec<String>,
    sources: Vec<(String, String)>,
    has_benches: bool,
}

impl Scenario {
    /// Reads the scenario at `directory`.
    fn read(directory: &Path) -> Scenario {
        let mut files = Vec::new();
        let mut sources = Vec::new();
        for entry in std::fs::read_dir(directory).expect("a readable scenario") {
            let path = entry.expect("a directory entry").path();
            let Some(name) = path.file_name().map(|name| name.to_string_lossy().into_owned()) else { continue };
            if path.is_file() && name.ends_with(".msl") {
                sources.push((name.clone(), std::fs::read_to_string(&path).expect("a readable model")));
            }
            if path.is_file() {
                files.push(name);
            }
        }
        files.sort();
        sources.sort();
        let name = directory.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Scenario { name, files, sources, has_benches: directory.join("benches").is_dir() }
    }

    /// Whether the scenario holds a file of that name.
    fn has_file(&self, name: &str) -> bool {
        self.files.iter().any(|file| file == name)
    }

    /// The stems of the files with `extension`, sorted.
    fn stems(&self, extension: &str) -> Vec<String> {
        let suffix = format!(".{extension}");
        self.files.iter().filter_map(|file| file.strip_suffix(&suffix)).map(str::to_string).collect()
    }

    /// The twins: an `.msl` beside a `.cir`.
    fn twin_names(&self) -> Vec<String> {
        let netlists = self.stems("cir");
        self.stems("msl").into_iter().filter(|name| netlists.contains(name)).collect()
    }

    /// The libraries: an `.msl` with no `.cir`.
    fn library_names(&self) -> Vec<String> {
        let netlists = self.stems("cir");
        self.stems("msl").into_iter().filter(|name| !netlists.contains(name)).collect()
    }

    /// The text of `<name>.msl`.
    fn source(&self, name: &str) -> Option<&str> {
        let file = format!("{name}.msl");
        self.sources.iter().find(|(found, _)| *found == file).map(|(_, text)| text.as_str())
    }
}

/// The tree's own files.
struct Tree;

impl Tree {
    /// `tests/` of the repository.
    fn tests() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests")
    }

    /// Every scenario directory of `tests/e2e/`, by name.
    fn scenarios() -> Vec<Scenario> {
        let mut directories: Vec<PathBuf> = std::fs::read_dir(Self::tests().join("e2e")).expect("tests/e2e exists").map(|entry| entry.expect("a directory entry").path()).filter(|path| path.is_dir()).collect();
        directories.sort();
        directories.iter().map(|directory| Scenario::read(directory)).collect()
    }

    /// The stems of the files with `extension`, over every scenario.
    fn stems(extension: &str) -> Vec<String> {
        let mut names: Vec<String> = Self::scenarios().iter().flat_map(|scenario| scenario.stems(extension)).collect();
        names.sort();
        names
    }

    /// The `parity_*` targets that compare against ngspice, as `(file name, text)`: every
    /// `tests/parity_*.rs` but this guard.
    fn parity_targets() -> Vec<(String, String)> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let mut targets: Vec<(String, String)> = std::fs::read_dir(directory)
            .expect("tests exists")
            .map(|entry| entry.expect("a directory entry").path())
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                (name.starts_with("parity_") && name.ends_with(".rs") && name != "parity_corpus.rs").then(|| (name, std::fs::read_to_string(&path).expect("a readable target")))
            })
            .collect();
        targets.sort();
        targets
    }
}

#[test]
fn every_twin_has_a_cir_and_a_test() {
    let twins = Tree::stems("msl");
    assert!(twins.iter().filter(|t| *t != "devices").count() == 19, "the corpus holds 19 twins: {twins:?}");
    let failures = Guard::unmatched_twins(&twins, &Tree::stems("cir"), &Tree::parity_targets());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn no_parity_target_compares_against_a_literal() {
    let targets = Tree::parity_targets();
    assert!(targets.len() >= 4, "the parity targets are found: {:?}", targets.iter().map(|(n, _)| n).collect::<Vec<_>>());
    let failures: Vec<String> = targets.iter().flat_map(|(file, text)| Guard::literal_comparisons(file, text)).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_literal_tolerance_in_a_parity_target_is_reported() {
    let planted = concat!(
        "fn x() {\n",
        "    assert!((a - b).abs() < 1e-3);\n",
        "    assert!((a - b).abs() <= 5.0);\n",
        "    assert!((a - b).abs() < tolerance);\n",
        "    assert!(1e-3 > (a - b).abs());\n",
        "    assert!(0.5 >= (a - b).abs());\n",
        "    assert!(c.abs() < d && (a - b).abs() < 1e-6);\n",
        "    assert!(1e-3 > c.abs() || tolerance > d.abs());\n",
        "    assert!(tolerance > (a - b).abs());\n",
        "    let f = |x| -> f64 { x.abs() };\n",
        "    assert!(a1 > (a - b).abs());\n",
        "}\n"
    );
    let failures = Guard::literal_comparisons("parity_planted.rs", planted);
    let lines: Vec<usize> = failures.iter().map(|f| f.split(':').nth(1).expect("a line number").parse().expect("a number")).collect();
    assert_eq!(lines, vec![2, 3, 5, 6, 7, 8], "{failures:?}");
    assert!(failures[0].starts_with("parity_planted.rs:2:"), "{failures:?}");
}

#[test]
fn the_guards_report_planted_text() {
    let twins = vec!["alpha".to_string(), "beta".to_string(), "devices".to_string()];
    let cirs = vec!["alpha".to_string()];
    let targets = vec![("parity_op.rs".to_string(), "#[test]\nfn alpha() {}\n".to_string())];
    let failures = Guard::unmatched_twins(&twins, &cirs, &targets);
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures.iter().any(|f| f.contains("beta.msl has no tests/e2e/twins/beta.cir")), "{failures:?}");
    assert!(failures.iter().any(|f| f.contains("beta.msl has no test function `beta`")), "{failures:?}");
    assert!(Guard::unmatched_twins(&twins[..1], &cirs, &targets).is_empty());
}

#[test]
fn every_twin_has_its_files_in_one_scenario_directory() {
    // RE-04, RE-05: a twin's `.cir`, `.command`, expected `.stdout` and library sit in one scenario
    // directory, and no twin name is in two of them
    let scenarios = Tree::scenarios();
    assert!(!scenarios.is_empty(), "tests/e2e/ holds a scenario directory");
    let failures: Vec<String> = scenarios.iter().flat_map(|scenario| [Guard::twins_without_expected_output(scenario), Guard::twins_without_command(scenario), Guard::libraries_no_twin_loads(scenario)].concat()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let mut names: Vec<String> = scenarios.iter().flat_map(Scenario::twin_names).collect();
    names.sort();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "a twin name is in two scenario directories");
}

#[test]
fn tests_ngspice_holds_no_twin() {
    // RE-04: the twins moved once, to `tests/e2e/`; the old home is gone
    let failures = Guard::the_old_home_exists(&Tree::tests());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_scenario_with_benches_holds_a_manifest() {
    // spec edge 3: a scenario directory that holds `benches/` is a package, so it holds `Piperine.toml`
    let failures: Vec<String> = Tree::scenarios().iter().flat_map(Guard::benches_without_manifest).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_layout_guards_report_planted_trees() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let scenario_directory = root.path().join("e2e").join("planted");
    std::fs::create_dir_all(scenario_directory.join("benches")).expect("a planted scenario");
    for (file, text) in [("alpha.msl", "mod devices;\n"), ("alpha.cir", ""), ("devices.msl", ""), ("orphan.msl", "")] {
        std::fs::write(scenario_directory.join(file), text).expect("a planted file");
    }
    std::fs::create_dir(root.path().join("ngspice")).expect("a planted old home");

    let scenario = Scenario::read(&scenario_directory);
    assert_eq!(Guard::twins_without_expected_output(&scenario), vec!["planted/alpha.stdout is missing: the twin has no expected output".to_string()]);
    assert_eq!(Guard::twins_without_command(&scenario), vec!["planted/alpha.command is missing: the twin has no command".to_string()]);
    let unloaded = Guard::libraries_no_twin_loads(&scenario);
    assert_eq!(unloaded.len(), 1, "{unloaded:?}");
    assert!(unloaded[0].contains("planted/orphan.msl") && unloaded[0].contains("mod orphan;"), "{unloaded:?}");
    assert_eq!(Guard::benches_without_manifest(&scenario), vec!["planted holds benches/ but no Piperine.toml".to_string()]);
    assert_eq!(Guard::the_old_home_exists(root.path()).len(), 1);

    // the same tree, repaired, is clean
    std::fs::write(scenario_directory.join("alpha.stdout"), "").expect("the expected output");
    std::fs::write(scenario_directory.join("alpha.command"), "piperine run alpha.msl\n").expect("the command");
    std::fs::write(scenario_directory.join("Piperine.toml"), "").expect("the manifest");
    std::fs::write(scenario_directory.join("alpha.msl"), "mod devices;\nmod orphan;\n").expect("a model that loads both libraries");
    std::fs::remove_dir(root.path().join("ngspice")).expect("the old home removed");
    let repaired = Scenario::read(&scenario_directory);
    let failures: Vec<String> = [Guard::twins_without_expected_output(&repaired), Guard::twins_without_command(&repaired), Guard::libraries_no_twin_loads(&repaired), Guard::benches_without_manifest(&repaired), Guard::the_old_home_exists(root.path())].concat();
    assert!(failures.is_empty(), "{failures:?}");
}
