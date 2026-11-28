//! Scope: running the real ngspice binary (PAR-03): batch mode in an empty scratch directory of its
//! own per run, removal on drop, parallel runs, and a failed run or a missing export naming the
//! fixture with ngspice's output. Needs ngspice on `PATH`; it fails, as every parity test does,
//! when it is absent. The compile cache every test target of a run shares is held here too: two processes
//! building one design into one cache directory leave one complete entry and no staging directory.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use piperine_ir_fixtures::alone::Alone;
use piperine_ir_fixtures::devices::Devices;
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::golden::Golden;
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_transpile::{Settings, Transpiler};

fn run(fixture: &str) -> Golden {
    match Ngspice::from_environment().run(fixture) {
        Reference::Golden(golden) => golden,
        Reference::Skipped => panic!("{fixture}: the harness tests need ngspice; PIPERINE_NGSPICE=skip is set"),
    }
}

fn panic_message(work: impl FnOnce() + std::panic::UnwindSafe) -> String {
    let payload = std::panic::catch_unwind(work).expect_err("the work must panic");
    match payload.downcast::<String>() {
        Ok(text) => *text,
        Err(other) => other.downcast::<&str>().map(|text| text.to_string()).expect("a text panic"),
    }
}

#[test]
fn ngspice_runs_in_batch_in_an_empty_scratch_directory() {
    let golden = run("divider");
    assert!(golden.stdout().contains("v(mid) = 7.500000e+00"), "{}", golden.stdout());
    assert!(golden.scratch_path().is_dir());
    assert!(golden.version().contains("ngspice"), "{}", golden.version());
    // divider.cir exports nothing, so the working directory it ran in is still empty.
    assert_eq!(std::fs::read_dir(golden.scratch_path()).expect("read scratch").count(), 0);
}

#[test]
fn each_run_has_its_own_scratch_removed_on_drop() {
    let first = run("divider");
    let second = run("divider");
    assert_ne!(first.scratch_path(), second.scratch_path());
    let kept: Vec<PathBuf> = vec![first.scratch_path().to_path_buf(), second.scratch_path().to_path_buf()];
    drop(first);
    drop(second);
    for path in kept {
        assert!(!path.exists(), "{} survived the drop", path.display());
    }
}

#[test]
fn a_failed_run_or_missing_export_names_the_fixture() {
    let message = panic_message(|| {
        run("no_such_fixture");
    });
    assert!(message.contains("no_such_fixture"), "{message}");

    let golden = run("diode_iv");
    let message = panic_message(std::panic::AssertUnwindSafe(|| {
        golden.export_file("no_such_export");
    }));
    assert!(message.contains("diode_iv"), "{message}");
    assert!(message.contains("no_such_export"), "{message}");
    assert!(message.contains("ngspice stdout"), "carries ngspice's output: {message}");
}

#[test]
fn parallel_runs_do_not_share_a_scratch_directory() {
    let handles: Vec<_> = (0..4).map(|_| std::thread::spawn(|| run("diode_iv"))).collect();
    let goldens: Vec<Golden> = handles.into_iter().map(|handle| handle.join().expect("a run thread")).collect();
    for (index, golden) in goldens.iter().enumerate() {
        assert!(golden.export_file("diode_iv_sweep").is_file());
        for other in &goldens[index + 1..] {
            assert_ne!(golden.scratch_path(), other.scratch_path());
        }
    }
}

#[test]
fn the_exports_of_the_corpus_parse_to_their_row_counts() {
    let diode = run("diode_iv");
    let sweep = diode.real("diode_iv_sweep");
    diode.expect_rows("diode_iv_sweep", &sweep, 37);
    assert_eq!(sweep[0], (-1.0, 1.01008091e-12));

    let ac = run("ac_rc");
    let response = ac.complex("ac_rc_ac");
    ac.expect_rows("ac_rc_ac", &response, 41);
    let corner = response.iter().find(|(frequency, _)| (frequency - 1e5).abs() < 1.0).expect("the 100 kHz point");
    assert!((corner.1.re - 0.5).abs() < 1e-6 && (corner.1.im + 0.5).abs() < 1e-6, "{:?}", corner.1);

    let tran = run("tran_rc");
    let rows = tran.real("tran_rc_tran");
    tran.expect_rows("tran_rc_tran", &rows, 100);
}

/// The processes that write one design into one cache directory at the same time: the test binary run again
/// with `a_process_that_writes_the_cache` selected and the directory named in its environment.
struct CacheWriters;

impl CacheWriters {
    /// The environment variable that names the directory a writer builds into.
    const DIRECTORY: &'static str = "PIPERINE_CACHE_WRITER_DIRECTORY";

    /// The writer: compiles the resistor design into the directory the environment names.
    fn write(directory: &std::path::Path) {
        let (ir, _) = Alone::design(Devices::resistor);
        let settings = Settings { rustc: None, cache: directory.to_path_buf() };
        Transpiler::new(Arc::new(ir)).unwrap_or_else(|error| panic!("{error}")).compile(&settings).unwrap_or_else(|error| panic!("{error}"));
    }

    /// A new writer process, running.
    fn start(directory: &std::path::Path) -> std::process::Child {
        Command::new(std::env::current_exe().expect("the test binary"))
            .args(["--exact", "harness::a_process_that_writes_the_cache", "--nocapture", "--test-threads=1"])
            .env(Self::DIRECTORY, directory)
            .spawn()
            .expect("start a writer")
    }
}

#[test]
fn a_process_that_writes_the_cache() {
    // Started by `two_processes_share_one_cache` it builds into the directory the environment names; run on
    // its own it builds into a directory of its own under the run's cache. Either way one entry results.
    let directory = match std::env::var(CacheWriters::DIRECTORY) {
        Ok(directory) => PathBuf::from(directory),
        Err(_) => CompileCache::shared().path().join(format!("one-writer-{}", std::process::id())),
    };
    CacheWriters::write(&directory);
    let entries = std::fs::read_dir(directory.join("rust")).expect("the cache's rust directory").count();
    assert!(entries > 0, "the writer left nothing in {}", directory.display());
}

#[test]
fn two_calls_of_the_shared_cache_give_one_directory_of_the_run() {
    let first = CompileCache::shared();
    let second = CompileCache::shared();
    assert!(std::ptr::eq(first, second));
    assert_eq!(first.path(), second.path());
    assert_eq!(first.path().file_name().and_then(|name| name.to_str()), Some(std::os::unix::process::parent_id().to_string().as_str()));
    assert_eq!(first.path().parent().and_then(|base| base.file_name()).and_then(|name| name.to_str()), Some("piperine-compile-cache"));
}

/// A base directory that holds one folder per run.
struct PlantedRuns {
    base: tempfile::TempDir,
}

impl PlantedRuns {
    /// A base holding the folders `names`.
    fn named(names: &[String]) -> PlantedRuns {
        let base = tempfile::tempdir().expect("a base directory");
        for name in names {
            std::fs::create_dir_all(base.path().join(name).join("rust")).expect("a planted run");
        }
        PlantedRuns { base }
    }

    /// Whether the folder `name` is still in the base after the removal a run does at its start.
    fn remains(&self, name: &str) -> bool {
        CompileCache::remove_finished_runs(self.base.path());
        self.base.path().join(name).exists()
    }
}

#[test]
fn a_live_process_keeps_its_folder_when_finished_runs_are_removed() {
    let live = std::process::id().to_string();
    assert!(PlantedRuns::named(&[live.clone()]).remains(&live));
}

#[test]
fn the_folder_of_a_finished_process_is_removed() {
    let finished = "4294967294".to_string();
    assert!(!PlantedRuns::named(&[finished.clone()]).remains(&finished));
}

#[test]
fn a_folder_that_is_no_process_number_is_removed() {
    let stray = "notapid".to_string();
    assert!(!PlantedRuns::named(&[stray.clone()]).remains(&stray));
}

#[test]
fn two_processes_share_one_cache() {
    let directory = CompileCache::shared().path().join(format!("two-writers-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a directory for the two writers");
    let mut writers = [CacheWriters::start(&directory), CacheWriters::start(&directory)];
    for writer in &mut writers {
        let status = writer.wait().expect("wait for a writer");
        assert!(status.success(), "a writer failed: {status}");
    }
    let names: Vec<String> = std::fs::read_dir(directory.join("rust")).expect("the cache's rust directory").map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned()).collect();
    assert!(names.iter().all(|name| !name.starts_with(".staging-")), "a staging directory was left: {names:?}");
    let entries = names.iter().filter(|name| name.len() == 32).count();
    let libraries = names.iter().filter(|name| name.starts_with("abi-")).count();
    assert_eq!((entries, libraries), (1, 1), "one design, one entry and one ABI library: {names:?}");
    std::fs::remove_dir_all(&directory).expect("remove the writers' directory");
}
