//! T8 — the LIVE-12 optimization-loop example (`examples/live_optimize.py`)
//! must pass end-to-end (bisection fit, 100-point equality vs fresh builds
//! within reltol 1e-3, >= 10x speedup — asserted inside the script), and its
//! whole run must JIT exactly `1 + 100` circuit builds: one for the live
//! session, one per fresh-build reference point — the live loop itself adds
//! **zero** compilations (MD-18).
//!
//! Isolated test binary: [`AnalogKernel::compile_count`] is process-global
//! (same pattern as `tests/live_session.rs`).

use std::path::PathBuf;

use piperine_codegen::AnalogKernel;
use piperine_python::embed::run_script;

#[test]
fn live_optimize_example_passes_with_one_live_compilation() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples");
    let example = examples.join("live_optimize.py");
    let example = example.to_str().expect("utf8 path");

    // Per-build kernel count for this fixture: one fresh module.op().
    let probe = format!(
        r#"import piperine
m = piperine.load({phdl:?}).module("Fitter")
v = m.op().v("out")
assert 0.6 < v < 0.75, v
"#,
        phdl = examples.join("live_optimize.phdl").to_str().expect("utf8 path"),
    );
    let probe_path = std::env::temp_dir().join("piperine_live_optimize_probe.py");
    std::fs::write(&probe_path, probe).expect("write probe");
    let before_probe = AnalogKernel::compile_count();
    run_script(probe_path.to_str().expect("utf8 path")).expect("probe op");
    let per_build = AnalogKernel::compile_count() - before_probe;
    assert!(per_build > 0, "a build must JIT at least one kernel");

    // The example: 1 session compile + 100 fresh-build reference points.
    // Bisection (61 ops) and the 100-point live loop restamp — zero builds.
    let before = AnalogKernel::compile_count();
    let result = run_script(example);
    assert!(result.is_ok(), "live_optimize.py must pass: {:?}", result.err());
    let compiles = AnalogKernel::compile_count() - before;
    assert_eq!(
        compiles,
        101 * per_build,
        "MD-18: expected 1 live + 100 reference builds ({} kernels each), got {compiles}",
        per_build
    );

    let _ = std::fs::remove_file(probe_path);
}
