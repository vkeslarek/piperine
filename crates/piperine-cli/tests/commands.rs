//! The project commands of the binary (feature `msl-cli`, phase 5; CLI-14 to CLI-18): `check`,
//! `build`, `test`, `new` and `clean`, each run as the real `piperine` child process on
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
    // CLI-14 AC 3: every diagnostic of every run, no `ok` line, exit 1. A type error in the
    // bench of a copy of `rc` is seen by both of its runs (`op` and `step` share it) and
    // reported with its code, file and line; the two runs share the source, so it is reported
    // once.
    let (_tmp, dir) = copy_of(&rc(), "rc");
    let bench = dir.join("benches/step.msl");
    let source = std::fs::read_to_string(&bench).expect("the bench");
    std::fs::write(&bench, source.replace("level: f64 = 0.5;", "level: bool = 0.5;")).expect("a write");
    let out = piperine_in(&dir, &["check"]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert_eq!(text(&out.stdout), "", "no ok line");
    assert!(err.contains("P0002") && err.contains("step.msl:4"), "{err}");
    assert_eq!(err.matches("error: typeck failed").count(), 1, "one report for two runs:\n{err}");
    // the broken fixture's three runs print three reports, none dropped
    let out = piperine_in(&fixture("broken_benches"), &["check"]);
    assert_eq!(text(&out.stdout), "");
    assert_eq!(text(&out.stderr).matches("\nerror: ").count() + usize::from(text(&out.stderr).starts_with("error: ")), 3, "{}", text(&out.stderr));
}

#[test]
fn check_without_a_run_says_type_checking_needs_one() {
    // CLI-14 AC 4: a project with no run is parsed and resolved only, and the line says so
    let out = piperine_in(&fixture("no_run"), &["check"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "ok: no_run (2 crates, 0 runs; type checking needs a run)\n");
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
