//! `piperine test` runs the project's Python testbenches (`*_tb.py`):
//! discovery (root + nested, skipping `.venv`/`target`), per-file PASS/FAIL
//! with tracebacks, a killable per-file timeout, and exit codes (BRM-08/09).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use piperine_cli::commands::test::discover_testbenches;

/// A scratch project: `Piperine.toml` marker + the given testbench files
/// (relative path → content).
fn scratch_project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("Piperine.toml"), "[project]\nname = \"scratch\"\n").unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

/// `piperine test` (optionally with extra args / env) run in `dir`.
fn piperine_test(dir: &Path, extra: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_piperine"));
    cmd.arg("test").args(extra).current_dir(dir);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn piperine test")
}

fn combined(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// Discovery finds `*_tb.py` at the root and nested under `tests/`, and
/// never recurses into `.venv`/`target`.
#[test]
fn discovery_finds_tb_files_and_skips_venv_and_target() {
    let project = scratch_project(&[
        ("top_tb.py", "pass"),
        ("tests/nested_tb.py", "pass"),
        ("tests/helper.py", "pass"),
        (".venv/hidden_tb.py", "pass"),
        ("target/hidden_tb.py", "pass"),
    ]);
    let found = discover_testbenches(project.path());
    let names: Vec<String> = found
        .iter()
        .map(|p| p.strip_prefix(project.path()).unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["tests/nested_tb.py".to_string(), "top_tb.py".to_string()],
        "sorted, filtered discovery"
    );
}

/// A passing testbench: exit 0, per-file PASS line, summary.
#[test]
fn passing_testbench_exits_zero() {
    let project = scratch_project(&[("ok_tb.py", "print('tb ran')\n")]);
    let out = piperine_test(project.path(), &[], &[]);
    assert!(out.status.success(), "exit 0 on pass: {}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("PASS") && text.contains("ok_tb.py"), "per-file PASS: {text}");
    assert!(text.contains("1 run, 1 passed, 0 failed"), "summary: {text}");
}

/// A raising testbench: exit 1, the file is marked FAIL and the traceback
/// text is shown.
#[test]
fn failing_testbench_shows_traceback_and_exits_one() {
    let project = scratch_project(&[("bad_tb.py", "raise RuntimeError('boom-marker')\n")]);
    let out = piperine_test(project.path(), &[], &[]);
    assert_eq!(out.status.code(), Some(1), "exit 1 on failure: {}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("FAIL") && text.contains("bad_tb.py"), "per-file FAIL: {text}");
    assert!(text.contains("boom-marker"), "traceback shown: {text}");
    assert!(text.contains("1 run, 0 passed, 1 failed"), "summary: {text}");
}

/// No testbenches: notice + exit 0.
#[test]
fn no_testbenches_is_a_notice_not_a_failure() {
    let project = scratch_project(&[]);
    let out = piperine_test(project.path(), &[], &[]);
    assert!(out.status.success(), "exit 0 when empty: {}", combined(&out));
    assert!(combined(&out).contains("No Python testbenches"), "notice: {}", combined(&out));
}

/// A hanging testbench is killed at the timeout and marked failed.
#[test]
fn hanging_testbench_is_killed_and_failed() {
    let project = scratch_project(&[("hang_tb.py", "import time\ntime.sleep(60)\n")]);
    let out = piperine_test(project.path(), &[], &[("PIPERINE_TEST_TIMEOUT_SECS", "2")]);
    assert_eq!(out.status.code(), Some(1), "exit 1 on timeout: {}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("FAIL") && text.contains("hang_tb.py"), "marked failed: {text}");
    assert!(text.contains("timeout"), "timeout named: {text}");
}

/// `--list` prints the discovered files without running them (a raising
/// script still exits 0).
#[test]
fn list_prints_without_running() {
    let project = scratch_project(&[("bad_tb.py", "raise RuntimeError('must not run')\n")]);
    let out = piperine_test(project.path(), &["--list"], &[]);
    assert!(out.status.success(), "exit 0 on --list: {}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("bad_tb.py"), "listed: {text}");
    assert!(!text.contains("must not run"), "not executed: {text}");
}

/// An explicit file argument runs just that file.
#[test]
fn explicit_file_runs_only_it() {
    let project = scratch_project(&[
        ("ok_tb.py", "pass\n"),
        ("other_tb.py", "raise RuntimeError('not run')\n"),
    ]);
    let out = piperine_test(project.path(), &["ok_tb.py"], &[]);
    assert!(out.status.success(), "exit 0: {}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("1 run, 1 passed, 0 failed"), "only the named file: {text}");
}

/// `piperine run <file>.phdl` no longer executes bench entry points (the
/// bench was removed): it elaborates the file and prints the migration
/// notice pointing at `*_tb.py` testbenches.
#[test]
fn run_phdl_elaborates_and_points_at_testbenches() {
    let project = scratch_project(&[(
        "src/main.phdl",
        "discipline Electrical { potential v: Real; flow i: Real; }\n\
         mod Top() { wire gnd : Electrical; }\n",
    )]);
    let out = Command::new(env!("CARGO_BIN_EXE_piperine"))
        .arg("run")
        .current_dir(project.path())
        .output()
        .expect("spawn piperine run");
    let text = combined(&out);
    assert!(text.contains("elaborates"), "elaboration reported: {text}");
    assert!(text.contains("bench") && text.contains("removed"), "removal named: {text}");
    assert!(text.contains("_tb.py"), "migration path shown: {text}");
}
