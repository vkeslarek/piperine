//! Locating and running `rustc` (feature `ir-rust`, RS-03 AC 1, 2, 4; checks C14, C15, C17): a
//! compiler that is not there is named and the interpreter is offered; one below 1.85 is refused
//! with both versions; `rustc` runs directly and no `cargo` is ever called. The compilers here
//! are shell wrappers written into a temp dir.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use piperine_transpile::{Error, Rustc, Settings};

/// Writing an executable and exec'ing it races with another thread's `fork` holding the write
/// descriptor (ETXTBSY); the tests here take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Associated test helpers of `rustc`, grouped on one fixture type.
struct RustcFixture;

impl RustcFixture {
    fn script(path: &Path, body: &str) {
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("write script");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    fn settings(rustc: &Path) -> Settings {
        Settings { rustc: Some(rustc.to_path_buf()), cache: PathBuf::from("unused") }
    }
}

// RS-03 AC 2: no compiler -> an error that names `rustc` as missing and says the interpreter
// needs none.
#[test]
fn a_missing_compiler_is_named_and_the_interpreter_is_offered() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-such-rustc");
    let err = Rustc::locate(&RustcFixture::settings(&missing)).expect_err("no compiler there");
    assert!(matches!(err, Error::NoCompiler { .. }), "{err:?}");
    let text = err.to_string();
    assert!(text.contains("rustc"), "{text}");
    assert!(text.contains("no-such-rustc"), "names where it looked: {text}");
    assert!(text.contains("interpreter backend needs no compiler"), "{text}");
}

// RS-03 AC 4: below 1.85 -> an error naming the version found and the version required.
#[test]
fn a_compiler_below_1_85_is_refused_with_both_versions() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("rustc");
    RustcFixture::script(&old, "printf 'rustc 1.70.0 (90c541806 2023-05-31)\\nbinary: rustc\\nrelease: 1.70.0\\n'");
    let err = Rustc::locate(&RustcFixture::settings(&old)).expect_err("1.70 is too old");
    assert_eq!(err, Error::CompilerTooOld { found: "1.70.0".into(), required: "1.85" });
    let text = err.to_string();
    assert!(text.contains("1.70.0") && text.contains("1.85"), "{text}");

    // the edge: 1.85 itself, and a newer pre-release, are accepted
    RustcFixture::script(&old, "printf 'rustc 1.85.0\\nrelease: 1.85.0\\n'");
    assert!(Rustc::locate(&RustcFixture::settings(&old)).is_ok());
    RustcFixture::script(&old, "printf 'rustc 1.86.0-nightly\\nrelease: 1.86.0-nightly\\n'");
    assert!(Rustc::locate(&RustcFixture::settings(&old)).is_ok());
}

// RS-03 AC 1: the compiler named is run, `cargo` is not run, not even one beside it.
#[test]
fn the_compiler_is_run_directly_and_cargo_never() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("rustc.log");
    let cargo_log = dir.path().join("cargo.log");
    // `cargo` beside the wrapper logs any call; the wrapper puts its own directory first on
    // PATH, so a `cargo` run by anything under it would be this one.
    RustcFixture::script(&dir.path().join("cargo"), &format!("echo \"$@\" >> {}", cargo_log.display()));
    let wrapper = dir.path().join("rustc");
    RustcFixture::script(
        &wrapper,
        &format!(
            "REAL=$(command -v rustc)\nPATH=\"{}:$PATH\"\necho \"$@\" >> {}\nexec \"$REAL\" \"$@\"",
            dir.path().display(),
            log.display()
        ),
    );

    let rustc = Rustc::locate(&RustcFixture::settings(&wrapper)).expect("the real rustc behind the wrapper");
    assert!(rustc.version_text().contains("release: "));
    let src = dir.path().join("abi.rs");
    std::fs::write(&src, "pub fn f() -> u32 { 1 }\n").unwrap();
    let out = dir.path().join("libpiperine_abi.rlib");
    let warnings = rustc.rlib(&src, &out).expect("the rlib builds");
    assert!(out.is_file());
    assert_eq!(warnings, "", "no warning on a clean source");
    assert_eq!(rustc.invocations(), 1, "one build; `-vV` is not one");

    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(calls.lines().any(|l| l.trim() == "-vV"), "the version is asked: {calls}");
    let build = calls.lines().find(|l| l.contains("--crate-type rlib")).expect("the build call");
    for flag in ["--edition 2024", "opt-level=2", "debuginfo=1", "--cap-lints warn"] {
        assert!(build.contains(flag), "RS-Q4 flag {flag}: {build}");
    }
    assert!(!cargo_log.exists(), "cargo was called: {:?}", std::fs::read_to_string(&cargo_log));

    // a failing build is CompileFailed with the compiler's stderr and the source's path
    std::fs::write(&src, "pub fn f( {").unwrap();
    let err = rustc.rlib(&src, &out).expect_err("bad source");
    match err {
        Error::CompileFailed { stderr, source } => {
            assert!(stderr.contains("error"), "{stderr}");
            assert_eq!(source, src);
        }
        other => panic!("{other:?}"),
    }
}
