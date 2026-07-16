//! MD-18 enforcement: a parameter sweep must elaborate/JIT **once** and
//! restamp the swept value on the compiled circuit — never re-compile per
//! point. Lives in its own test binary so [`AnalogKernel::compile_count`]
//! deltas are not polluted by concurrent tests in the same process.

use std::path::PathBuf;

use piperine_bench::{NetRef, SimSession, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::eval::Value;
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn diode_session() -> SimSession {
    let phdl = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ngspice/diode_iv.phdl"));
    let src = std::fs::read_to_string(&phdl).expect("diode_iv.phdl fixture");
    let design =
        piperine_lang::parse_and_elaborate(&src, &headers_source_map()).expect("elaboration");
    SimSession::new(design, "Top".to_string())
}

/// `run_op_sweep` JITs exactly one circuit build for the whole sweep, and
/// every point matches the staged per-point path within the validation
/// tolerance (`|Δ| ≤ 1e-9 + 1e-3·max`).
#[test]
fn sweep_compiles_once_and_matches_the_staged_path() {
    let session = diode_session();
    let source = "v1";
    let (branch_a, branch_b) = ("vin", "gnd");
    let values: Vec<f64> = (0..=12).map(|i| -0.6 + 0.1 * i as f64).collect();
    let config = SolverConfig::default();
    let read_i = |op: &piperine_bench::OpResult| {
        op.i(&NetRef { name: branch_a.into() }, Some(&NetRef { name: branch_b.into() }))
            .expect("current readback")
    };

    // Reference: the staged per-point path (build_circuit per call).
    let reference: Vec<f64> = values
        .iter()
        .map(|&v| {
            session.stage(source, "dc", Value::Real(v));
            read_i(&session.run_op(&config, &Value::Unit).expect("staged op"))
        })
        .collect();

    // One single build, for the per-build compile count.
    let before_single = AnalogKernel::compile_count();
    session.run_op(&config, &Value::Unit).expect("single op");
    let per_build = AnalogKernel::compile_count() - before_single;
    assert!(per_build > 0, "a build must JIT at least one kernel");

    // The compile-once sweep.
    let before_sweep = AnalogKernel::compile_count();
    let ops = session
        .run_op_sweep(source, "dc", &values, &config, &Value::Unit)
        .expect("compile-once sweep");
    let sweep_compiles = AnalogKernel::compile_count() - before_sweep;

    assert_eq!(ops.len(), values.len());
    assert_eq!(
        sweep_compiles, per_build,
        "MD-18: a {}-point sweep must JIT one build ({per_build} kernel(s)), got {sweep_compiles}",
        values.len()
    );

    for ((v, r), op) in values.iter().zip(&reference).zip(&ops) {
        let i = read_i(op);
        assert!(
            (i - r).abs() <= 1e-9 + 1e-3 * i.abs().max(r.abs()),
            "point {source}={v}: restamped path i={i:+.6e} vs staged path i={r:+.6e}"
        );
    }

    // The restamp path is loud on bad addressing: unknown instance labels
    // and unknown parameters both fail with the offending name. (Same test
    // body — a second `#[test]` in this file would run concurrently and
    // pollute the compile-count deltas above.)
    let err = session
        .run_op_sweep("nope", "dc", &[0.0], &config, &Value::Unit)
        .expect_err("unknown label must fail");
    assert!(err.to_string().contains("nope"), "names the label: {err}");

    let err = session
        .run_op_sweep("v1", "bogus_param", &[0.0], &config, &Value::Unit)
        .expect_err("unknown param must fail");
    assert!(err.to_string().contains("bogus_param"), "names the param: {err}");
}
