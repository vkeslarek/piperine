//! `Transpiler::compile` end to end (feature `ir-rust`, RS-01 AC 1, RS-02 AC 4, RS-03 AC 3,
//! RS-04 AC 1, 2, 3 and 5, RS-08; checks C1, C9, C16, C18, C19, C23, C24, C25): the five M1
//! designs build with no error and no warning; a second build runs no compilation (only the
//! `rustc -vV` probe of the key), counted by a wrapper script's log and not by the crate; a changed IR or another compiler text is another
//! entry; a compiler error keeps the source and shows the compiler's words; a corrupted library
//! is rebuilt; the source sits beside the library; a build reports its cost.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use piperine_ir::{BodyId, Ir, IrBuilder};
use piperine_transpile::{Compiled, Error, Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;

use devices::Devices;

/// Writing an executable and exec'ing it races with another thread's `fork` holding the write
/// descriptor (ETXTBSY); the tests here take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The M1 body `build` makes, alone as a design.
fn design(build: fn(&mut IrBuilder) -> BodyId, name: &str, params: Vec<f64>) -> Ir {
    let mut b = IrBuilder::new();
    let body = build(&mut b);
    let top = Devices::alone(&mut b, body, name, 1, params);
    b.finish(top)
}

fn resistor() -> Ir {
    design(Devices::resistor, "R", vec![1000.0])
}

fn capacitor() -> Ir {
    design(Devices::capacitor, "C", vec![1e-9])
}

fn transpiler(ir: Ir) -> Transpiler {
    Transpiler::new(Arc::new(ir)).unwrap_or_else(|e| panic!("{e}"))
}

/// A compiler wrapper in `dir`: logs each call's arguments to `rustc.log`, then runs the real
/// `rustc`. `stamp` is added to the `-vV` text (another compiler text); `fail_cdylib` makes a
/// `cdylib` build fail with a message of its own.
struct Wrapper {
    program: PathBuf,
    log: PathBuf,
}

impl Wrapper {
    fn new(dir: &Path, name: &str, stamp: &str, fail_cdylib: bool) -> Wrapper {
        let program = dir.join(name);
        let log = dir.join(format!("{name}.log"));
        let failing = if fail_cdylib {
            "case \"$*\" in *cdylib*) echo \"error: boom from the wrapper\" >&2; exit 1;; esac"
        } else {
            ""
        };
        let text = format!(
            "#!/bin/sh\nREAL=$(command -v rustc)\necho \"$@\" >> {log}\n\
             if [ \"$1\" = \"-vV\" ]; then \"$REAL\" -vV; echo \"wrapper: {stamp}\"; exit 0; fi\n{failing}\nexec \"$REAL\" \"$@\"\n",
            log = log.display()
        );
        std::fs::write(&program, text).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Wrapper { program, log }
    }

    fn settings(&self, cache: &Path) -> Settings {
        Settings { rustc: Some(self.program.clone()), cache: cache.to_path_buf() }
    }

    /// Every call, its arguments on one line.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// The calls that build something: every call but `-vV`.
    fn builds(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.trim() != "-vV")
            .map(str::to_string)
            .collect()
    }
}

fn compile(t: &Transpiler, settings: &Settings) -> Compiled {
    t.compile(settings).unwrap_or_else(|e| panic!("{e}"))
}

// RS-01 AC 1, RS-02 AC 4: each of the five M1 designs compiles with no error and the compiler
// has nothing to say (a fresh build, so its standard error is the build's).
#[test]
fn the_m1_design_compiles_with_no_error_and_no_warning() {
    let cache = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let bodies = [
        ("Resistor", resistor()),
        ("Capacitor", capacitor()),
        ("Vsource", design(Devices::vsource, "V", vec![0.0, 0.0])),
        ("Isource", design(Devices::isource, "I", vec![0.0, 0.0])),
        ("Diode", design(Devices::diode, "D", vec![1e-14, 1.0, 0.0])),
    ];
    for (name, ir) in bodies {
        let compiled = compile(&transpiler(ir), &settings);
        let report = compiled.report();
        assert!(!report.cache_hit, "{name}: a first build");
        assert_eq!(report.warnings, "", "{name}: rustc has no warning for the generated source");
        assert_eq!(compiled.loaded().names().len(), 1, "{name}: one module per running body");
    }
}

// RS-04 AC 1: an unchanged design built a second time invokes the compiler zero times and loads
// the cached library.
#[test]
fn a_second_build_runs_no_compiler() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let wrapper = Wrapper::new(work.path(), "rustc", "one", false);
    let settings = wrapper.settings(&work.path().join("cache"));
    let t = transpiler(resistor());

    let first = compile(&t, &settings);
    let after_first = wrapper.builds();
    assert_eq!(after_first.len(), 2, "the ABI rlib and the design's cdylib: {after_first:?}");
    let calls_before = wrapper.calls().len();

    let second = compile(&t, &settings);
    assert_eq!(wrapper.builds(), after_first, "the second build ran no compiler");
    // RS-04 AC 1 as amended (verification F6): the warm build ran the version probe the key
    // needs (RS-Q3) and nothing else
    assert_eq!(wrapper.calls()[calls_before..], ["-vV".to_string()], "a warm build runs `rustc -vV` once and builds nothing");
    assert!(second.report().cache_hit);
    assert_eq!(second.report().rustc_invocations, 0);
    assert_eq!(second.source_path(), first.source_path(), "the cached entry was loaded");
    assert_eq!(second.loaded().names(), first.loaded().names());
}

// RS-04 AC 2: a change to the IR that changes the source builds and loads another entry; the
// ABI rlib is not built again.
#[test]
fn a_changed_ir_is_another_entry() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let wrapper = Wrapper::new(work.path(), "rustc", "one", false);
    let settings = wrapper.settings(&work.path().join("cache"));

    let resistor = compile(&transpiler(resistor()), &settings);
    let capacitor = compile(&transpiler(capacitor()), &settings);
    assert_ne!(resistor.source_path(), capacitor.source_path());
    assert_ne!(resistor.entry().dir(), capacitor.entry().dir());
    assert!(resistor.entry().library().is_file() && capacitor.entry().library().is_file());
    assert_eq!(resistor.loaded().names(), vec!["Resistor"]);
    assert_eq!(capacitor.loaded().names(), vec!["Capacitor"]);
    let builds = wrapper.builds();
    assert_eq!(builds.iter().filter(|l| l.contains("--crate-type rlib")).count(), 1, "the ABI is built once: {builds:?}");
    assert_eq!(builds.iter().filter(|l| l.contains("--crate-type cdylib")).count(), 2, "{builds:?}");
}

// RS-04 AC 3: the output of `rustc -vV` differs -> different entries; each builds.
#[test]
fn another_compiler_version_is_another_entry() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let cache = work.path().join("cache");
    let a = Wrapper::new(work.path(), "rustc-a", "build A", false);
    let b = Wrapper::new(work.path(), "rustc-b", "build B", false);
    let t = transpiler(resistor());

    let with_a = compile(&t, &a.settings(&cache));
    let with_b = compile(&t, &b.settings(&cache));
    assert_ne!(with_a.entry().dir(), with_b.entry().dir());
    assert!(!with_b.report().cache_hit, "the other compiler text is a miss");
    assert_eq!(b.builds().len(), 2, "the other compiler built the ABI and the design again: {:?}", b.builds());
    // and each compiler text is a hit on its own entry afterwards
    assert!(compile(&t, &a.settings(&cache)).report().cache_hit);
    assert_eq!(a.builds().len(), 2);
}

// RS-03 AC 3: a compiler error is an error holding the compiler's standard error and the path
// of the generated source, which stays on disk.
#[test]
fn a_compile_error_keeps_the_source_and_shows_stderr() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let wrapper = Wrapper::new(work.path(), "rustc", "one", true);
    let cache = work.path().join("cache");
    let t = transpiler(resistor());
    let err = t.compile(&wrapper.settings(&cache)).expect_err("the compiler fails");
    let Error::CompileFailed { stderr, source } = &err else { panic!("{err:?}") };
    assert!(stderr.contains("boom from the wrapper"), "{stderr}");
    assert!(source.is_file(), "the generated source stays on disk: {}", source.display());
    assert!(source.starts_with(&cache), "under the cache: {}", source.display());
    assert_eq!(std::fs::read_to_string(source).unwrap(), t.source().unwrap(), "it is the source that was compiled");
    let text = err.to_string();
    assert!(text.contains("boom from the wrapper") && text.contains(&*source.to_string_lossy()), "{text}");
}

// RS-04 AC 5: a cached library whose bytes changed is not loaded; the entry is rebuilt (the ABI
// rlib is kept: one more cdylib build and no more).
#[test]
fn a_corrupted_library_is_rebuilt() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let wrapper = Wrapper::new(work.path(), "rustc", "one", false);
    let settings = wrapper.settings(&work.path().join("cache"));
    let t = transpiler(resistor());

    let first = compile(&t, &settings);
    let library = first.entry().library();
    let mut bad = std::fs::read(&library).unwrap();
    bad.extend_from_slice(b"corrupted");
    // Replaced, not rewritten in place: this process still has the first library mapped, and
    // truncating a mapped file under it would crash the test, not exercise the check.
    let scratch = library.with_extension("corrupted");
    std::fs::write(&scratch, &bad).unwrap();
    std::fs::rename(&scratch, &library).unwrap();
    assert_eq!(wrapper.builds().len(), 2);

    let again = compile(&t, &settings);
    assert!(!again.report().cache_hit, "a corrupted entry is not a hit");
    assert_eq!(wrapper.builds().len(), 3, "one rebuild: {:?}", wrapper.builds());
    assert!(wrapper.builds()[2].contains("--crate-type cdylib"));
    // the rebuilt library is a whole one again: it hashes to what its manifest records (the
    // bytes are not the first build's, which carry the staging path in their debug info)
    let rebuilt = std::fs::read(again.entry().library()).unwrap();
    assert_ne!(rebuilt, bad);
    assert_eq!(piperine_transpile::Manifest::hash(&rebuilt), again.entry().manifest().unwrap().library_sha256);
    assert_eq!(again.loaded().names(), vec!["Resistor"]);
}

// RS-02 AC 4 / RS-03 AC 3: the generated source is kept beside the library and reported.
#[test]
fn the_source_is_kept_beside_the_library_and_reported() {
    let cache = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let t = transpiler(resistor());
    let compiled = compile(&t, &settings);
    let source = compiled.source_path();
    assert_eq!(source.file_name().unwrap(), "design.rs");
    assert_eq!(std::fs::read_to_string(&source).unwrap(), t.source().unwrap());
    assert_eq!(source.parent(), compiled.entry().library().parent(), "beside the library");
    assert!(compiled.entry().library().is_file());
    assert!(source.parent().unwrap().join("rt.rs").is_file(), "with the runtime source it declares");
    assert!(source.starts_with(cache.path()));
}

// RS-08: a build reports what it cost; a hit says it was a hit and ran no compiler.
#[test]
fn a_build_reports_its_cost() {
    let cache = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let t = transpiler(resistor());

    let built = compile(&t, &settings);
    let r = built.report();
    assert!(!r.cache_hit);
    assert_eq!(r.rustc_invocations, 2, "the ABI rlib and the cdylib");
    assert!(r.transpile > Duration::ZERO && r.compile > Duration::ZERO && r.load > Duration::ZERO, "{r:?}");

    let hit = compile(&t, &settings);
    let r = hit.report();
    assert!(r.cache_hit);
    assert_eq!(r.rustc_invocations, 0);
    assert_eq!(r.compile, Duration::ZERO);
    assert!(r.load > Duration::ZERO && r.transpile > Duration::ZERO, "{r:?}");
}
