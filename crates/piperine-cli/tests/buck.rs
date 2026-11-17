//! The buck converter from the command line (feature `msl-cli`, T21; CLI-08): the real binary
//! runs `idle` and `startup` of `examples/buck`. Expected values come from
//! `examples/buck/SPEC.md` and the supervisor's modes (`Off` is 0, §8), never from a run. The
//! `startup` run is the closed-loop start of `msl-digital` DIG-16 (about two minutes in a dev
//! build), so one shared fixture runs it once for every test that reads it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use piperine_api::Project;

/// `examples/buck`.
fn buck() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck")
}

/// One cache directory for both runs of this target.
fn cache() -> &'static Path {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE.get_or_init(|| tempfile::tempdir().expect("tmp")).path()
}

/// `piperine run <name>` in `examples/buck`, its standard output; the run must have completed.
fn run(name: &str) -> String {
    let out: Output = Command::new(env!("CARGO_BIN_EXE_piperine")).args(["run", name]).current_dir(buck()).env("PIPERINE_CACHE_DIR", cache()).env_remove("PIPERINE_BACKEND").output().expect("run piperine");
    assert_eq!(out.status.code(), Some(0), "{name}: stderr:\n{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("text")
}

/// The one run of `startup` of this target.
fn startup() -> &'static str {
    static OUT: OnceLock<String> = OnceLock::new();
    OUT.get_or_init(|| run("startup"))
}

/// The table of a run: its header row and data rows, as cells.
fn table(out: &str) -> (Vec<&str>, Vec<Vec<&str>>) {
    let mut lines = out.lines().skip_while(|l| !l.is_empty()).skip(1);
    let header = lines.next().unwrap_or_else(|| panic!("no table in the output")).split_whitespace().collect();
    (header, lines.map(|l| l.split_whitespace().collect()).collect())
}

/// The cells of the column `name` of a table.
fn column<'a>(header: &[&str], rows: &'a [Vec<&'a str>], name: &str) -> Vec<&'a str> {
    let at = header.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column `{name}` in {header:?}"));
    rows.iter().map(|r| r[at]).collect()
}

#[test]
fn the_buck_has_its_bench_and_two_runs() {
    // CLI-08 AC 1: `benches/startup.msl` and the runs `idle` and `startup`
    assert!(buck().join("benches/startup.msl").is_file(), "benches/startup.msl");
    let project = Project::at(&buck()).unwrap_or_else(|f| panic!("{f}"));
    let mut names: Vec<&str> = project.runs().iter().map(|r| r.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["idle", "startup"]);
    assert!(project.faults().is_empty(), "{:?}", project.faults());
}

#[test]
fn startup_ends_with_the_reference_at_128() {
    // CLI-08 AC 2: the run completes on the Rust backend and the last row of its table gives 128
    // for the controller's reference (SPEC §7.2: 0.8 V is code 0, reference 128), and SPEC §2's
    // DC regulation, 1 % or 10 mV whichever is larger, holds for `vout` in that row
    let out = startup();
    assert_eq!(out.lines().next(), Some("run startup: transient, top Startup, backend rust"));
    let (header, rows) = table(out);
    let reference = column(&header, &rows, "Startup.buck.ctl.reference");
    assert_eq!(reference.last(), Some(&"128"), "{header:?} {:?}", rows.last());
    let vout: f64 = column(&header, &rows, "Startup.buck.vout").last().expect("a row").parse().expect("a number");
    assert!((vout - 0.8).abs() <= 10e-3, "vout at the end: {vout}");
    // the supervisor ended in `Regulate` (mode 2, after `Off` 0 and `SoftStart` 1, SPEC §8)
    assert_eq!(column(&header, &rows, "Startup.mode").last(), Some(&"2"));
}

#[test]
fn startup_prints_no_failed_check() {
    // CLI-08 AC 3: a passing check is hidden on the terminal (CLI-06 AC 9) and a failing one
    // would print, so no ` checks ` line on standard output means no failed check; the claim is
    // not vacuous: the DPWM's no-shoot-through monitor checks at every clock of the 600 us, and
    // `--records` keeps every record, all of them passing
    assert!(!startup().lines().any(|l| l.starts_with('@') && l.split_whitespace().nth(2) == Some("checks")), "a checks line was printed");
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("startup.jsonl");
    let out = Command::new(env!("CARGO_BIN_EXE_piperine"))
        .args(["run", "startup", "--records"])
        .arg(&file)
        .current_dir(buck())
        .env("PIPERINE_CACHE_DIR", cache())
        .env_remove("PIPERINE_BACKEND")
        .output()
        .expect("run piperine");
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
    let records = std::fs::read_to_string(&file).expect("the records file");
    let checks: Vec<&str> = records.lines().filter(|l| l.contains("\"stream\":\"checks\"")).collect();
    assert!(checks.len() > 1000, "the monitor checks at every clock: {} records", checks.len());
    let failed: Vec<&&str> = checks.iter().filter(|l| !l.contains("\"pass\":true")).collect();
    assert!(failed.is_empty(), "{} checks not passing, the first: {:?}", failed.len(), failed.first());
}

#[test]
fn the_buck_runs_print_at_most_16_lines() {
    // CLI-08 AC 6: with no flag, `idle` and `startup` print at most 16 lines
    for (name, out) in [("startup", startup().to_string()), ("idle", run("idle"))] {
        assert!(out.lines().count() <= 16, "{name} printed {} lines", out.lines().count());
    }
}

#[test]
fn idle_keeps_the_mode_off() {
    // CLI-08 AC 4: with `en` low the supervisor stays `Off`, 0 in every row (SPEC §8)
    let out = run("idle");
    assert_eq!(out.lines().next(), Some("run idle: transient, top Idle, backend rust"));
    let (header, rows) = table(&out);
    assert!(!rows.is_empty());
    let mode = column(&header, &rows, "Idle.mode");
    assert!(mode.iter().all(|m| *m == "0"), "a row where the mode is not 0: {:?}", mode.iter().find(|m| **m != "0"));
}

#[test]
fn piperine_test_passes_on_the_buck() {
    // CLI-08 AC 5: `piperine test` in `examples/buck` exits 0, both runs judged by the DPWM's
    // monitor (the passing checks are hidden on a terminal, but they are counted)
    let out: Output = Command::new(env!("CARGO_BIN_EXE_piperine")).arg("test").current_dir(buck()).env("PIPERINE_CACHE_DIR", cache()).env_remove("PIPERINE_BACKEND").output().expect("run piperine");
    let stdout = String::from_utf8(out.stdout).expect("text");
    assert_eq!(out.status.code(), Some(0), "stdout:\n{stdout}\nstderr:\n{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(lines[0].starts_with("PASS idle (") && lines[1].starts_with("PASS startup ("), "{stdout}");
    assert_eq!(lines[2], "2 runs, 2 passed, 0 failed");
}
