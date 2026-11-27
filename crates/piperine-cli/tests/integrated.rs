//! The binary on the integrated package `tests/e2e/integrated/` (feature `integrated-tests`, IT-10;
//! MD-77): `piperine run pi_loop` prints its run line and the whole table of the closed loop, no failed
//! check, with every check record passing in the records file; `piperine test` passes `pi_loop` and
//! `threshold`. The closed loop is a 100 us transient on a 10 ns grid (10 001 rows), so one shared
//! run serves the tests that read its output. This is the binary's own proof that replaced the buck's
//! long runs, which are on demand (`examples/buck/README.md`, "On demand").

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use piperine_parity::compile_cache::CompileCache;

/// The package `tests/e2e/integrated/` as the binary runs it.
struct IntegratedPackage;

impl IntegratedPackage {
    /// The directory of the package.
    fn dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/integrated")
    }

    /// One cache directory for every run of this target.
    fn cache() -> &'static Path {
        CompileCache::shared().path()
    }

    /// `piperine <args>` in the package, with the shared cache.
    fn run(args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_piperine")).args(args).current_dir(IntegratedPackage::dir()).env("PIPERINE_CACHE_DIR", IntegratedPackage::cache()).env_remove("PIPERINE_BACKEND").output().expect("run piperine")
    }

    /// The standard output of `piperine run pi_loop`, run once; the run must have completed.
    fn pi_loop() -> &'static str {
        static OUT: OnceLock<String> = OnceLock::new();
        OUT.get_or_init(|| {
            let out = IntegratedPackage::run(&["run", "pi_loop"]);
            assert_eq!(out.status.code(), Some(0), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).expect("text")
        })
    }
}

/// The table a run printed: its header row and its data rows, as cells.
struct PrintedTable {
    /// The names of the columns.
    header: Vec<String>,
    /// The data rows, one vector of cells each.
    rows: Vec<Vec<String>>,
}

impl PrintedTable {
    /// The table of `stdout`: after the run line and a blank line, a header row, then the data rows.
    fn of(stdout: &str) -> PrintedTable {
        let mut lines = stdout.lines().skip_while(|line| !line.is_empty()).skip(1);
        let header = lines.next().unwrap_or_else(|| panic!("no table in the output")).split_whitespace().map(str::to_string).collect();
        PrintedTable { header, rows: lines.map(|line| line.split_whitespace().map(str::to_string).collect()).collect() }
    }

    /// The cells of the column `name`.
    fn column(&self, name: &str) -> Vec<&str> {
        let at = self.header.iter().position(|label| label == name).unwrap_or_else(|| panic!("no column `{name}` in {:?}", self.header));
        self.rows.iter().map(|row| row[at].as_str()).collect()
    }
}

#[test]
fn pi_loop_prints_its_run_line_and_10001_rows_ending_near_128() {
    // IT-10 AC 1: the run line first, then 10 001 rows (100 us on a 10 ns grid, both ends); the last row's
    // code is within 128 +- 3
    let out = IntegratedPackage::pi_loop();
    assert_eq!(out.lines().next(), Some("run pi_loop: transient, top PiLoop, backend rust"));
    let table = PrintedTable::of(out);
    assert_eq!(table.rows.len(), 10_001, "{:?}", table.header);
    let code: i64 = table.column("PiLoop.code").last().expect("a row").parse().expect("a code");
    assert!((125..=131).contains(&code), "the last code: {code}");
}

#[test]
fn pi_loop_prints_no_failed_check_and_records_every_passing_one() {
    // IT-10 AC 2: a passing check is hidden on the terminal and a failing one would print, so no `checks` line
    // means no failed check; not vacuous: the records file holds more than 50 checks, all passing
    assert!(!IntegratedPackage::pi_loop().lines().any(|line| line.starts_with('@') && line.split_whitespace().nth(2) == Some("checks")), "a checks line was printed");
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("pi_loop.jsonl");
    let out = IntegratedPackage::run(&["run", "pi_loop", "--records", file.to_str().expect("a path")]);
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
    let records = std::fs::read_to_string(&file).expect("the records file");
    let checks: Vec<&str> = records.lines().filter(|line| line.contains("\"stream\":\"checks\"")).collect();
    assert!(checks.len() > 50, "the monitors check at every clock: {} records", checks.len());
    let failed: Vec<&&str> = checks.iter().filter(|line| !line.contains("\"pass\":true")).collect();
    assert!(failed.is_empty(), "{} checks not passing, the first: {:?}", failed.len(), failed.first());
}

#[test]
fn piperine_test_passes_pi_loop_and_threshold() {
    // IT-10 AC 3: `piperine test pi_loop threshold` exits 0 and prints a PASS line for each and the summary
    let out = IntegratedPackage::run(&["test", "pi_loop", "threshold"]);
    let stdout = String::from_utf8(out.stdout).expect("text");
    assert_eq!(out.status.code(), Some(0), "stdout:\n{stdout}\nstderr:\n{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(lines[0].starts_with("PASS pi_loop (") && lines[1].starts_with("PASS threshold ("), "{stdout}");
    assert_eq!(lines[2], "2 runs, 2 passed, 0 failed");
}
