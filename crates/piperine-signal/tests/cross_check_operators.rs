//! Cross-check fixture (DVT-06, `design.md` §C1): the `transition` runtime
//! operator, run both through codegen's own JIT device and through
//! `piperine-signal::ops::Operator` directly, on the same accepted-time
//! sequence, values asserted equal.
//!
//! **This fixture is deleted by `codegen-simplification`** — once codegen
//! delegates to `piperine-signal` instead of carrying its own copy of
//! `device/analog/operators.rs`, there is only one implementation left and
//! nothing to cross-check.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_signal::ops::Operator;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// A `transition($abstime, td, tr, tf)` ideal-forced source: the node
/// voltage equals the operator's output exactly at every accepted point
/// (no impedance drop), and the input is `$abstime` itself — known exactly
/// without probing anything else. Feeding `$abstime` (continuously
/// changing) keeps `transition` re-anchoring every accepted step, which
/// exercises rule (b) — the re-anchor-on-retarget rule — throughout the run.
fn run(td: f64, tr: f64, tf: f64, stop: f64, dt: f64) -> Vec<(f64, f64)> {
    let src = format!(
        "{DISCIPLINE}
mod Xition(inout p: Electrical, inout n: Electrical) {{
    param td: Real = {td};
    param tr: Real = {tr};
    param tf: Real = {tf};
}}
analog Xition {{ V(p, n) <- transition($abstime, td, tr, tf); }}
mod Top() {{
    wire gnd: Electrical;
    wire out: Electrical;
    x1: Xition(.p = out, .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    let out = info.nets.get("out").expect("net `out`").clone();

    let opts = piperine_solver::prelude::TransientAnalysisOptions::new(stop, dt);
    let ctx = piperine_solver::prelude::Context::default();
    let result = circuit.transient(opts, ctx).unwrap().solve().expect("transient completes");

    (0..result.len())
        .filter_map(|i| {
            let step = result.get(i)?;
            let v = step.get_node(&out)?;
            Some((step.time(), v))
        })
        .collect()
}

#[test]
fn transition_matches_codegens_jit_device_on_the_same_accepted_points() {
    let (td, tr, tf) = (0.0, 1e-3, 1.2e-3);
    let jit = run(td, tr, tf, 3e-3, 1e-4);
    assert!(jit.len() > 10, "the fixture must produce a non-trivial accepted-point sequence");

    // The kernel's residual at accepted step `i` reads the operator's
    // *committed* state — whatever `accept` returned as of step `i-1` — and
    // only calls `accept` with this step's own (time, input) afterwards, to
    // commit the state the *next* step will read (the same companion-model
    // lag `ddt`/`idt` use: the current step's unknowns solve against state
    // fixed at the start of the step). So the recorded value at `jit[i]` is
    // `piperine_state` *before* this step's `accept` call, not after it.
    let mut op = Operator::transition(td, tr, tf);
    let mut piperine_state = 0.0;
    for &(t, jit_v) in &jit {
        assert!(
            (piperine_state - jit_v).abs() < 1e-9,
            "t={t}: piperine-signal={piperine_state} codegen-jit={jit_v}"
        );
        // Input = $abstime = t, exactly what the PHDL device fed its own
        // transition() call at this accepted point.
        piperine_state = op.accept(t, t);
    }
}
