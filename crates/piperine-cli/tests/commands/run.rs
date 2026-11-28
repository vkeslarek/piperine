//! `piperine run` (feature `msl-cli`, phase 3; CLI-06 and CLI-07): the real binary runs the
//! named runs of `examples/rc` and its output is held to the spec's format, the order of the
//! header, the records and the table, the exit code, the determinism of standard output and the
//! announcement of a first compile on standard error.
//!
//! Every child process gets `PIPERINE_CACHE_DIR` set to a temporary directory shared by this
//! target (a design compiles once), except where a test needs a cache of its own. No test sets
//! a variable of its own process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use piperine_parity::compile_cache::CompileCache;

/// `examples/rc`, the project every test here runs.
fn rc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc")
}

/// `piperine <args>` in `dir`, on the cache `cache`.
fn piperine_in(dir: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", cache).env_remove("PIPERINE_BACKEND").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(env!("CARGO_BIN_EXE_piperine")).display()))
}

fn piperine(args: &[&str]) -> Output {
    piperine_in(&rc(), CompileCache::shared().path(), args)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The standard output of a run that must have completed.
fn stdout_of(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", text(&out.stderr));
    text(&out.stdout)
}

/// The cells of a line, split at runs of spaces.
fn cells(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

/// The reals in `a` and `b` agree to the relative tolerance `rel` (the parity contract's 1e-3).
fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(f64::MIN_POSITIVE)
}

/// The value of `name=<value>` among the cells of a record line.
fn field(line: &str, name: &str) -> String {
    let prefix = format!("{name}=");
    cells(line).into_iter().find_map(|c| c.strip_prefix(prefix.as_str()).map(str::to_string)).unwrap_or_else(|| panic!("no `{name}=` in `{line}`"))
}

#[test]
fn run_prints_header_records_blank_line_and_table() {
    // Book claims held here: cli~31fc7600, cli~e7ed5598
    // CLI-07 AC 1: the header `run <name>: <analysis>, top <Top>, backend <backend>`, the record
    // lines, one empty line, the table. The name and analysis are `examples/rc/Piperine.toml`'s
    // (`[run.step]`, `analysis = "transient"`); the top is the one module without ports of its
    // bench, `Step`; the backend is `rust`, the default (CLI-04 AC 7).
    let out = stdout_of(&piperine(&["run", "step"]));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "run step: transient, top Step, backend rust", "{out}");
    let blank = lines.iter().position(|l| l.is_empty()).unwrap_or_else(|| panic!("no empty line in\n{out}"));
    assert!(lines[1..blank].iter().all(|l| l.starts_with('@')), "between the header and the empty line only records:\n{out}");
    assert!(!lines[blank + 1..].is_empty(), "a table follows the empty line:\n{out}");
    // the table's first column is `time` (CLI-05 AC 2), then the top's own signals in
    // declaration order (CLI-05 AC 3): `vin`, `vout`, named by their full path (AC 5)
    assert_eq!(cells(lines[blank + 1]), ["time", "Step.vin", "Step.vout"], "{out}");
    // the time column starts at 0 and ends at the manifest's `stop = 5e-6`
    let rows: Vec<Vec<&str>> = lines[blank + 2..].iter().map(|l| cells(l)).collect();
    assert_eq!(rows[0][0], "0.000000e0", "{out}");
    assert_eq!(rows[rows.len() - 1][0], "5.000000e-6", "{out}");
    // the step response of the RC: tau = 1 kOhm * 1 nF = 1 us, so v(vout) = 1 - exp(-t/tau);
    // vin holds 1 V (CLI-05 AC 8's 1e-3 contract, here as an absolute bound on a 1 V swing)
    for row in &rows {
        assert_eq!(row[1], "1.000000e0", "vin of {row:?}");
        let (t, vout): (f64, f64) = (row[0].parse().expect("a time"), row[2].parse().expect("a value"));
        assert!((vout - (1.0 - (-t / 1e-6).exp())).abs() <= 1e-3, "vout at {t}: {vout}");
    }
}

#[test]
fn records_come_before_the_table() {
    // CLI-06 AC 7 and CLI-06 AC 1: every record line comes before the table; the RC's monitor
    // (`examples/rc/benches/step.msl`) emits `trip` with `v = v(a)` when the output crosses its
    // `level` 0.5, which a 1 kOhm / 1 nF step does at t = tau * ln 2
    // (`--all-checks`: a passing check is hidden on the terminal otherwise, CLI-06 AC 9)
    let out = stdout_of(&piperine(&["run", "step", "--all-checks"]));
    let lines: Vec<&str> = out.lines().collect();
    let table = lines.iter().position(|l| l.starts_with("time")).unwrap_or_else(|| panic!("no table in\n{out}"));
    let records: Vec<&str> = lines.iter().copied().filter(|l| l.starts_with('@')).collect();
    assert!(!records.is_empty(), "the crossing emits records:\n{out}");
    assert!(lines.iter().rposition(|l| l.starts_with('@')) < Some(table), "a record after the table:\n{out}");
    let trips: Vec<&&str> = records.iter().filter(|l| cells(l)[2] == "trip").collect();
    assert_eq!(trips.len(), 1, "one trip record:\n{out}");
    let trip = cells(trips[0]);
    let at: f64 = trip[0].trim_start_matches('@').parse().expect("a time");
    assert!(close(at, 1e-6 * std::f64::consts::LN_2, 1e-3), "the crossing time {at}");
    assert_eq!(trip[1], "Step.mon", "the instance path of the site");
    let v: f64 = field(trips[0], "v").parse().expect("a value");
    assert!((v - 0.5).abs() <= 1e-3, "v at the crossing: {v}");
    // CLI-06 AC 6: the `require` of the monitor passes, one `checks` record with pass=true
    let checks: Vec<&&str> = records.iter().filter(|l| cells(l)[2] == "checks").collect();
    assert_eq!(checks.len(), 1, "{out}");
    assert_eq!(field(checks[0], "pass"), "true");
    assert_eq!(field(checks[0], "name"), "\"crossed_upward\"");
    // each record is no later than the next (CLI-06 AC 3)
    let times: Vec<f64> = records.iter().map(|l| cells(l)[0].trim_start_matches('@').parse().expect("a time")).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]), "{times:?}");
}

#[test]
fn a_completed_run_exits_0() {
    // CLI-07 AC 3
    let out = piperine(&["run", "op"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    // the operating point of the same project: `signal` and `value`, one row per printed
    // signal (CLI-05 AC 1); the source holds `vin` at 1 V
    let lines: Vec<String> = text(&out.stdout).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "run op: op, top Step, backend rust");
    assert_eq!(lines[1], "");
    assert_eq!(cells(&lines[2]), ["signal", "value"]);
    assert_eq!(cells(&lines[3]), ["Step.vin", "1.000000e0"]);
}

#[test]
fn stdout_is_the_same_from_two_caches_and_two_directories() {
    // Book claims held here: cli~59441d1b
    // CLI-07 AC 6: no wall-clock time, no absolute path and no cache key on standard output, so
    // the bytes do not depend on the cache or the directory the command ran from
    let (cache_a, cache_b) = (tempfile::tempdir().expect("tmp"), tempfile::tempdir().expect("tmp"));
    let a = piperine_in(&rc(), cache_a.path(), &["run", "step"]);
    let b = piperine_in(&rc().join("benches"), cache_b.path(), &["run", "step"]);
    let (a, b) = (stdout_of(&a), stdout_of(&b));
    assert_eq!(a, b);
    for forbidden in [cache_a.path().to_string_lossy().into_owned(), cache_b.path().to_string_lossy().into_owned(), rc().to_string_lossy().into_owned(), "/tmp".to_string(), "/home".to_string()] {
        assert!(!a.contains(&forbidden), "`{forbidden}` on standard output:\n{a}");
    }
}

#[test]
fn a_first_compile_is_announced_on_stderr() {
    // CLI-07 AC 8: WHILE a design that is not cached compiles, standard error holds one line
    // saying that it compiles and which run; a cached one prints none. The compiler is a wrapper
    // of `rustc` that, at its first build (not the `-vV` probe), copies what `piperine` has
    // written on standard error so far: the line is there before `rustc` builds anything.
    let cache = tempfile::tempdir().expect("tmp");
    let err = cache.path().join("stderr.txt");
    let seen = cache.path().join("seen-by-rustc.txt");
    let wrapper = cache.path().join("rustc-wrapper.sh");
    let script = format!("#!/bin/sh\ncase \"$1\" in -vV) ;; *) [ -f '{seen}' ] || cp '{err}' '{seen}' ;; esac\nexec rustc \"$@\"\n", seen = seen.display(), err = err.display());
    std::fs::write(&wrapper, script).expect("the wrapper");
    #[cfg(unix)]
    std::fs::set_permissions(&wrapper, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("an executable wrapper");
    let store = cache.path().join("store");
    let run = |args: &[&str]| {
        let file = std::fs::File::create(&err).expect("a stderr file");
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(args).current_dir(rc()).env("PIPERINE_CACHE_DIR", &store).env("PIPERINE_RUSTC", &wrapper).env_remove("PIPERINE_BACKEND").stderr(file).output().expect("run piperine")
    };
    let first = run(&["run", "op"]);
    let stderr = std::fs::read_to_string(&err).expect("stderr");
    assert_eq!(first.status.code(), Some(0), "{stderr}");
    let announced: Vec<&str> = stderr.lines().filter(|l| l.contains("compiling")).collect();
    assert_eq!(announced, ["compiling run `op` (top Step) with rustc"], "one announcement naming the run:\n{stderr}");
    let before = std::fs::read_to_string(&seen).expect("the wrapper built something");
    assert!(before.contains("compiling run `op` (top Step) with rustc"), "the line is written before rustc builds; rustc saw:\n{before}");
    assert!(!text(&first.stdout).contains("compiling"), "standard output is data only");
    let second = run(&["run", "op"]);
    let stderr = std::fs::read_to_string(&err).expect("stderr");
    assert_eq!(second.status.code(), Some(0), "{stderr}");
    assert!(!stderr.contains("compiling"), "a cached design is not announced:\n{stderr}");
}

#[test]
fn the_project_flag_uses_that_directory_and_never_searches() {
    // CLI-02 AC 2: `--project <dir>` uses the package at `<dir>` and does not search upward.
    // From an unrelated directory the run of `examples/rc` is found by `--project` alone...
    let elsewhere = tempfile::tempdir().expect("tmp");
    let project = rc();
    let out = piperine_in(elsewhere.path(), CompileCache::shared().path(), &["run", "op", "--project", &project.to_string_lossy()]);
    assert_eq!(text(&out.stdout).lines().next(), Some("run op: op, top Step, backend rust"), "{}", text(&out.stderr));
    // ...and a directory of the package that holds no manifest is E0170 although `examples/rc`
    // is upward from it
    let benches = project.join("benches");
    let out = piperine_in(&project, CompileCache::shared().path(), &["run", "op", "--project", &benches.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("E0170"), "{}", text(&out.stderr));
    assert!(out.stdout.is_empty());
}

#[test]
fn a_lone_file_run_is_named_by_its_stem() {
    // CLI-07 AC 7 and CLI-02 AC 9: `piperine run <file>.msl` with no manifest upward is a crate
    // alone and its header names the file's stem, here `divider`; the flags make the request.
    // CLI-05 AC 7: the divider of `tests/e2e/twins/divider.msl` (10 V across 1k and 3k) gives
    // 7.5 for `Divider.mid`, the spec's independent test (`Divider.mid  7.500000e0`)
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/divider.msl");
    let nowhere = tempfile::tempdir().expect("tmp");
    let out = piperine_in(nowhere.path(), CompileCache::shared().path(), &["run", &file.to_string_lossy(), "--analysis", "op"]);
    let out = stdout_of(&out);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "run divider: op, top Divider, backend rust", "{out}");
    let mid = lines.iter().find(|l| l.starts_with("Divider.mid")).unwrap_or_else(|| panic!("no Divider.mid row in\n{out}"));
    assert_eq!(cells(mid), ["Divider.mid", "7.500000e0"], "{out}");
}

/// The fixture project `name` under `tests/projects/`.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects").join(name)
}

/// Writes `files` (relative path, text) under a new temporary directory and returns it.
fn written(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tmp");
    for (path, text) in files {
        let at = dir.path().join(path);
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("mkdir");
        std::fs::write(at, text).expect("write");
    }
    dir
}

/// A run that failed with `code`: nothing on standard output, a message on standard error.
fn failed(out: &Output, code: i32) -> String {
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(code), "stdout:\n{}\nstderr:\n{err}", text(&out.stdout));
    err
}

#[test]
fn no_manifest_is_e0170_and_exit_1() {
    // Book claims held here: cli~4142e137, cli~39a2cb1d
    // CLI-02 AC 3: no manifest upward, and the command needs a project: E0170, exit 1
    let nowhere = tempfile::tempdir().expect("tmp");
    let out = piperine_in(nowhere.path(), CompileCache::shared().path(), &["run", "step"]);
    let err = failed(&out, 1);
    assert!(err.contains("E0170"), "{err}");
    assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
}

#[test]
fn a_package_error_is_rendered_with_code_file_and_line() {
    // CLI-02 AC 6: the code, the file and the source line of a package error on standard error.
    // `missing_dependency/Piperine.toml` names a path no package is at, on its line 6.
    let out = piperine_in(&fixture("missing_dependency"), CompileCache::shared().path(), &["run", "main"]);
    let err = failed(&out, 1);
    assert!(err.contains("E0172"), "the code:\n{err}");
    assert!(err.contains("Piperine.toml:6:1"), "the file and the line:\n{err}");
    assert!(err.contains("parts = { path = \"../no_such_package\" }"), "the source line:\n{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn no_name_with_no_run_or_several_lists_them_and_exits_2() {
    // Book claims held here: cli~07125ec2
    // CLI-03 AC 10: no name and several runs (`two_runs` has `first` and `second`), or no run at
    // all: exit 2, the run names listed on standard error
    let several = piperine_in(&fixture("two_runs"), CompileCache::shared().path(), &["run"]);
    let err = failed(&several, 2);
    assert!(err.contains("first") && err.contains("second"), "{err}");
    assert!(several.stdout.is_empty());
    let none = written(&[("Piperine.toml", "[package]\nname = \"none\"\nversion = \"0.1.0\"\n"), ("src/lib.msl", "module Top() { signal a: Electrical; }\n")]);
    let out = piperine_in(none.path(), CompileCache::shared().path(), &["run"]);
    let err = failed(&out, 2);
    assert!(err.contains("no run"), "{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn an_unknown_run_name_lists_the_runs_and_exits_2() {
    // CLI-03 AC 11
    let out = piperine_in(&fixture("two_runs"), CompileCache::shared().path(), &["run", "third"]);
    let err = failed(&out, 2);
    assert!(err.contains("third"), "it names what was asked:\n{err}");
    assert!(err.contains("first") && err.contains("second"), "it lists the runs:\n{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn a_lone_file_without_analysis_exits_2() {
    // CLI-03 AC 12: a run asked from flags alone with no --analysis exits 2 naming the flag
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/divider.msl");
    let out = piperine_in(&rc(), CompileCache::shared().path(), &["run", &file.to_string_lossy()]);
    let err = failed(&out, 2);
    assert!(err.contains("--analysis"), "{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn a_single_valued_flag_given_twice_exits_2() {
    // CLI-01 AC 7 (a usage error is exit 2): `--stop` takes one value
    let out = piperine(&["run", "step", "--stop", "1e-6", "--stop", "2e-6"]);
    let err = failed(&out, 2);
    assert!(err.contains("--stop") && err.contains("Usage:"), "{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn a_type_error_is_rendered_with_its_stage_and_exits_1() {
    // CLI-04 AC 4: every diagnostic with its code, file, line and excerpt, the stage named,
    // exit 1. `type_error/src/lib.msl` puts a real where a `bool` is declared, at line 2.
    let out = piperine_in(&fixture("type_error"), CompileCache::shared().path(), &["run", "main"]);
    let err = failed(&out, 1);
    assert!(err.contains("E0180"), "the code:\n{err}");
    assert!(err.contains("lib.msl:2:"), "the file and the line:\n{err}");
    assert!(err.contains("flag: bool = 1.5;"), "the excerpt:\n{err}");
    assert!(err.contains("error: typeck failed with 1 diagnostic(s)"), "the stage:\n{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn a_bad_backend_exits_2_for_the_flag_and_1_otherwise() {
    // CLI-04 AC 8: a value that is no backend names the two valid ones; exit 2 for the flag, 1
    // for `PIPERINE_BACKEND` and for the run's `backend` key
    let flag = piperine(&["run", "op", "--backend", "spice"]);
    let err = failed(&flag, 2);
    assert!(err.contains("interp") && err.contains("rust"), "{err}");
    let variable = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine"))).args(["run", "op"]).current_dir(rc()).env("PIPERINE_CACHE_DIR", CompileCache::shared().path()).env("PIPERINE_BACKEND", "spice").output().expect("run");
    let err = failed(&variable, 1);
    assert!(err.contains("interp") && err.contains("rust"), "{err}");
    let key = written(&[
        ("Piperine.toml", "[package]\nname = \"keyed\"\nversion = \"0.1.0\"\n\n[run.main]\nanalysis = \"op\"\nbackend = \"spice\"\n"),
        ("src/lib.msl", "module Top() { signal a: Electrical; }\n"),
    ]);
    let out = piperine_in(key.path(), CompileCache::shared().path(), &["run", "main"]);
    let err = failed(&out, 1);
    assert!(err.contains("interp") && err.contains("rust"), "{err}");
    for out in [flag, variable, out] {
        assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
    }
}

#[test]
fn a_solver_failure_prints_no_table_and_exits_1() {
    // CLI-05 AC 9: the solver's error and the analysis on standard error, no table. Two ideal
    // sources of 1 V and 2 V across the same node (`two_sources`) have no operating point.
    let out = piperine_in(&fixture("two_sources"), CompileCache::shared().path(), &["run", "main"]);
    let err = failed(&out, 1);
    assert!(err.contains("op analysis failed"), "the analysis:\n{err}");
    let stdout = text(&out.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["run main: op, top Top, backend rust"], "the header and no table:\n{stdout}");
}

#[test]
fn a_failure_before_the_analysis_prints_nothing_on_stdout() {
    // Book claims held here: cli~57fa9eec
    // CLI-07 AC 4: any stage before the analysis fails: nothing on standard output, exit 1.
    // A parse-stage error is no different from a typeck one, a bad run table, a missing
    // dependency, a `print` that names nothing (E0801) or a missing project.
    let cases: [(&str, &[&str]); 4] = [
        ("type_error", &["run", "main"]),
        ("bad_run_table", &["run", "main"]),
        ("missing_dependency", &["run", "main"]),
        ("two_runs", &["run", "first", "--print", "nothing"]),
    ];
    for (project, args) in cases {
        let out = piperine_in(&fixture(project), CompileCache::shared().path(), args);
        let err = failed(&out, 1);
        assert!(out.stdout.is_empty(), "{project}: {}", text(&out.stdout));
        assert!(err.contains("error:"), "{project}: {err}");
    }
    let bad_table = piperine_in(&fixture("bad_run_table"), CompileCache::shared().path(), &["run", "main"]);
    let err = text(&bad_table.stderr);
    assert!(err.contains("P0010") && err.contains("Piperine.toml:7:"), "the run table's fault points at `stopp`:\n{err}");
}

#[test]
fn records_stay_when_the_analysis_fails() {
    // Book claims held here: cli~5037c701
    // CLI-07 AC 5: records already printed stay, the error goes to standard error, exit 1.
    // `fails_after_emit`: a node that starts at 0.1 V crosses the monitor's 0.5 V at 1.6 us
    // (0.1 * exp(t / 1 us)), emitting `trip`, then grows until the solver meets an infinity.
    let out = piperine_in(&fixture("fails_after_emit"), CompileCache::shared().path(), &["run", "grow"]);
    let err = failed(&out, 1);
    let stdout = text(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "run grow: transient, top Top, backend rust", "{stdout}");
    assert_eq!(lines.len(), 2, "the header and the one record, no table:\n{stdout}");
    let trip = cells(lines[1]);
    assert_eq!(&trip[1..3], ["Top.mon", "trip"], "{stdout}");
    let at: f64 = trip[0].trim_start_matches('@').parse().expect("a time");
    assert!(close(at, 1e-6 * 5f64.ln(), 1e-2), "the crossing time {at}");
    assert!(err.contains("transient analysis failed"), "{err}");
}

#[test]
fn a_closed_stdout_ends_with_exit_1_and_no_panic() {
    // design.md "Error Handling Strategy": a closed standard output ends the command
    // with exit 1 and no panic message. The read end of the pipe is closed before the child
    // writes its header.
    let mut child = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_piperine")))
        .args(["run", "step"])
        .current_dir(rc())
        .env("PIPERINE_CACHE_DIR", CompileCache::shared().path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn");
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait");
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(!err.contains("panicked"), "{err}");
    assert!(err.contains("standard output"), "{err}");
}

/// The Book's blinky: a clock and a counter, no analog node.
const BLINKY: &str = "module Osc(out clk: Bit = false) {\n    f: f64 = 1M where f > 0.0;\n}\ndigital for Osc {\n    on clk.changed() { clk <- !clk after 0.5 / f; }\n}\n\nmodule Counter(in clk: Bit, out count: Bit<u8> = 0) {}\ndigital for Counter {\n    on clk.rise() {\n        count <- count + 1;\n        emit!(\"edge\", t = now(), count = count);\n    }\n}\n\nmodule Blinky() {\n    signal clk: Bit;\n    pub signal count: Bit<u8>;\n    let osc = Osc(clk) { f: 1M };\n    let ctr = Counter(clk, count);\n}\n";

#[test]
fn a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot() {
    // HII-17 (L56; supersedes CLI-05 AC 11's refusal of this design): a design with no analog node
    // runs to its stop. Three `edge` records, then the table on the 1 us grid with the counter at
    // 0, 1, 2, 3, exit 0, for the table, `--csv` and `--plot` alike. A transient that genuinely
    // ends short has no source on this branch (nothing ends a run early before a `finish` ends a
    // transient), so CLI-05 AC 11 keeps its unit test in `piperine-api`'s recorder.
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("blinky.msl"), BLINKY).expect("blinky.msl");
    let base = ["run", "blinky.msl", "--analysis", "transient", "--stop", "3e-6", "--print-step", "1e-6", "--print", "count"];
    for extra in [&[][..], &["--csv", "out.csv"][..], &["--plot", "out.svg"][..]] {
        let args: Vec<&str> = base.iter().chain(extra).copied().collect();
        let out = piperine_in(dir.path(), CompileCache::shared().path(), &args);
        let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
        assert_eq!(out.status.code(), Some(0), "{args:?}: {stdout}\n{stderr}");
        assert_eq!(stdout.lines().filter(|l| l.starts_with('@')).count(), 3, "{args:?}: the records:\n{stdout}");
        match extra.first() {
            None => {
                let rows: Vec<Vec<&str>> = stdout.lines().skip_while(|l| !l.starts_with("time")).skip(1).map(|l| l.split_whitespace().collect()).collect();
                assert_eq!(rows, [["0.000000e0", "0"], ["1.000000e-6", "1"], ["2.000000e-6", "2"], ["3.000000e-6", "3"]], "{stdout}");
            }
            Some(&"--csv") => {
                let csv = std::fs::read_to_string(dir.path().join("out.csv")).expect("out.csv");
                let counts: Vec<&str> = csv.lines().skip(1).filter_map(|l| l.rsplit(',').next()).collect();
                assert_eq!(counts, ["0", "1", "2", "3"], "{csv}");
            }
            Some(_) => assert!(dir.path().join("out.svg").exists(), "the plot is written"),
        }
    }
}

#[test]
fn run_prints_the_fatal_line_and_exits_1() {
    // FIX-02 AC 2: `fatal("boom")` at 2 us in a run to 5 us: the table up to 2 us is printed,
    // `fatal at 2.000000e-6: boom` is the last line of standard error, and the exit code is 1
    let out = piperine_in(&fixture("fatal_run"), CompileCache::shared().path(), &["run", "fatal"]);
    let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
    assert_eq!(out.status.code(), Some(1), "{stdout}\n{stderr}");
    assert_eq!(stderr.lines().last(), Some("fatal at 2.000000e-6: boom"), "{stderr}");
    let rows: Vec<&str> = stdout.lines().skip_while(|l| !l.is_empty()).skip(2).collect();
    let last = cells(rows.last().expect("table rows"));
    assert!((last[0].parse::<f64>().expect("a time") - 2e-6).abs() <= 1e-12, "the table ends at the fatal instant: {stdout}");
}

#[test]
fn a_fatal_run_prints_its_msg_and_exits_1() {
    // HII-15 AC 1 (`std/io.msl`: `fatal` writes `sim` with `failed = true` and its `msg`): the
    // records already printed stay, the fatal line is the last of standard error, exit 1; the
    // same bench with `finish` instead is no failure and prints its table
    let design = |end: &str| format!("pub module Resistor(p: Electrical, n: Electrical) {{ r: f64 where r > 0.0; }}\nanalog for Resistor {{ i(p, n) +> v(p, n) / r; }}\npub module Vsource(p: Electrical, n: Electrical) {{ dc: f64 = 1.0; }}\nanalog for Vsource {{ v(p, n) +> dc; }}\n\nmodule M() {{}}\ndigital for M {{ on at(1u) {{ emit!(\"note\", n = 1); {end} }} }}\n\nmodule Top() {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n    let m = M();\n}}\n");
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("brown.msl"), design("fatal(\"brownout\");")).expect("brown.msl");
    let args = ["run", "brown.msl", "--analysis", "transient", "--stop", "2e-6", "--top", "Top"];
    let out = piperine_in(dir.path(), CompileCache::shared().path(), &args);
    let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
    assert_eq!(out.status.code(), Some(1), "{stdout}\n{stderr}");
    assert_eq!(stderr.lines().last(), Some("fatal at 1.000000e-6: brownout"), "the fatal line is the last of standard error:\n{stderr}");
    assert!(stdout.lines().any(|l| l.contains("note")), "the records already printed stay:\n{stdout}");

    std::fs::write(dir.path().join("fine.msl"), design("finish();")).expect("fine.msl");
    let out = piperine_in(dir.path(), CompileCache::shared().path(), &["run", "fine.msl", "--analysis", "transient", "--stop", "2e-6", "--top", "Top"]);
    assert_eq!(out.status.code(), Some(0), "finish is no failure: {}\n{}", text(&out.stdout), text(&out.stderr));
}

#[test]
fn a_guarded_record_prints_when_its_guard_holds_and_no_record_carries_a_maybe() {
    // US-05 AC 1, 2 (explicit-unknowns T29): `d` starts false and is never driven, so `if d { emit!(..) }` does
    // not run and writes no record; the `emit!` outside the guard is a plain record: no line prints `maybe`
    let design = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\nmodule M(in d: Bit) {}\ndigital for M { on at(1u) { if d { emit!(\"unsure\", n = 1); } emit!(\"sure\", n = 1); } }\n\nmodule Top() {\n    signal a: Electrical; signal d: Bit = false;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n    let m = M(d);\n}\n";
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("maybe.msl"), design).expect("maybe.msl");
    let args = ["run", "maybe.msl", "--analysis", "transient", "--stop", "2e-6", "--top", "Top"];
    let out = piperine_in(dir.path(), CompileCache::shared().path(), &args);
    let stdout = stdout_of(&out);
    let line = |stream: &str| stdout.lines().find(|l| l.contains(&format!(" {stream} "))).unwrap_or_else(|| panic!("no `{stream}` record in:\n{stdout}")).to_string();
    assert!(!stdout.contains(" unsure "), "the guard is false, so the record is not written:\n{stdout}");
    assert!(line("sure").ends_with(" n=1") && !stdout.contains(" maybe="), "{}", line("sure"));
}

#[test]
fn run_of_a_top_with_a_node_port_is_e0810() {
    // PI-07 AC 5, X8, §23.1 rule 1: no manifest key drives a port, so `piperine run` of a top
    // with a node port has no host source for it: E0810 naming the port, exit 1, and no table on
    // standard output (the P0001 "continuous port" of the elaborator is gone)
    let files = written(&[("bench.msl", "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n\nmodule Top(vin: Electrical) {\n    let r1 = Resistor(vin, gnd) { r: 1k };\n}\n")]);
    let file = files.path().join("bench.msl");
    let out = piperine_in(files.path(), CompileCache::shared().path(), &["run", &file.to_string_lossy(), "--analysis", "op", "--top", "Top"]);
    let err = failed(&out, 1);
    assert!(err.contains("E0810"), "{err}");
    assert!(err.contains("`vin`"), "the port is named:\n{err}");
    assert!(!err.contains("P0001"), "{err}");
    assert_eq!(text(&out.stdout).trim_end(), "run bench: op, top Top, backend rust", "only the header, no table");
}

/// A design whose carriers hold a unit enum, a struct, an enum with a payload and an array, committed
/// at each rising edge of a clock, and a record that writes the same values.
const COMPOSITES: &str = "#[derive(Repr, PartialEq)]\nenum Mode { Idle, Regulate }\n#[derive(Repr, PartialEq)]\nstruct Pair { a: u8, b: bool }\n#[derive(Repr, PartialEq)]\nenum Tx { Idle, Data { n: u3 } }\n\
pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\
module Logger(in clk: Bit, out mode: Bit<Mode> = Mode::Idle, out pair: Bit<Pair> = Pair { a: 0, b: false }, out tx: Bit<Tx> = Tx::Idle, out v: Bit<[u8; 2]> = [0, 0]) {}\n\
digital for Logger {\n    on clk.rise() {\n        mode <- Mode::Regulate;\n        pair <- Pair { a: 7, b: true };\n        tx <- Tx::Data { n: 5 };\n        v <- [3, 4];\n        emit!(\"sample\", pair = Pair { a: 7, b: true }, tx = Tx::Data { n: 5 }, arr = [1u8, 2]);\n    }\n}\n\
module Top() {\n    signal clk: Bit;\n    signal mode: Bit<Mode>;\n    signal pair: Bit<Pair>;\n    signal tx: Bit<Tx>;\n    signal v: Bit<[u8; 2]>;\n    let osc = Osc(clk) { f: 200M };\n    let logger = Logger(clk, mode, pair, tx, v);\n}\n";

/// The files `piperine run` wrote for `COMPOSITES` and what it printed.
struct CompositeRun {
    /// Standard output.
    stdout: String,
    /// The directory holding the written files.
    dir: tempfile::TempDir,
}

impl CompositeRun {
    /// `piperine run bench.msl` printing `print`; with `files`, writing a CSV, JSON Lines and a plot
    /// instead of printing the table and the records.
    fn of(print: &[&str], files: bool) -> CompositeRun {
        let dir = written(&[("bench.msl", COMPOSITES)]);
        let mut args: Vec<String> = ["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9"].iter().map(|s| s.to_string()).collect();
        for signal in print {
            args.extend(["--print".to_string(), signal.to_string()]);
        }
        if files {
            args.extend(["--csv", "out.csv", "--records", "out.jsonl", "--plot", "out.svg"].iter().map(|s| s.to_string()));
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let stdout = stdout_of(&piperine_in(dir.path(), CompileCache::shared().path(), &refs));
        CompositeRun { stdout, dir }
    }

    /// A file the run wrote.
    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(name)).unwrap_or_else(|e| panic!("no {name}: {e}"))
    }
}

#[test]
fn composites_print_in_each_output_format() {
    // CV-21 AC 1, CV-22 AC 1 (Q2 (c), D13): the text table, the record lines and JSON Lines write a
    // composite in literal syntax (JSON nested, an enum `{"Variant": {..}}`), and an enum with no
    // payload as its variant's name; CSV and the plot take one column per dotted leaf
    let run = CompositeRun::of(&["mode", "pair", "tx", "v"], false);
    let last = run.stdout.lines().rev().find(|l| l.contains("Regulate")).unwrap_or_else(|| panic!("no final row in\n{}", run.stdout));
    assert!(last.contains("Pair { a: 7, b: true }") && last.contains("Tx::Data { n: 5 }") && last.contains("[3, 4]"), "the text table: {last}");
    let record = run.stdout.lines().find(|l| l.contains(" sample ")).unwrap_or_else(|| panic!("no record in\n{}", run.stdout));
    assert!(record.contains("pair=Pair { a: 7, b: true }") && record.contains("tx=Tx::Data { n: 5 }") && record.contains("arr=[1, 2]"), "the record line: {record}");
    let run = CompositeRun::of(&["mode", "pair", "tx", "v"], true);
    let json = run.file("out.jsonl");
    let line = json.lines().next().unwrap_or_else(|| panic!("no JSON line"));
    assert!(line.contains("\"pair\":{\"a\":7,\"b\":true}") && line.contains("\"tx\":{\"Data\":{\"n\":5}}") && line.contains("\"arr\":[1,2]"), "the JSON line: {line}");
    let csv = run.file("out.csv");
    let mut rows = csv.lines();
    assert_eq!(rows.next(), Some("time,Top.mode,Top.pair.a,Top.pair.b,Top.tx,Top.tx.Data.n,Top.v[0],Top.v[1]"), "one CSV column per leaf: {csv}");
    let final_row = rows.last().expect("a CSV row");
    assert!(final_row.ends_with(",Regulate,7,true,Data,5,3,4"), "the leaves of the last row: {final_row}");
    let svg = run.file("out.svg");
    for series in ["Top.pair.a", "Top.tx.Data.n", "Top.v[1]"] {
        assert!(svg.contains(&format!("data-signal=\"{series}\"")), "the plot has `{series}`: {svg}");
    }
}

/// A design with an array of carriers `v` whose element 1 takes 4 at the first rising edge, and an array of nodes `bl`.
const ARRAYS: &str = "pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\
module Memory(in clk: Bit) { signal v: [Bit<u8>; 3] = [7; 3]; signal n: Bit<u8> = 0; }\n\
digital for Memory { on clk.rise() { emit!(\"sample\", v = v, one = v[1]); n <- n + 1; if n == 0 { v[1] <- 4; } } }\n\
module Ladder(feed: Electrical) { signal bl: [Electrical; 2]; }\n\
analog for Ladder { v(bl[0], gnd) +> 2.0; i(bl[0], bl[1]) +> v(bl[0], bl[1]) / 1000.0; i(bl[1], gnd) +> v(bl[1], gnd) / 1000.0; }\n\
module Top() { signal clk: Bit; let osc = Osc(clk) { f: 200M }; let memory = Memory(clk); let ladder = Ladder(gnd); }\n";

/// `piperine run` over `ARRAYS`.
struct ArrayRun;

impl ArrayRun {
    /// The output of printing `print`, with `files` writing a CSV and a plot instead of printing only the table.
    fn of(print: &[&str], files: bool) -> (String, tempfile::TempDir) {
        let dir = written(&[("bench.msl", ARRAYS)]);
        let mut args: Vec<String> = ["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9"].iter().map(|s| s.to_string()).collect();
        for signal in print {
            args.extend(["--print".to_string(), signal.to_string()]);
        }
        if files {
            args.extend(["--csv", "out.csv", "--plot", "out.svg", "--records", "out.jsonl"].iter().map(|s| s.to_string()));
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let stdout = stdout_of(&piperine_in(dir.path(), CompileCache::shared().path(), &refs));
        (stdout, dir)
    }
}

#[test]
fn an_array_of_carriers_prints_by_element_and_whole_in_every_format() {
    // CV-28 AC 1, AC 2, CV-21 (Q2 (c)): `Top.memory.v[1]` prints that element alone, `Top.memory.v` the whole array in literal
    // syntax in the text table, and CSV and the plot take one column per element; an array of nodes is one column per node
    let (stdout, _) = ArrayRun::of(&["Top.memory.v[1]", "Top.memory.v"], false);
    let header = stdout.lines().find(|l| l.starts_with("time")).unwrap_or_else(|| panic!("no table in\n{stdout}"));
    assert_eq!(cells(header)[1], "Top.memory.v[1]", "{stdout}");
    let last = stdout.lines().rev().find(|l| !l.is_empty()).expect("a row");
    assert_eq!(cells(last)[1], "4", "the element committed at the first edge: {last}");
    assert!(last.contains("[7, 4, 7]"), "the whole array in literal syntax: {last}");
    let (_, dir) = ArrayRun::of(&["Top.memory.v", "Top.ladder.bl"], true);
    let csv = std::fs::read_to_string(dir.path().join("out.csv")).expect("out.csv");
    let mut rows = csv.lines();
    assert_eq!(rows.next(), Some("time,Top.memory.v[0],Top.memory.v[1],Top.memory.v[2],Top.ladder.bl[0],Top.ladder.bl[1]"), "one CSV column per element and per node: {csv}");
    let final_row = rows.last().expect("a CSV row");
    assert!(final_row.contains(",7,4,7,"), "the elements of the last row: {final_row}");
    // the record lines of `emit!("sample", v = v, one = v[1])`: the whole array in literal syntax and the element as a number, at the
    // first edge (before its commit) and at the second (after it); JSON Lines holds the array as an array
    let (stdout, _) = ArrayRun::of(&["Top.memory.v"], false);
    let records: Vec<&str> = stdout.lines().filter(|l| l.contains(" sample ")).collect();
    assert_eq!(records.len(), 2, "one record per rising edge in 10 ns: {stdout}");
    assert!(records[0].contains("v=[7, 7, 7]") && records[0].contains("one=7"), "the first edge reads the initial elements: {}", records[0]);
    assert!(records[1].contains("v=[7, 4, 7]") && records[1].contains("one=4"), "the second reads the committed element: {}", records[1]);
    let json = std::fs::read_to_string(dir.path().join("out.jsonl")).expect("out.jsonl");
    let lines: Vec<&str> = json.lines().collect();
    assert_eq!(lines.len(), 2, "one JSON line per record: {json}");
    assert!(lines[0].contains("\"v\":[7,7,7]") && lines[0].contains("\"one\":7"), "{}", lines[0]);
    assert!(lines[1].contains("\"v\":[7,4,7]") && lines[1].contains("\"one\":4"), "{}", lines[1]);
    let svg = std::fs::read_to_string(dir.path().join("out.svg")).expect("out.svg");
    for series in ["Top.memory.v[1]", "Top.ladder.bl[1]"] {
        assert!(svg.contains(&format!("data-signal=\"{series}\"")), "the plot has `{series}`: {svg}");
    }
}

/// A design whose carrier `o` holds an `Option<u8>` that starts at `None` and takes `Some(7)` at the
/// first rising edge of a clock.
const OPTIONS: &str = "pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\
module Logger(in clk: Bit, out o: Bit<Option<u8>>) {}\n\
digital for Logger {\n    on clk.rise() {\n        emit!(\"seen\", o = o);\n        o <- Some(7);\n        emit!(\"set\", o = Some(7));\n    }\n}\n\
module Top() {\n    signal clk: Bit;\n    signal o: Bit<Option<u8>>;\n    let osc = Osc(clk) { f: 200M };\n    let logger = Logger(clk, o);\n}\n";

#[test]
fn an_option_cell_prints_none_or_the_value() {
    // EU-15 AC 5 (explicit-unknowns T39): an absent `Option` is written `None`, a present one `Some(x)`,
    // and no format has an unknown cell: the text table and CSV (one column per leaf, `some` and `value`);
    // the record line and JSON Lines (`None` as `None` and `null`, `Some` as its payload) are held by an `emit!`
    // field of type `Option` (§19 rule 11): the record before the commit is `None`, after it `Some(7)`
    let dir = written(&[("bench.msl", OPTIONS)]);
    let args = ["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9", "--print", "o"];
    let stdout = stdout_of(&piperine_in(dir.path(), CompileCache::shared().path(), &args));
    let table: Vec<&str> = stdout.lines().skip_while(|l| !l.starts_with("time")).collect();
    assert!(table.get(1).is_some_and(|row| row.ends_with("None")), "the first row holds the initial `None`: {table:?}");
    assert!(table.iter().rev().find(|row| !row.is_empty()).is_some_and(|row| row.ends_with("Some(7)")), "the last row holds the committed value: {table:?}");
    assert!(!table.iter().any(|row| row.split_whitespace().any(|cell| cell == "x")), "no unknown cell: {stdout}");
    assert!(stdout.lines().any(|l| l.contains("Top.logger seen o=None")), "the record of an absent `Option` field: {stdout}");
    assert!(stdout.lines().any(|l| l.contains("Top.logger set o=Some(7)")), "the record of a present one is `Some`: {stdout}");
    piperine_in(dir.path(), CompileCache::shared().path(), &["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9", "--print", "o", "--csv", "out.csv"]);
    let csv = std::fs::read_to_string(dir.path().join("out.csv")).expect("out.csv");
    let mut rows = csv.lines();
    assert_eq!(rows.next(), Some("time,Top.o.some,Top.o.value"), "one CSV column per leaf: {csv}");
    assert_eq!(rows.next().map(|row| row.split(',').skip(1).collect::<Vec<_>>()), Some(vec!["false", ""]), "an absent Option has no `value` leaf: {csv}");
    assert!(csv.lines().last().is_some_and(|row| row.ends_with(",true,7")), "the last row is `Some(7)`: {csv}");
}

#[test]
fn a_leaf_path_prints_that_leaf_alone() {
    // CV-21 AC 2: a printed path that names a field or an element prints that leaf and nothing else
    let run = CompositeRun::of(&["Top.pair.a", "Top.tx.Data.n", "Top.v[1]"], false);
    let header = run.stdout.lines().find(|l| l.starts_with("time")).unwrap_or_else(|| panic!("no table in\n{}", run.stdout));
    assert_eq!(cells(header), ["time", "Top.pair.a", "Top.tx.Data.n", "Top.v[1]"], "{}", run.stdout);
    let last = run.stdout.lines().rev().find(|l| !l.is_empty()).expect("a row");
    assert_eq!(&cells(last)[1..], ["7", "5", "4"], "each leaf alone: {last}");
    let table = run.stdout.split("\n\n").nth(1).expect("a table after the records");
    assert!(!table.contains("Pair {") && !table.contains("Tx::"), "no whole value in the table: {table}");
}

/// A lone bench: 1 V behind 1 kOhm into a load of `n` mS, `n` an integer param of 4 by default.
const COUNTED: &str = "pub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\
    pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n\
    module Load(p: Electrical) { n: u8 = 4; }\nanalog for Load { i(p, gnd) +> (n as f64) * v(p) / 1k; }\n\
    module Top() { signal src: Electrical; signal a: Electrical; let v1 = Vsource(src, gnd); let r1 = Resistor(src, a) { r: 1k }; let l = Load(a); }\n";

#[test]
fn param_of_an_integer_from_the_command_line_reads_from_the_start() {
    // PI-01 AC 5 (D24): `--param l.n=6` is the load of the run from its operating point, v(a) = 1 / (1 + n); the default
    // n = 4 gives 0.2 V, and the literal `0x6` and `6` are the same value.
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("counted.msl");
    std::fs::write(&file, COUNTED).expect("write");
    let file = file.to_str().expect("utf-8 path");
    let volts = |extra: &[&str]| {
        let args: Vec<&str> = ["run", file, "--analysis", "op", "--print", "a"].iter().chain(extra).copied().collect();
        let out = stdout_of(&piperine_in(dir.path(), CompileCache::shared().path(), &args));
        let row = out.lines().find(|l| l.starts_with("Top.a")).unwrap_or_else(|| panic!("no row of `a` in\n{out}")).to_string();
        cells(&row)[1].parse::<f64>().expect("a number")
    };
    assert!(close(volts(&[]), 1.0 / 5.0, 1e-6), "the default n = 4");
    assert!(close(volts(&["--param", "l.n=6"]), 1.0 / 7.0, 1e-6), "n = 6 from the start");
    assert!(close(volts(&["--param", "l.n=0x6"]), 1.0 / 7.0, 1e-6), "the literal `0x6` is 6");
    // PI-01 AC 4, D24: what is no integer literal of the type and range is `E0802` naming the param and the value, a number included
    for bad in ["300", "6.5", "1e1", "-1"] {
        let entry = format!("l.n={bad}");
        let refused = piperine_in(dir.path(), CompileCache::shared().path(), &["run", file, "--analysis", "op", "--param", &entry]);
        assert_ne!(refused.status.code(), Some(0), "`{bad}` is no `u8`");
        assert!(text(&refused.stderr).contains("E0802") && text(&refused.stderr).contains("l.n") && text(&refused.stderr).contains(bad), "{bad}: {}", text(&refused.stderr));
    }
}

/// A design whose carrier `l` holds a `Logic<4>` that starts at `xxxx` and takes `10xz` at the first rising edge of a clock,
/// and a record that writes two four-state values.
const LOGICS: &str = "pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\
module Logger(in clk: Bit, out l: Bit<Logic<4>>) {}\n\
digital for Logger {\n    on clk.rise() {\n        l <- logic(\"10xz\");\n        emit!(\"sample\", w = logic(\"10xz\"), n = Logic::from(0b0110u4));\n    }\n}\n\
module Top() {\n    signal clk: Bit;\n    signal l: Bit<Logic<4>>;\n    let osc = Osc(clk) { f: 200M };\n    let logger = Logger(clk, l);\n}\n";

#[test]
fn a_logic_cell_prints_its_pattern() {
    // EU-15 AC 4 (explicit-unknowns T47, C63): a `Logic<4>` cell is its four characters of `0`, `1`, `x`, `z`, highest level
    // first, in the text table (`xxxx` before the commit, `10xz` after), the record line and JSON Lines, and CSV
    let dir = written(&[("bench.msl", LOGICS)]);
    let args = ["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9", "--print", "l"];
    let stdout = stdout_of(&piperine_in(dir.path(), CompileCache::shared().path(), &args));
    let table: Vec<&str> = stdout.lines().skip_while(|l| !l.starts_with("time")).collect();
    assert!(table.get(1).is_some_and(|row| row.ends_with("xxxx")), "the first row holds the initial `xxxx`: {table:?}");
    assert!(table.iter().rev().find(|row| !row.is_empty()).is_some_and(|row| row.ends_with("10xz")), "the last row holds the committed pattern: {table:?}");
    let record = stdout.lines().find(|l| l.contains(" sample ")).unwrap_or_else(|| panic!("no record in\n{stdout}"));
    assert!(record.contains("w=10xz") && record.contains("n=0110"), "the record line: {record}");
    piperine_in(dir.path(), CompileCache::shared().path(), &["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9", "--print", "l", "--csv", "out.csv", "--records", "out.jsonl"]);
    let json = std::fs::read_to_string(dir.path().join("out.jsonl")).expect("out.jsonl");
    let line = json.lines().next().unwrap_or_else(|| panic!("no JSON line"));
    assert!(line.contains("\"w\":\"10xz\"") && line.contains("\"n\":\"0110\""), "the JSON line: {line}");
    let csv = std::fs::read_to_string(dir.path().join("out.csv")).expect("out.csv");
    assert!(csv.lines().last().is_some_and(|row| row.ends_with(",10xz")), "the last CSV row: {csv}");
}

/// A design whose carriers hold an `f32`, an `f16`, an `f128` and a `UInt<200>` committed at the first rising edge, a record of
/// the four and of the two params `k` (`f128`) and `m` (`UInt<200>`).
const WIDTHS: &str = "pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\
module Logger(in clk: Bit, out s: Bit<f32> = 0.0, out h: Bit<f16> = 0.0, out q: Bit<f128> = 0.0, out w: Bit<UInt<200>> = 0) { k: f128 = 0.5; m: UInt<200> = 7; }\n\
digital for Logger {\n    on clk.rise() {\n        s <- 0.1;\n        h <- 0.1;\n        q <- 0.1;\n        let big: UInt<200> = (1 << 199u8) + 1;\n        w <- big;\n        emit!(\"sample\", s = 0.1f32, k = k, m = m, w = big);\n    }\n}\n\
module Top() {\n    signal clk: Bit;\n    signal s: Bit<f32>;\n    signal h: Bit<f16>;\n    signal q: Bit<f128>;\n    signal w: Bit<UInt<200>>;\n    let osc = Osc(clk) { f: 200M };\n    let logger = Logger(clk, s, h, q, w);\n}\n";

/// `2^199 + 1`, the value the carrier `w` holds.
const TWO_TO_199_PLUS_1: &str = "803469022129495137770981046170581301261101496891396417650689";

/// `piperine run` over `WIDTHS`: the files it wrote and what it printed.
struct WidthRun;

impl WidthRun {
    /// The run printing `print`, with `files` writing a CSV, JSON Lines and a plot instead of printing, and `extra` flags.
    fn of(print: &[&str], files: bool, extra: &[&str]) -> CompositeRun {
        let dir = written(&[("bench.msl", WIDTHS)]);
        let mut args: Vec<String> = ["run", "bench.msl", "--analysis", "transient", "--stop", "1e-8", "--max-step", "1e-9"].iter().map(|s| s.to_string()).collect();
        for signal in print {
            args.extend(["--print".to_string(), signal.to_string()]);
        }
        if files {
            args.extend(["--csv", "out.csv", "--records", "out.jsonl", "--plot", "out.svg"].iter().map(|s| s.to_string()));
        }
        args.extend(extra.iter().map(|s| s.to_string()));
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let stdout = stdout_of(&piperine_in(dir.path(), CompileCache::shared().path(), &refs));
        CompositeRun { stdout, dir }
    }
}

#[test]
fn number_widths_print_exactly_in_every_format() {
    // CV-32 AC 4, CV-21 (Q2 (c), QD6): a carried and an emitted `f32`, `f16`, `f128` and `UInt<200>` are written as the shortest
    // decimal that reads back in their format (`0.1` in each) and the exact integer, in the text table, the record lines, JSON
    // Lines (a number each) and CSV; the plot takes the nearest `f64`; `--param` sets an `f128` and a `UInt<200>` param in
    // their own format
    let run = WidthRun::of(&["s", "h", "q", "w"], false, &[]);
    let last = run.stdout.lines().rev().find(|l| l.contains(TWO_TO_199_PLUS_1)).unwrap_or_else(|| panic!("no final row in\n{}", run.stdout));
    assert_eq!(cells(last)[1..], ["0.1", "0.1", "0.1", TWO_TO_199_PLUS_1], "the text table: {last}");
    let record = run.stdout.lines().find(|l| l.contains(" sample ")).unwrap_or_else(|| panic!("no record in\n{}", run.stdout));
    assert!(record.contains("s=0.1 k=0.5 m=7 w=803469022129495137770981046170581301261101496891396417650689"), "the record line: {record}");

    let run = WidthRun::of(&["s", "h", "q", "w"], true, &["--param", "logger.k=0.3", "--param", "logger.m=0x8000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0001"]);
    let json = run.file("out.jsonl");
    let line = json.lines().next().unwrap_or_else(|| panic!("no JSON line"));
    assert!(line.contains("\"s\":0.1,\"k\":0.3,\"m\":3138550867693340381917894711603833208051177722232017256449,\"w\":803469022129495137770981046170581301261101496891396417650689"), "the JSON line (the params as `--param` set them, in their own format): {line}");
    let csv = run.file("out.csv");
    let mut rows = csv.lines();
    assert_eq!(rows.next(), Some("time,Top.s,Top.h,Top.q,Top.w"), "{csv}");
    assert!(rows.last().expect("a CSV row").ends_with(&format!(",0.1,0.1,0.1,{TWO_TO_199_PLUS_1}")), "the last CSV row: {csv}");
    let svg = run.file("out.svg");
    for series in ["Top.s", "Top.h", "Top.q", "Top.w"] {
        assert!(svg.contains(&format!("data-signal=\"{series}\"")), "the plot has `{series}`: {svg}");
    }
}
