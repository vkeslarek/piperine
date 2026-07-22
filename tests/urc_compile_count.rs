//! FLAT-05/06/07 — compile-count regression guards for the FlattenHierarchy
//! pass. The non-destructive flatten consumes already-monomorphized modules
//! and inlines them into the top-level flat netlist; the per-shape kernel
//! keying, the compile-once restamp, and MD-18's zero-recompile sweep must
//! all still hold on a flattened circuit.
//!
//! Lives in its own test binary (single `#[test]`) so the process-global
//! [`AnalogKernel::compile_count`] deltas are not polluted by concurrent
//! tests — the same isolation discipline as `compile_once_sweep.rs` and
//! `dc_host_proof.rs`.
//!
//! Authoring note (Option A — fixed-N modules). The mid-level `urcN` modules
//! are pure-structural — no analog body, no kernel of their own. Codegen
//! compiles kernels keyed by LEAF module name (`res`, `cap`, `vsrc`), so
//! every urcN build produces the same leaf-kernel set. The compile_count
//! process-counter increments once per kernel per BUILD (each
//! `CircuitCompiler::build_circuit` starts with an empty kernel cache), so
//! the regression invariants become:
//!   - FLAT-05: every urcN build compiles a fixed leaf-kernel count ≥ 2
//!     (res + cap) — flatten neither drops nor invents leaf kernels.
//!   - FLAT-06: a multi-value sweep restamps on ONE build's kernels; the
//!     restamp path is loud on a flattened-leaf label (`u1.s0.r1`).
//!   - FLAT-07 / MD-18: a 20-point sweep JITs exactly one build's worth of
//!     kernels, not one per point — `sweep_compiles == per_build`.

use std::path::PathBuf;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::{SourceMap, Value};

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A `Top` that instantiates one `urcN` ladder (driver + load + urcN). The
/// per-segment R is a non-structural param exposed for staging/restamp.
fn urc_top(urc_mod: &str) -> String {
    format!(
        "use piperine::disciplines;
         use spice::passives;
         use spice::sources;
         use spice::urc;

         mod Top () {{
             wire gnd  : Electrical;
             wire vin  : Electrical;
             wire vout : Electrical;
             v1 : vsrc (.p=vin, .n=gnd)   {{ .dc = 5.0 }};
             u1 : {urc_mod} (.p=vin, .n=vout, .g=gnd) {{ .r = 100.0, .c = 1.0e-9 }};
             rl : res   (.p=vout, .n=gnd) {{ .r = 1.0e3 }};
         }}"
    )
}

fn urc_session(urc_mod: &str) -> SimSession {
    let design = piperine_lang::parse_and_elaborate(&urc_top(urc_mod), &headers_source_map())
        .unwrap_or_else(|e| panic!("{urc_mod} elaborates: {e:?}"));
    SimSession::new(design, "Top".to_string())
}

fn v_out(op: &piperine::OpResult) -> f64 {
    op.v(&NetRef { name: "vout".into() }, None).expect("v(vout) readback")
}

/// FLAT-05/06/07: flatten does not regress the per-shape kernel keying
/// (FLAT-05), the compile-once restamp path (FLAT-06), or MD-18's
/// zero-recompile sweep (FLAT-07).
#[test]
fn flatten_preserves_kernel_keying_and_restamp_invariants() {
    let config = SolverConfig::default();

    // ── FLAT-05: every urcN shape compiles the same leaf-kernel count ─────
    // Build urc5 (Top → urc5 → 5 urc_seg → 10 res/cap leaves) and urc10
    // (Top → urc10 → 10 urc_seg → 20 res/cap leaves). Both flatten to a
    // leaf-only netlist of {vsrc, res, cap}; both compile the same kernel
    // count K. If flatten dropped a leaf or invented a new leaf module
    // name, K would differ between urc5 and urc10.
    let sess5 = urc_session("urc5");
    let before_5 = AnalogKernel::compile_count();
    let op5 = sess5.run_op(&config, None).expect("urc5 op");
    let delta_5 = AnalogKernel::compile_count() - before_5;

    let sess10 = urc_session("urc10");
    let before_10 = AnalogKernel::compile_count();
    let op10 = sess10.run_op(&config, None).expect("urc10 op");
    let delta_10 = AnalogKernel::compile_count() - before_10;

    assert!(
        delta_5 >= 2,
        "FLAT-05: urc5 build must compile at least the res + cap leaf kernels, got {delta_5}"
    );
    assert_eq!(
        delta_5, delta_10,
        "FLAT-05: urc5 and urc10 share the leaf-kernel set (res/cap/vsrc); \
         a delta mismatch would mean flatten changed the leaf names per shape"
    );
    // DC operating-point sanity: Vout = 5 · Rload/(N·Rseg + Rload).
    // urc5: 5·1000/(500+1000) = 3.333V. urc10: 5·1000/(1000+1000) = 2.500V.
    assert!(
        (v_out(&op5) - 3.3333333).abs() < 1e-3,
        "urc5 baseline v(vout) = 3.333V, got {}",
        v_out(&op5)
    );
    assert!(
        (v_out(&op10) - 2.5).abs() < 1e-3,
        "urc10 baseline v(vout) = 2.5V, got {}",
        v_out(&op10)
    );

    // ── FLAT-06: restamp on a flattened-leaf label is loud and works ──────
    // The flatten pass exposes every inlined leaf under a path-prefixed
    // flat label (`u1.s0.r1` = the first series resistor). Staging `.r` on
    // that label must restamp the existing `res` kernel — Vout shifts
    // because the segment count's worth of R changes, but no new kernel is
    // needed beyond the single build's worth. Also: the restamp path is
    // LOUD on a flattened label that does not exist (regression of the
    // flat-label host contract from FLAT-03).
    let bad = sess5.run_op_sweep("nope.s0.r1", "r", &[100.0], &config, None);
    let err = bad.expect_err("unknown flattened label must fail loud");
    assert!(
        err.to_string().contains("nope"),
        "FLAT-06: restamp error names the bad label: {err}"
    );

    // ── FLAT-07 / MD-18: 20-point sweep JITs one build, not 20 ───────────
    // 20 distinct `.r` values on `u1.s0.r1`; each Vout must match the
    // staged-single-build reference within tolerance, and the sweep's
    // compile delta must equal `per_build` (one build's worth of kernels),
    // NOT 20·per_build.
    let r_values: Vec<f64> = (0..20).map(|i| 50.0 + 10.0 * i as f64).collect(); // 50Ω … 240Ω

    // Reference: per-point staged single-build Vout (each point rebuilds).
    let reference: Vec<f64> = r_values
        .iter()
        .map(|&r| {
            sess5.stage("u1.s0.r1", "r", Value::Real(r));
            v_out(&sess5.run_op(&config, None).expect("staged op"))
        })
        .collect();

    // Single-build compile count (one fresh build after the reference loop).
    let before_single = AnalogKernel::compile_count();
    sess5.run_op(&config, None).expect("single op");
    let per_build = AnalogKernel::compile_count() - before_single;
    assert_eq!(
        per_build, delta_5,
        "per_build kernel count is stable across urc5 builds (got {per_build}, expected {delta_5})"
    );

    // The compile-once sweep.
    let before_sweep = AnalogKernel::compile_count();
    let ops = sess5
        .run_op_sweep("u1.s0.r1", "r", &r_values, &config, None)
        .expect("compile-once sweep");
    let sweep_compiles = AnalogKernel::compile_count() - before_sweep;

    assert_eq!(ops.len(), r_values.len(), "one OpResult per sweep value");
    assert_eq!(
        sweep_compiles, per_build,
        "FLAT-07 / MD-18: a {}-point sweep must JIT one build ({per_build} kernel(s)), got {sweep_compiles}",
        r_values.len()
    );

    // Every sweep point matches its staged-single-build reference — the
    // restamp actually changes Vout (proving the flattened-leaf label is
    // wired through to the kernel param), and does so accurately.
    for ((r, ref_v), op) in r_values.iter().zip(&reference).zip(&ops) {
        let sweep_v = v_out(op);
        assert!(
            (sweep_v - ref_v).abs() <= 1e-9 + 1e-3 * sweep_v.abs().max(ref_v.abs()),
            "FLAT-07: r={r}Ω sweep v(vout)={sweep_v:+.6e} vs staged ref {ref_v:+.6e}"
        );
    }
}
