//! Scope: running the real ngspice binary (PAR-03): batch mode in an empty scratch directory of its
//! own per run, removal on drop, parallel runs, and a failed run or a missing export naming the
//! fixture with ngspice's output. Needs ngspice on `PATH`; it fails, as every parity test does,
//! when it is absent.

use std::path::PathBuf;

use piperine_parity::golden::Golden;
use piperine_parity::ngspice::{Ngspice, Reference};

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
