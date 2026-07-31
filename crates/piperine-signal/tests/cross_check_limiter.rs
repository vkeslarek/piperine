//! Cross-check fixture (DVT-06, `design.md` §C1): `$limit`'s three kinds
//! (`pnjlim`/`fetlim`/`limvds`), run both through codegen's JIT-emitted
//! formula (`kernel.eval_limit_update`, Cranelift `select` trees compiled
//! from `emit/analog_expr.rs`) and through `piperine-signal::limit`'s plain
//! `f64` port, values asserted equal across a grid of inputs covering every
//! branch of each formula.
//!
//! **This fixture is deleted by `codegen-simplification`** — once codegen
//! delegates to `piperine-signal` instead of carrying its own JIT-emitted
//! copy, there is only one implementation left and nothing to cross-check.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;
use piperine_signal::limit::{fetlim, limvds, pnjlim};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// Compile a one-`$limit` device (mirrors `piperine-codegen`'s own
/// `tests/limiters.rs::jit_limit`) and return `eval_limit_update(vnew, vold)`.
fn jit_limit(kind: &str, vnew: f64, vold: f64, vte_or_vto: f64, vcrit: f64) -> f64 {
    let src = format!(
        "{DISCIPLINE}
mod L (inout d: Electrical, inout s: Electrical) {{ param vte: Real = 1.0; param vcrit: Real = 1.0; }}
analog L {{ I(d, s) <+ $limit(\"{kind}\", V(d, s), 0.0, vte, vcrit); }}"
    );
    let elab = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    let kernel = AnalogKernel::compile(&bodies["L"]).expect("compile limiter device");

    let mut state = vec![0.0; kernel.num_state_slots()];
    state[kernel.limit_base()] = vold;
    let params = vec![vte_or_vto, vcrit];
    let vars = vec![0.0; kernel.num_vars()];
    let volts = vec![vnew, 0.0]; // V(d, s) = volts[0] - volts[1]
    let sim = SimCtx::default();
    let mut out = vec![0.0; 1];
    kernel.eval_limit_update(&volts, &params, &state, &vars, &sim, &mut out);
    out[0]
}

#[test]
fn pnjlim_matches_codegens_jit_device() {
    let vte = 0.025852;
    let vcrit = 0.7;
    // Covers both the pass-through branch (small step) and the clamp
    // branch (large step past vcrit, both signs of vold).
    let cases = [
        (0.5, 0.5),  // within tolerance: pass through
        (2.0, 0.5),  // large positive step past vcrit, vold > 0
        (2.0, -0.1), // large positive step past vcrit, vold <= 0
        (0.9, 0.85),
    ];
    for (vnew, vold) in cases {
        let jit = jit_limit("pnjlim", vnew, vold, vte, vcrit);
        let signal = pnjlim(vnew, vold, vte, vcrit);
        assert!(
            (jit - signal).abs() < 1e-9,
            "pnjlim(vnew={vnew}, vold={vold}, vte={vte}, vcrit={vcrit}): jit={jit} signal={signal}"
        );
    }
}

#[test]
fn fetlim_matches_codegens_jit_device() {
    let vto = 1.0;
    // Same branch coverage as piperine-codegen's own `tests/limiters.rs`.
    let cases = [
        (20.0, 5.0),
        (10.0, 5.0),
        (4.6, 5.0),
        (2.0, 5.0),
        (5.0, 3.0),
        (-1.0, 3.0),
        (-5.0, 0.5),
        (0.0, 0.5),
        (3.0, 0.5),
        (1.2, 0.5),
    ];
    for (vnew, vold) in cases {
        let jit = jit_limit("fetlim", vnew, vold, vto, 0.0);
        let signal = fetlim(vnew, vold, vto);
        assert!(
            (jit - signal).abs() < 1e-9,
            "fetlim(vnew={vnew}, vold={vold}, vto={vto}): jit={jit} signal={signal}"
        );
    }
}

#[test]
fn limvds_matches_codegens_jit_device() {
    let cases = [
        (20.0, 4.0),
        (10.0, 4.0),
        (3.0, 4.0),
        (1.0, 4.0),
        (3.6, 4.0),
        (5.0, 1.0),
        (3.0, 1.0),
        (-2.0, 1.0),
        (2.0, 2.5),
    ];
    for (vnew, vold) in cases {
        let jit = jit_limit("limvds", vnew, vold, 0.0, 0.0);
        let signal = limvds(vnew, vold);
        assert!(
            (jit - signal).abs() < 1e-9,
            "limvds(vnew={vnew}, vold={vold}): jit={jit} signal={signal}"
        );
    }
}
