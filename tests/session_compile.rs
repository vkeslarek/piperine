//! HOST-01 — `Session`: the compiled center of gravity. `module.compile()`'s
//! Rust equivalent — elaborate + JIT once, then `set` + re-run without
//! recompiling (MD-18). `cargo test -p piperine` (Phase 1 / T3 quick gate).
//!
//! Single `#[test]` in its own binary (mirrors `tests/dc_host_proof.rs`): the
//! process-global `AnalogKernel::compile_count` delta this test asserts on
//! would be polluted by any other test compiling a kernel concurrently in
//! the same process (see `tests/session_analyses.rs` for the rest of the
//! `Session` surface).

use std::path::PathBuf;

use piperine::{NetRef, Session, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn divider_design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &headers_source_map()).expect("divider elaborates")
}

/// `Session::compile` holds the built circuit: the baseline `op` reads the
/// same value a fresh `Session` would, `rebuilds()` starts at `0`, and a
/// restamp `set` + re-run loop matches independent fresh builds — one
/// compilation total (MD-18), never a re-JIT per `set`.
#[test]
fn session_compiles_once_and_set_op_matches_fresh_builds() {
    let mid = NetRef { name: "mid".into() };

    let mut session = Session::compile(&divider_design(), "Divider").expect("session compiles");
    assert_eq!(session.rebuilds(), 0, "no structural set yet");

    let op0 = session.op(&SolverConfig::default(), None).expect("baseline op");
    assert!((op0.v(&mid).expect("v(mid)") - 2.0).abs() < 1e-9);

    // The live set/op loop, measured in isolation (MD-18: zero JITs).
    let before = AnalogKernel::compile_count();
    let mut live_values = Vec::new();
    for r in [1e3, 2e3, 4e3, 6e3] {
        session.set("r_top", "r", r).expect("restamp set");
        live_values.push(session.op(&SolverConfig::default(), None).expect("op").v(&mid).expect("v(mid)"));
    }
    let sweep_compiles = AnalogKernel::compile_count() - before;
    assert_eq!(sweep_compiles, 0, "the set/op loop must never re-JIT (MD-18), got {sweep_compiles}");

    // Independent fresh-build references, computed AFTER the sweep so their
    // JITs don't pollute the compile-count window above.
    for (r, live) in [1e3, 2e3, 4e3, 6e3].into_iter().zip(live_values) {
        let mut fresh_session = Session::builder(&divider_design(), "Divider")
            .stage("r_top", "r", piperine_lang::Value::Real(r))
            .compile()
            .expect("session compiles");
        let fresh = fresh_session
            .op(&SolverConfig::default(), None)
            .expect("fresh op")
            .v(&mid)
            .expect("v(mid)");
        assert!((live - fresh).abs() < 1e-9, "r_top = {r}: live {live} V vs fresh build {fresh} V");
    }
}
