//! MD-18 enforcement: a parameter sweep must elaborate/JIT **once** and
//! restamp the swept value on the compiled circuit — never re-compile per
//! point. Lives in its own test binary so [`AnalogKernel::compile_count`]
//! deltas are not polluted by concurrent tests in the same process.

use std::path::PathBuf;

use piperine::{Session, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::Value;
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn diode_design() -> piperine_lang::Design {
    let phdl = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ngspice/diode_iv.phdl"));
    let src = std::fs::read_to_string(&phdl).expect("diode_iv.phdl fixture");
    piperine_lang::parse_and_elaborate(&src, &headers_source_map()).expect("elaboration")
}

/// A `Session::sweep` JITs nothing at all — the whole sweep runs on the one
/// build `Session::compile` produced — and every point matches the staged
/// per-point path (a fresh compile per value) within the validation tolerance
/// (`|Δ| ≤ 1e-9 + 1e-3·max`).
#[test]
fn sweep_compiles_once_and_matches_the_staged_path() {
    let design = diode_design();
    let source = "v1";
    let (branch_a, branch_b) = ("vin", "gnd");
    let values: Vec<f64> = (0..=12).map(|i| -0.6 + 0.1 * i as f64).collect();
    let config = SolverConfig::default();
    let read_i = |op: &piperine::OpResult| op.i((branch_a, branch_b)).expect("current readback");

    // Reference: the staged per-point path (one compile per point).
    let reference: Vec<f64> = values
        .iter()
        .map(|&v| {
            let mut staged = Session::builder(&design, "Top")
                .stage(source, "dc", Value::Real(v))
                .compile()
                .expect("staged compile");
            read_i(&staged.op(&config, None).expect("staged op"))
        })
        .collect();

    // One single build, for the per-build compile count.
    let before_single = AnalogKernel::compile_count();
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let per_build = AnalogKernel::compile_count() - before_single;
    assert!(per_build > 0, "a build must JIT at least one kernel");

    // The compile-once sweep: restamp `source.dc` on the built circuit and
    // solve an operating point per point.
    let before_sweep = AnalogKernel::compile_count();
    let mut ops = Vec::with_capacity(values.len());
    {
        let mut sweep = session.sweep(source, "dc", &values);
        while let Some(point) = sweep.next() {
            let mut point = point.expect("sweep point restamps");
            ops.push(point.op(&config, None).expect("swept op"));
        }
    }
    let sweep_compiles = AnalogKernel::compile_count() - before_sweep;

    assert_eq!(ops.len(), values.len());
    assert_eq!(
        sweep_compiles, 0,
        "MD-18: a {}-point sweep restamps on the one build ({per_build} kernel(s)) and must \
         JIT nothing more, got {sweep_compiles}",
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
    let err = match session.sweep("nope", "dc", &[0.0]).next().expect("one point") {
        Err(e) => e,
        Ok(_) => panic!("unknown label must fail"),
    };
    assert!(err.to_string().contains("nope"), "names the label: {err}");

    let err = match session.sweep("v1", "bogus_param", &[0.0]).next().expect("one point") {
        Err(e) => e,
        Ok(_) => panic!("unknown param must fail"),
    };
    assert!(err.to_string().contains("bogus_param"), "names the param: {err}");
}
