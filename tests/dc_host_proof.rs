//! SC-07 — host-level `.dc` proof: nested two-param and source-only sweeps
//! run on ONE compiled circuit via the restamp path (`set_element_param`),
//! and every point equals an independent fresh-build solve of the same
//! values. Closes ROADMAP's ".dc native analysis" item at the host level
//! (user decision 2026-07-18). Lives in its own test binary (single
//! `#[test]`) so the process-global `AnalogKernel::compile_count` deltas are
//! not polluted by concurrent tests.

use std::path::PathBuf;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_codegen::{AnalogKernel, CircuitCompiler};
use piperine_lang::{SourceMap, Value};
use piperine_solver::prelude::Context;

const DIVIDER: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod R (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog R { I(p, n) <+ V(p, n) / r; }

    mod Vsrc (inout p : Electrical, inout n : Electrical) {
        param dc : Real = 10.0;
    }
    analog Vsrc { V(p, n) <- dc; }

    mod Top () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire mid : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = mid) {};
        r2 : R(.p = mid, .n = gnd) {};
    }
"#;

fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Independent fresh-build reference: a brand-new session per point with the
/// values staged, consumed by elaboration (the maximally-independent path).
fn fresh_build_v_mid(v_dc: f64, r_bot: f64) -> f64 {
    let design = piperine_lang::parse_and_elaborate(DIVIDER, &headers_source_map())
        .expect("divider elaborates");
    let session = SimSession::new(design, "Top".to_string());
    session.stage("v1", "dc", Value::Real(v_dc));
    session.stage("r2", "r", Value::Real(r_bot));
    let op = session.run_op(&SolverConfig::default(), None).expect("fresh op");
    op.v(&NetRef { name: "mid".into() }, None).expect("v(mid)")
}

/// Nested `(v1.dc × r2.r)` and source-only sweeps on one compilation:
/// zero JITs after the build, every point equal to the fresh-build solve.
#[test]
fn nested_and_source_sweeps_restamp_one_compilation() {
    let design = piperine_lang::parse_and_elaborate(DIVIDER, &headers_source_map())
        .expect("divider elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    circuit.init_digital().expect("digital init");
    circuit.rebuild_digital_topology();
    let mid = info.nets.get("mid").expect("net `mid` mapped").clone();

    let outer: Vec<f64> = vec![2.0, 5.0, 10.0];
    let inner: Vec<f64> = vec![5.0e2, 1.0e3, 2.0e3, 4.0e3];

    // Nested two-param sweep — restamp both params per point on the one
    // compilation, collecting solutions; fresh-build references are computed
    // AFTER the sweep so their JITs don't pollute the compile-count window.
    let before = AnalogKernel::compile_count();
    let mut nested: Vec<(f64, f64, f64)> = Vec::new();
    for &v_dc in &outer {
        circuit
            .set_element_param("v1", "dc", piperine_solver::abi::Value::Real(v_dc))
            .expect("set v1.dc");
        for &r_bot in &inner {
            circuit
                .set_element_param("r2", "r", piperine_solver::abi::Value::Real(r_bot))
                .expect("set r2.r");
            let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
            nested.push((v_dc, r_bot, result.get_node(&mid).expect("v(mid)")));
        }
    }

    // Source-only sweep on the same compilation (SC-07/AC2: a *source*
    // value restamps like any param).
    let mut source_only: Vec<(f64, f64)> = Vec::new();
    for &v_dc in &[1.0, 3.0, 7.5, 12.0] {
        circuit
            .set_element_param("v1", "dc", piperine_solver::abi::Value::Real(v_dc))
            .expect("set v1.dc");
        circuit
            .set_element_param("r2", "r", piperine_solver::abi::Value::Real(1.0e3))
            .expect("reset r2.r");
        let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
        source_only.push((v_dc, result.get_node(&mid).expect("v(mid)")));
    }

    // MD-18: the whole nested + source sweep JIT'd nothing after the build.
    let sweep_compiles = AnalogKernel::compile_count() - before;
    assert_eq!(sweep_compiles, 0, "restamp sweeps must never JIT (got {sweep_compiles})");

    // Every point equals an independent fresh-build solve — exact equality
    // (linear circuit: both paths run the same kernel arithmetic on the
    // same values).
    for (v_dc, r_bot, restamped) in nested {
        let fresh = fresh_build_v_mid(v_dc, r_bot);
        assert!(
            restamped == fresh,
            "point (v1.dc={v_dc}, r2.r={r_bot}): restamped v(mid)={restamped:?} != fresh-build {fresh:?}"
        );
    }
    for (v_dc, restamped) in source_only {
        let fresh = fresh_build_v_mid(v_dc, 1.0e3);
        assert!(
            restamped == fresh,
            "source point v1.dc={v_dc}: restamped v(mid)={restamped:?} != fresh-build {fresh:?}"
        );
    }
}
