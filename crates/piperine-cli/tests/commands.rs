//! The project commands of the binary (feature `msl-cli`, phases 5 and 6; CLI-14 to CLI-18 and
//! CLI-24): `check`, `build`, `test`, `new`, `clean` and `fmt`, each run as the real `piperine`
//! child process on
//! projects on disk (`examples/rc` and the fixtures under `tests/projects/`). A project that a
//! test changes is copied to a temporary directory first.
//!
//! Every child gets `PIPERINE_CACHE_DIR` set (a shared temporary directory, or one of the
//! test's own); no test sets a variable of its own process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

/// The binary cargo built for this target.
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_piperine"))
}

/// `examples/rc`.
fn rc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc")
}

/// A fixture project under `tests/projects/`.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects").join(name)
}

/// One cache directory for every child that does not ask for its own.
fn shared_cache() -> &'static Path {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE.get_or_init(|| tempfile::tempdir().expect("tmp")).path()
}

/// `piperine <args>` in `dir`, on the cache `cache`, with a compiler path `rustc` when given.
fn piperine_with(dir: &Path, cache: &Path, rustc: Option<&str>, args: &[&str]) -> Output {
    let mut command = Command::new(binary());
    command.args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", cache).env_remove("PIPERINE_BACKEND").env_remove("PIPERINE_RUSTC");
    if let Some(rustc) = rustc {
        command.env("PIPERINE_RUSTC", rustc);
    }
    command.output().unwrap_or_else(|e| panic!("cannot run {}: {e}", binary().display()))
}

fn piperine_in(dir: &Path, args: &[&str]) -> Output {
    piperine_with(dir, shared_cache(), None, args)
}

/// `piperine <args>` in `dir` with `PIPERINE_BACKEND` set to `backend`.
fn piperine_on(dir: &Path, backend: &str, args: &[&str]) -> Output {
    Command::new(binary()).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", shared_cache()).env("PIPERINE_BACKEND", backend).env_remove("PIPERINE_RUSTC").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", binary().display()))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Copies the directory `from` into `to` (a project is plain files).
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for entry in std::fs::read_dir(from).expect("a readable directory") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a copy");
        }
    }
}

/// A temporary copy of `source`, named `name` inside its own temporary directory.
fn copy_of(source: &Path, name: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join(name);
    copy_tree(source, &dir);
    (tmp, dir)
}

// ---- check (CLI-14) -------------------------------------------------------------------------

#[test]
fn check_runs_every_stage_and_no_backend() {
    // CLI-14 AC 1: parse, type-check and elaborate every run, invoking no backend. The three
    // runs of `broken_benches` fail in three different stages, and all three are reported:
    // a syntax error (E0900, parse), a type error (P0002, typeck) and a `where` clause that is
    // false for a param (E0317, elaborate). The stage lines name each.
    let out = piperine_in(&fixture("broken_benches"), &["check"]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    for needle in ["E0900", "syntax.msl:1", "error: parse failed", "P0002", "types.msl:2", "error: typeck failed", "E0317", "values.msl:6", "error: elaborate failed"] {
        assert!(err.contains(needle), "`{needle}` missing from\n{err}");
    }
    // no backend: a compiler that does not exist and an empty cache, and the good project
    // still checks; the cache stays empty because no design was compiled
    let cache = tempfile::tempdir().expect("tmp");
    let out = piperine_with(&rc(), cache.path(), Some("/no/such/rustc"), &["check"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_dir(cache.path()).expect("a directory").count(), 0, "check wrote into the cache");
}

#[test]
fn check_prints_ok_with_the_counts() {
    // CLI-14 AC 2: `ok: <package> (<c> crates, <r> runs)` on standard output, exit 0, nothing
    // on standard error. `rc` is two crates (itself and the embedded `std`) with two runs; `with_dependency` is
    // three (itself, `parts` and `std`) with one run.
    let out = piperine_in(&rc(), &["check"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "ok: rc (2 crates, 2 runs)\n");
    assert_eq!(text(&out.stderr), "");
    let out = piperine_in(&fixture("with_dependency"), &["check"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "ok: with_dependency (3 crates, 1 runs)\n");
}

#[test]
fn check_renders_every_diagnostic_and_exits_1() {
    // Book claims held here: errors~a1afa480, structure~58f974e3
    // CLI-14 AC 3: every diagnostic of every run, no `ok` line, exit 1. A type error in the
    // bench of a copy of `rc` is seen by both of its runs (`op` and `step` share it) and
    // reported with its code, file and line; the two runs share the source, so it is reported
    // once.
    let (_tmp, dir) = copy_of(&rc(), "rc");
    let bench = dir.join("benches/step.msl");
    let source = std::fs::read_to_string(&bench).expect("the bench");
    std::fs::write(&bench, source.replace("level: f64 = 0.5;", "level: bool = 0.5;")).expect("a write");
    // the line of `level` in the bench (line 5 since `examples/rc` is committed formatted, T49)
    let line = source[..source.find("level: f64 = 0.5;").expect("the bench declares `level`")].lines().count();
    let out = piperine_in(&dir, &["check"]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert_eq!(text(&out.stdout), "", "no ok line");
    assert!(err.contains("P0002") && err.contains(&format!("step.msl:{line}:")), "{err}");
    assert_eq!(err.matches("error: typeck failed").count(), 1, "one report for two runs:\n{err}");
    // the broken fixture's three runs print three reports, none dropped
    let out = piperine_in(&fixture("broken_benches"), &["check"]);
    assert_eq!(text(&out.stdout), "");
    assert_eq!(text(&out.stderr).matches("\nerror: ").count() + usize::from(text(&out.stderr).starts_with("error: ")), 3, "{}", text(&out.stderr));
}

#[test]
fn check_without_a_run_types_the_whole_package() {
    // GN-08 AC 2 (S3; CLI-14 AC 4 moved): a project with no run is type-checked whole, so the line
    // no longer says type checking needs a run
    let out = piperine_in(&fixture("no_run"), &["check"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "ok: no_run (2 crates, 0 runs)\n");
}

#[test]
fn check_takes_a_lone_file() {
    // CLI-14 AC 5: `check <file>.msl` checks that file's crate with the top rule of a session
    // (the one module without ports is the top); a type error in it is exit 1 with P0002
    let tmp = tempfile::tempdir().expect("tmp");
    let good = tmp.path().join("lone.msl");
    std::fs::copy(fixture("two_runs").join("src/lib.msl"), &good).expect("a copy");
    let out = piperine_in(tmp.path(), &["check", "lone.msl"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "ok: lone (2 crates, 0 runs)\n");
    let bad = tmp.path().join("bad.msl");
    std::fs::write(&bad, "module Top() {\n    flag: bool = 1.5;\n}\n").expect("a write");
    let out = piperine_in(tmp.path(), &["check", "bad.msl"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
    assert!(text(&out.stderr).contains("P0002") && text(&out.stderr).contains("bad.msl:2"), "{}", text(&out.stderr));
}

// ---- build (CLI-15) -------------------------------------------------------------------------

#[test]
fn build_prints_one_line_per_run() {
    // Book claims held here: cli~00319f47
    // CLI-15 AC 1: one line per run, in manifest order, `built <name>: compiled | cached |
    // interp, nothing to compile`. On an empty cache the first Rust run of `rc` compiles; the
    // second run has the same design (both name the bench's `Step`), so it is found cached.
    let cache = tempfile::tempdir().expect("tmp");
    let out = piperine_with(&rc(), cache.path(), None, &["build"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "built op: compiled\nbuilt step: cached\n");
    // the interpreter has nothing to compile
    let out = piperine_on(&fixture("two_runs"), "interp", &["build"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "built first: interp, nothing to compile\nbuilt second: interp, nothing to compile\n");
    // a run named on the command line is the only one built
    let out = piperine_on(&fixture("two_runs"), "interp", &["build", "second"]);
    assert_eq!(text(&out.stdout), "built second: interp, nothing to compile\n");
    // a name that is no run is a usage error
    let out = piperine_on(&fixture("two_runs"), "interp", &["build", "third"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
}

#[test]
fn a_second_build_is_cached_and_compiles_nothing() {
    // CLI-15 AC 2: the second build of an unchanged project says `cached` for every Rust run and
    // invokes the compiler for no design. The compiler is a wrapper that logs every argument
    // list and runs the real `rustc`; the first build logs at least one compile (an invocation
    // that makes a library), the second logs none (the version probe that keys the cache is
    // not a design build).
    let cache = tempfile::tempdir().expect("tmp");
    let tools = tempfile::tempdir().expect("tmp");
    let log = tools.path().join("calls.log");
    let wrapper = tools.path().join("rustc-wrapper");
    std::fs::write(&wrapper, format!("#!/bin/sh\necho \"$*\" >> {}\nexec rustc \"$@\"\n", log.display())).expect("a wrapper");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let compiles = || std::fs::read_to_string(&log).unwrap_or_default().lines().filter(|l| l.contains("--crate-type")).count();
    let wrapper = wrapper.to_str().expect("a path");
    let first = piperine_with(&rc(), cache.path(), Some(wrapper), &["build"]);
    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));
    assert!(text(&first.stdout).contains("compiled"), "{}", text(&first.stdout));
    let after_first = compiles();
    assert!(after_first >= 1, "the first build compiled nothing: {}", std::fs::read_to_string(&log).unwrap_or_default());
    let second = piperine_with(&rc(), cache.path(), Some(wrapper), &["build"]);
    assert_eq!(second.status.code(), Some(0), "{}", text(&second.stderr));
    assert_eq!(text(&second.stdout), "built op: cached\nbuilt step: cached\n");
    assert_eq!(compiles(), after_first, "the second build invoked the compiler on a design");
}

#[test]
fn verbose_prints_a_source_path_that_exists() {
    // CLI-15 AC 3: after each compiled or cached line, a line with the path of the generated
    // Rust source, and the file exists (the cached line too); an interpreted run has none
    let cache = tempfile::tempdir().expect("tmp");
    let out = piperine_with(&rc(), cache.path(), None, &["build", "--verbose"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "{stdout}");
    assert_eq!(lines[0], "built op: compiled");
    assert_eq!(lines[2], "built step: cached");
    for path in [lines[1], lines[3]] {
        assert!(Path::new(path).is_file(), "`{path}` is not a file");
        assert!(std::fs::read_to_string(path).expect("source").contains("fn "), "`{path}` holds no Rust");
    }
    let out = piperine_on(&fixture("two_runs"), "interp", &["build", "--verbose"]);
    assert_eq!(text(&out.stdout), "built first: interp, nothing to compile\nbuilt second: interp, nothing to compile\n");
}

#[test]
fn build_goes_on_after_a_failing_run_and_exits_1() {
    // CLI-15 AC 4: the middle run fails its type check; its error is rendered on standard
    // error, the runs before and after it are still built, and the exit code is 1
    let out = piperine_on(&fixture("one_good_one_bad"), "interp", &["build"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "built good: interp, nothing to compile\nbuilt also_good: interp, nothing to compile\n");
    let err = text(&out.stderr);
    assert!(err.contains("P0002") && err.contains("bad.msl:2") && err.contains("error: typeck failed"), "{err}");
}

// ---- test (CLI-08, CLI-16) ------------------------------------------------------------------

/// The lines of a standard output.
fn lines_of(out: &Output) -> Vec<String> {
    text(&out.stdout).lines().map(str::to_string).collect()
}

#[test]
fn test_runs_every_run_or_the_named_ones() {
    // Book claims held here: digital~4830dc9c
    // CLI-16 AC 1: every run in manifest order (`good`, `bad`), or only the runs named, in the
    // order named; a name that is no run is a usage error
    let dir = fixture("one_pass_one_fail");
    let all = lines_of(&piperine_in(&dir, &["test"]));
    assert!(all[0].starts_with("PASS good ") && all[1].starts_with("FAIL bad "), "{all:?}");
    let named = lines_of(&piperine_in(&dir, &["test", "bad", "good"]));
    assert!(named[0].starts_with("FAIL bad ") && named.iter().any(|l| l.starts_with("PASS good ")), "{named:?}");
    assert_eq!(named.last().map(String::as_str), Some("2 runs, 1 passed, 1 failed"));
    let one = lines_of(&piperine_in(&dir, &["test", "good"]));
    assert_eq!(one, ["PASS good (1 checks)", "1 runs, 1 passed, 0 failed"]);
    let out = piperine_in(&dir, &["test", "nope"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
}

#[test]
fn test_prints_pass_with_the_check_count() {
    // CLI-16 AC 2: `PASS <name> (<n> checks)`. In `examples/rc` the `op` run writes no check and
    // `step` writes the one `require` of its monitor. The count includes passing checks the
    // terminal hides: `run step` prints no `checks` line by default (CLI-06 AC 9) and `test`
    // still counts it, so the hiding changes neither the count nor the verdict.
    let out = piperine_in(&rc(), &["test"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "PASS op (0 checks)\nPASS step (1 checks)\n2 runs, 2 passed, 0 failed\n");
    let hidden = text(&piperine_in(&rc(), &["run", "step"]).stdout);
    assert!(!hidden.contains(" checks "), "a passing check is hidden on the terminal:\n{hidden}");
    let shown = text(&piperine_in(&rc(), &["run", "step", "--all-checks"]).stdout);
    assert_eq!(shown.lines().filter(|l| l.contains(" checks ")).count(), 1, "{shown}");
}

#[test]
fn test_prints_fail_and_the_failing_records() {
    // Book claims held here: cli~9ed0890c
    // CLI-16 AC 3: `FAIL <name> (<k> of <n> checks failed)` then the line of each failing
    // record, and exit 1; the monitor requires `v <= 0.1` at the 0.5 V crossing, which is false
    let out = piperine_in(&fixture("failing_require"), &["test"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    let lines = lines_of(&out);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0], "FAIL fails (1 of 1 checks failed)");
    assert!(lines[1].starts_with('@') && lines[1].ends_with(" Fails.mon checks kind=\"require\" name=\"under_limit\" pass=false"), "{}", lines[1]);
    assert_eq!(lines[2], "1 runs, 0 passed, 1 failed");
    // of two checks' worth of a run, only the failing record is listed: `bad` of the mixed project
    let mixed = lines_of(&piperine_in(&fixture("one_pass_one_fail"), &["test", "bad"]));
    assert_eq!(mixed[0], "FAIL bad (1 of 1 checks failed)");
    assert!(mixed[1].contains("pass=false"), "{mixed:?}");
}

#[test]
fn test_prints_no_table_and_no_other_record() {
    // Book claims held here: cli~3c91ac79
    // CLI-16 AC 4: `rc`'s `step` also writes a `trip` record and a table of `time`, `vin` and
    // `vout`; none reaches standard output, only the verdict and the count
    let out = piperine_in(&rc(), &["test", "step"]);
    assert_eq!(text(&out.stdout), "PASS step (1 checks)\n1 runs, 1 passed, 0 failed\n");
    // a failing run lists its failing check and nothing else either
    let out = text(&piperine_in(&fixture("failing_require"), &["test"]).stdout);
    assert!(!out.contains("time") && !out.contains(" sim ") && out.matches('@').count() == 1, "{out}");
}

#[test]
fn test_prints_fail_fatal_with_the_msg() {
    // Book claims held here: cli~906cefdd
    // HII-15 AC 2: a run whose bench calls `fatal("brownout")` is `FAIL <run> (fatal at <t>: <msg>)` and
    // exit 1; the other run of the project still runs and passes
    let out = piperine_in(&fixture("fatal_and_target"), &["test", "brown"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "FAIL brown (fatal at 1.000000e-6: brownout)\n1 runs, 0 passed, 1 failed\n");
}

#[test]
fn test_prints_target_margins_after_the_run_line() {
    // Book claims held here: cli~3bb4a4a0, cli~f6eae41a
    // HII-16 AC 1 to 3 (decision X3): `margins` writes `gain` twice (0.5, then -0.25) and `power`
    // twice (2, then 1.5); the last of each is printed in the order of the first, after the
    // PASS line, and the negative margin does not fail the run
    let out = piperine_in(&fixture("fatal_and_target"), &["test", "margins"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "PASS margins (4 checks)\n  target gain margin=-2.500000e-1\n  target power margin=1.500000e0\n1 runs, 1 passed, 0 failed\n"
    );
}

#[test]
fn a_run_that_does_not_complete_is_a_fail() {
    // CLI-16 AC 5: `FAIL <name> (did not run)` on standard output, the error on standard error.
    // `fails_after_emit` meets an infinity in its solve; `type_error` fails before any solve.
    let out = piperine_in(&fixture("fails_after_emit"), &["test"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "FAIL grow (did not run)\n1 runs, 0 passed, 1 failed\n");
    assert!(text(&out.stderr).contains("contributed a NaN"), "{}", text(&out.stderr));
    let out = piperine_in(&fixture("type_error"), &["test"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "FAIL main (did not run)\n1 runs, 0 passed, 1 failed\n");
    assert!(text(&out.stderr).contains("P0002"), "{}", text(&out.stderr));
}

#[test]
fn a_dc_run_of_a_design_with_records_is_refused_never_passed() {
    // CLI-09 AC 10: a DC sweep point has no time to stamp a record with, so a `dc` run of a
    // design that holds a record stream is refused with `P0010` before anything compiles, and
    // `test` counts it a failure. `dc_require`'s monitor requires `v(a) <= 0.1` at time 0 over a
    // divider at 0.5 V: the `op` run shows the check fails, so a PASS of `sweep` would hide it.
    let op = piperine_in(&fixture("dc_require"), &["test", "op"]);
    assert_eq!(lines_of(&op)[0], "FAIL op (1 of 1 checks failed)", "{}", text(&op.stderr));
    let out = piperine_in(&fixture("dc_require"), &["test", "sweep"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "FAIL sweep (did not run)\n1 runs, 0 passed, 1 failed\n");
    let err = text(&out.stderr);
    assert!(err.contains("P0010") && err.contains("`dc`") && err.contains("Divider.mon.checks"), "{err}");
    // `run` refuses it the same way: nothing on standard output, exit 1
    let out = piperine_in(&fixture("dc_require"), &["run", "sweep"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "");
    assert!(text(&out.stderr).contains("P0010"), "{}", text(&out.stderr));
}

#[test]
fn the_last_line_counts_and_the_exit_code_follows() {
    // CLI-16 AC 6: `<r> runs, <p> passed, <f> failed` is the last line; exit 0 when `f` is 0, 1
    // otherwise
    let ok = piperine_in(&rc(), &["test"]);
    assert_eq!(lines_of(&ok).last().map(String::as_str), Some("2 runs, 2 passed, 0 failed"));
    assert_eq!(ok.status.code(), Some(0));
    let mixed = piperine_in(&fixture("one_pass_one_fail"), &["test"]);
    assert_eq!(lines_of(&mixed).last().map(String::as_str), Some("2 runs, 1 passed, 1 failed"));
    assert_eq!(mixed.status.code(), Some(1));
}

#[test]
fn a_project_without_a_run_passes_with_zero() {
    // CLI-16 AC 7
    let out = piperine_in(&fixture("no_run"), &["test"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "0 runs, 0 passed, 0 failed\n");
}

// ---- new (CLI-17) ---------------------------------------------------------------------------

/// The files of a project, by path under `dir`, as (path, text), sorted.
fn files_under(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, at: &Path, found: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(at).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                found.push((path.strip_prefix(root).expect("under root").to_string_lossy().into_owned(), std::fs::read_to_string(&path).expect("text")));
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

#[test]
fn new_creates_the_four_files_and_says_created() {
    // Book claims held here: cli~bc9ef575
    // CLI-17 AC 1: the directory `<name>` holds `Piperine.toml`, `src/lib.msl`,
    // `benches/step.msl` and `.gitignore`, and `created <name>` is printed
    let tmp = tempfile::tempdir().expect("tmp");
    let out = piperine_in(tmp.path(), &["new", "demo"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "created demo\n");
    let paths: Vec<String> = files_under(&tmp.path().join("demo")).into_iter().map(|(p, _)| p).collect();
    assert_eq!(paths, [".gitignore", "Piperine.toml", "benches/step.msl", "src/lib.msl"]);
}

#[test]
fn new_writes_the_rc_example_under_the_new_name() {
    // CLI-17 AC 2: the files of `examples/rc` with the package name replaced by `<name>`
    // (the manifest's `name` and the bench's `use`); every other byte is the example's, and a
    // project named `rc` is the example itself
    let tmp = tempfile::tempdir().expect("tmp");
    assert_eq!(piperine_in(tmp.path(), &["new", "filter"]).status.code(), Some(0));
    let want: Vec<(String, String)> = files_under(&rc())
        .into_iter()
        .map(|(path, text)| {
            let text = text.replace("name = \"rc\"", "name = \"filter\"").replace("use rc::", "use filter::");
            (path, text)
        })
        .collect();
    assert_eq!(files_under(&tmp.path().join("filter")), want);
    assert_eq!(piperine_in(tmp.path(), &["new", "rc"]).status.code(), Some(0));
    assert_eq!(files_under(&tmp.path().join("rc")), files_under(&rc()));
}

#[test]
fn new_refuses_an_existing_directory() {
    // CLI-17 AC 3: nothing is written and the exit code is 1; what was there is untouched
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join("demo");
    std::fs::create_dir(&dir).expect("a directory");
    std::fs::write(dir.join("mine.txt"), "keep").expect("a file");
    let out = piperine_in(tmp.path(), &["new", "demo"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "");
    assert!(text(&out.stderr).contains("`demo` already exists"), "{}", text(&out.stderr));
    assert_eq!(files_under(&dir), [("mine.txt".to_string(), "keep".to_string())]);
}

#[test]
fn new_refuses_a_name_the_manifest_rule_forbids() {
    // CLI-17 AC 4: a hyphen, a leading digit, a keyword and `std` are no package names; each
    // writes nothing, exits 1 and states the rule
    let tmp = tempfile::tempdir().expect("tmp");
    for name in ["my-filter", "1filter", "module", "std", "a/b"] {
        let out = piperine_in(tmp.path(), &["new", name]);
        assert_eq!(out.status.code(), Some(1), "{name}: {}", text(&out.stderr));
        assert_eq!(text(&out.stdout), "", "{name}");
        assert!(text(&out.stderr).contains(&format!("`{name}` is not a package name")) && text(&out.stderr).contains("one identifier"), "{name}: {}", text(&out.stderr));
    }
    assert_eq!(std::fs::read_dir(tmp.path()).expect("a directory").count(), 0, "a refused name wrote something");
}

#[test]
fn a_new_project_checks_runs_and_tests() {
    // CLI-17 AC 5: in a project `new` created, `check`, `run step` and `test` each exit 0
    let tmp = tempfile::tempdir().expect("tmp");
    assert_eq!(piperine_in(tmp.path(), &["new", "demo"]).status.code(), Some(0));
    let dir = tmp.path().join("demo");
    for args in [&["check"][..], &["run", "step"], &["test"]] {
        let out = piperine_in(&dir, args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out.stderr));
    }
    assert_eq!(text(&piperine_in(&dir, &["check"]).stdout), "ok: demo (2 crates, 2 runs)\n");
}

#[test]
fn new_without_a_name_exits_2() {
    // CLI-01 AC 7: `new` takes one required name; none is a usage error
    let tmp = tempfile::tempdir().expect("tmp");
    let out = piperine_in(tmp.path(), &["new"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_dir(tmp.path()).expect("a directory").count(), 0);
}

// ---- clean (CLI-18) -------------------------------------------------------------------------
// Every `clean` here works inside a temporary directory: the project is a copy, and the cache
// is a directory of the test's own named by `PIPERINE_CACHE_DIR`.

/// A copy of `rc` in a temporary directory, with a `target/` tree and a separate cache
/// directory holding a file; returns the temporary directory, the project and the cache.
fn project_with_target() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let (tmp, dir) = copy_of(&rc(), "rc");
    let cache = tmp.path().join("cache");
    std::fs::create_dir_all(dir.join("target/debug")).expect("a target");
    std::fs::write(dir.join("target/debug/out.bin"), "built").expect("a file");
    // the transpiler's layout: an entry `<cache>/rust/<key>/`
    std::fs::create_dir_all(cache.join(ENTRY)).expect("a cache");
    std::fs::write(cache.join(ENTRY).join("design.so"), "compiled").expect("a file");
    (tmp, dir, cache)
}

/// An entry of the cache, under the cache directory, as the transpiler lays it out.
const ENTRY: &str = "rust/0123456789abcdef0123456789abcdef";

#[test]
fn clean_removes_the_target_directory() {
    // Book claims held here: cli~8ba7a6de
    // CLI-18 AC 1: `<project>/target` is removed and `removed target` is printed; the cache,
    // which `--cache` was not given for, is still there
    let (_tmp, dir, cache) = project_with_target();
    let out = piperine_with(&dir, &cache, None, &["clean"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "removed target\n");
    assert!(!dir.join("target").exists());
    assert!(cache.join(ENTRY).join("design.so").is_file(), "the cache is removed only with --cache");
    // run from a subdirectory: the nearest `Piperine.toml` upward is the project
    std::fs::create_dir_all(dir.join("target")).expect("a target");
    let out = piperine_with(&dir.join("src"), &cache, None, &["clean"]);
    assert_eq!(text(&out.stdout), "removed target\n");
    assert!(!dir.join("target").exists());
}

#[test]
fn clean_cache_removes_the_compiled_cache() {
    // CLI-18 AC 2: `--cache` also removes the cache directory the settings name and says so on
    // standard error. The Independent Test: build, `clean --cache`, build prints `compiled`
    // again (a cached build prints `cached`).
    let (_tmp, dir, cache) = project_with_target();
    let out = piperine_with(&dir, &cache, None, &["clean", "--cache"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "removed target\n");
    assert!(text(&out.stderr).contains(&cache.display().to_string()), "{}", text(&out.stderr));
    assert!(!cache.exists() && !dir.join("target").exists());
    let first = piperine_with(&dir, &cache, None, &["build", "step"]);
    assert_eq!(text(&first.stdout), "built step: compiled\n", "{}", text(&first.stderr));
    let again = piperine_with(&dir, &cache, None, &["build", "step"]);
    assert_eq!(text(&again.stdout), "built step: cached\n");
    assert_eq!(piperine_with(&dir, &cache, None, &["clean", "--cache"]).status.code(), Some(0));
    let third = piperine_with(&dir, &cache, None, &["build", "step"]);
    assert_eq!(text(&third.stdout), "built step: compiled\n", "{}", text(&third.stderr));
}

#[test]
fn clean_with_nothing_to_remove_exits_0() {
    // CLI-18 AC 3: no `target`, no cache directory: `nothing to remove` and exit 0, twice
    let (tmp, dir) = copy_of(&rc(), "rc");
    let cache = tmp.path().join("no_cache_here");
    for _ in 0..2 {
        let out = piperine_with(&dir, &cache, None, &["clean", "--cache"]);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        assert_eq!(text(&out.stdout), "nothing to remove\n");
        assert_eq!(text(&out.stderr), "");
    }
}

#[test]
fn clean_touches_nothing_else() {
    // CLI-18 AC 4: with `--cache` the project's sources and manifest, a file next to `target`,
    // a sibling of the cache directory and a file next to the project are all still there
    let (tmp, dir, cache) = project_with_target();
    std::fs::write(dir.join("target_notes.txt"), "mine").expect("a file");
    std::fs::write(tmp.path().join("cache_sibling.txt"), "mine").expect("a file");
    std::fs::create_dir_all(tmp.path().join("other")).expect("a directory");
    std::fs::write(tmp.path().join("other/keep.txt"), "mine").expect("a file");
    let before: Vec<(String, String)> = files_under(tmp.path()).into_iter().filter(|(p, _)| !p.starts_with("rc/target/") && !p.starts_with("cache/")).collect();
    let out = piperine_with(&dir, &cache, None, &["clean", "--cache"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(files_under(tmp.path()), before);
}

#[test]
fn clean_cache_refuses_a_directory_it_does_not_own() {
    // CLI-18 AC 5 (verification F3): `PIPERINE_CACHE_DIR=. piperine clean --cache` in a project
    // removed the project. A cache directory that is or holds the project, the working directory
    // or the home directory, or that holds no `rust/` of the transpiler, is refused: nothing is
    // removed, exit 1, the reason on standard error. Every case runs in a temporary copy.
    let (tmp, dir, cache) = project_with_target();
    let everything = files_under(tmp.path());
    let refused = |out: &Output, why: &str| {
        assert_eq!(out.status.code(), Some(1), "{why}: {}", text(&out.stderr));
        assert_eq!(text(&out.stdout), "", "{why}");
        assert!(text(&out.stderr).contains("refused"), "{why}: {}", text(&out.stderr));
    };
    // the verifier's repro: the cache is the project itself, given relative
    refused(&piperine_with(&dir, Path::new("."), None, &["clean", "--cache"]), "the project");
    // the cache holds the project (its parent), and the cache is the working directory
    refused(&piperine_with(&dir, tmp.path(), None, &["clean", "--cache"]), "a parent of the project");
    refused(&piperine_with(&dir.join("src"), &dir.join("src"), None, &["clean", "--cache"]), "the working directory");
    // the cache is the home directory
    let home = Command::new(binary()).args(["clean", "--cache"]).current_dir(&dir).env("HOME", &cache).env("PIPERINE_CACHE_DIR", &cache).env_remove("PIPERINE_BACKEND").output().expect("run piperine");
    refused(&home, "the home directory");
    // a directory with no `rust/` of the transpiler is no cache of compiled designs
    let foreign = tmp.path().join("foreign");
    std::fs::create_dir_all(&foreign).expect("a directory");
    std::fs::write(foreign.join("notes.txt"), "mine").expect("a file");
    refused(&piperine_with(&dir, &foreign, None, &["clean", "--cache"]), "no rust/");
    let mut expected = everything.clone();
    expected.push(("foreign/notes.txt".into(), "mine".into()));
    expected.sort();
    assert_eq!(files_under(tmp.path()), expected, "a refused clean removes nothing, `target` included");
    // inside a real cache only what the layout writes goes: a file of the user's stays
    std::fs::write(cache.join("mine.txt"), "mine").expect("a file");
    std::fs::write(cache.join("rust/mine.txt"), "mine").expect("a file");
    let out = piperine_with(&dir, &cache, None, &["clean", "--cache"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!cache.join(ENTRY).exists(), "the entry is removed");
    assert!(cache.join("mine.txt").is_file() && cache.join("rust/mine.txt").is_file(), "a file the layout does not write stays");
}

// ---- fmt (CLI-24) ---------------------------------------------------------------------------
// Every `fmt` here works on a copy of `examples/rc` (committed formatted, CLI-24 AC 6) in a
// temporary directory; no test formats a file of the repository in place.

/// `examples/rc/src/lib.msl` with its first member re-spaced and a trailing comment after it,
/// and the text the formatter gives for it: the member in the printer's layout and the comment
/// after it, one space before it (CLI-23 AC 5).
fn unformatted_lib() -> (String, String) {
    let lib = std::fs::read_to_string(rc().join("src/lib.msl")).expect("the rc library");
    let member = "    r: f64 where r > 0.0;";
    assert!(lib.contains(member), "examples/rc is formatted:\n{lib}");
    (lib.replace(member, "  r:f64   where r>0.0;     // why r > 0"), lib.replace(member, "    r: f64 where r > 0.0; // why r > 0"))
}

/// `examples/rc/benches/step.msl` with its last member re-spaced and three blank lines and an
/// own-line comment after it, and the formatted text: the member in the printer's layout, one
/// blank line, and the comment at the body's indentation before the closing brace (CLI-23 AC 6
/// and 8).
fn unformatted_step() -> (String, String) {
    let step = std::fs::read_to_string(rc().join("benches/step.msl")).expect("the rc bench");
    let member = "    let mon = Monitor(vout);\n";
    assert!(step.contains(member), "examples/rc is formatted:\n{step}");
    (
        step.replace(member, "        let mon = Monitor( vout );\n\n\n\n// the monitor is last\n"),
        step.replace(member, "    let mon = Monitor(vout);\n\n    // the monitor is last\n"),
    )
}

/// A copy of `examples/rc` with both sources unformatted and a third, formatted source;
/// returns the temporary directory, the project and the two formatted texts expected.
fn unformatted_rc() -> (tempfile::TempDir, PathBuf, String, String) {
    let (tmp, dir) = copy_of(&rc(), "rc");
    let ((lib, lib_formatted), (step, step_formatted)) = (unformatted_lib(), unformatted_step());
    std::fs::write(dir.join("src/lib.msl"), lib).expect("a file");
    std::fs::write(dir.join("benches/step.msl"), step).expect("a file");
    std::fs::write(dir.join("src/parts.msl"), "/// A part with nothing in it.\npub module Part() {}\n").expect("a file");
    (tmp, dir, lib_formatted, step_formatted)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn fmt_rewrites_what_differs_and_names_it() {
    // Book claims held here: cli~e088bcbe
    // CLI-24 AC 1: every `.msl` under `src/` and `benches/` that differs is rewritten in place
    // and named by its project-relative path; a formatted one is not touched
    let (_tmp, dir, lib, step) = unformatted_rc();
    let parts = dir.join("src/parts.msl");
    let parts_time = std::fs::metadata(&parts).and_then(|m| m.modified()).expect("a modification time");
    let out = piperine_in(&dir, &["fmt"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "formatted src/lib.msl\nformatted benches/step.msl\n");
    assert_eq!(read(&dir.join("src/lib.msl")), lib);
    assert_eq!(read(&dir.join("benches/step.msl")), step);
    assert!(read(&dir.join("src/lib.msl")).contains("; // why r > 0\n"), "the comment is read back");
    assert_eq!(std::fs::metadata(&parts).and_then(|m| m.modified()).expect("a modification time"), parts_time, "the formatted file is not rewritten");
    // the temporary file of the rename does not stay behind
    let names: Vec<String> = files_under(&dir).into_iter().map(|(p, _)| p).filter(|p| p.contains(".tmp")).collect();
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn fmt_takes_files() {
    // CLI-24 AC 2: the files named, and only those
    let (_tmp, dir, _, step) = unformatted_rc();
    let lib_before = read(&dir.join("src/lib.msl"));
    let out = piperine_in(&dir, &["fmt", "benches/step.msl"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "formatted benches/step.msl\n");
    assert_eq!(read(&dir.join("benches/step.msl")), step);
    assert_eq!(read(&dir.join("src/lib.msl")), lib_before);
}

#[test]
fn fmt_check_writes_nothing_and_exits_1_on_a_difference() {
    // Book claims held here: cli~41e380a0
    // CLI-24 AC 3: `would format <path>` for each file that differs, no byte changed, exit 1
    let (_tmp, dir, _, _) = unformatted_rc();
    let before = files_under(&dir);
    let out = piperine_in(&dir, &["fmt", "--check"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "would format src/lib.msl\nwould format benches/step.msl\n");
    assert_eq!(files_under(&dir), before);
}

#[test]
fn fmt_skips_a_file_that_does_not_parse_and_exits_1() {
    // CLI-24 AC 4: the broken file's diagnostics on standard error, the file untouched, the
    // others formatted, exit 1
    let (_tmp, dir, lib, _) = unformatted_rc();
    let broken = format!("{}module Broken( {{\n", read(&dir.join("benches/step.msl")));
    std::fs::write(dir.join("benches/step.msl"), &broken).expect("a file");
    let out = piperine_in(&dir, &["fmt"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(read(&dir.join("benches/step.msl")), broken, "the broken file is byte-equal");
    assert_eq!(read(&dir.join("src/lib.msl")), lib);
    assert_eq!(text(&out.stdout), "formatted src/lib.msl\n");
    let err = text(&out.stderr);
    assert!(err.contains("benches/step.msl:") && err.contains("module Broken( {"), "the diagnostic with its file and excerpt:\n{err}");
    assert!(err.contains("error: benches/step.msl is left as it is"), "{err}");
}

#[test]
fn fmt_on_formatted_files_is_silent() {
    // CLI-24 AC 5: a second `fmt` finds every file formatted: nothing printed, exit 0
    let (_tmp, dir, _, _) = unformatted_rc();
    assert_eq!(piperine_in(&dir, &["fmt"]).status.code(), Some(0));
    let before = files_under(&dir);
    let out = piperine_in(&dir, &["fmt"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!((text(&out.stdout), text(&out.stderr)), (String::new(), String::new()));
    assert_eq!(files_under(&dir), before);
}

#[test]
fn fmt_check_passes_on_the_rc_example_and_on_a_new_project() {
    // CLI-24 AC 6: `examples/rc` (a byte-equal copy) and a project `new` created are formatted
    let (tmp, dir) = copy_of(&rc(), "rc");
    assert_eq!(files_under(&dir), files_under(&rc()));
    let out = piperine_in(&dir, &["fmt", "--check"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
    assert_eq!(piperine_in(tmp.path(), &["new", "demo"]).status.code(), Some(0));
    let out = piperine_in(&tmp.path().join("demo"), &["fmt", "--check"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
}

#[test]
fn test_prints_fail_with_the_fatal_message() {
    // FIX-02 AC 3: a run that called `fatal("boom")` at 2 us is `FAIL <name>` with the record's
    // message and time, and the exit code is 1
    let out = piperine_in(&fixture("fatal_run"), &["test"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "FAIL fatal (fatal at 2.000000e-6: boom)\n1 runs, 0 passed, 1 failed\n");
}

#[test]
fn test_passes_a_run_ended_by_finish() {
    // FIX-02 AC 4: `finish()` at 2 us after one passing check is `PASS <name> (1 checks)`
    let out = piperine_in(&fixture("finish_run"), &["test"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "PASS finish (1 checks)\n1 runs, 1 passed, 0 failed\n");
}
