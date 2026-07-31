//! Cross-check fixture (DVT-06, `design.md` §C1): the `cross` event
//! detector, run both through codegen's own JIT device (an `@cross` action
//! incrementing a counter, exposed as a voltage — the same shape as
//! `examples/13_zero_cross_counter.phdl`) and through
//! `piperine-signal::events::EventDetector` directly, values asserted equal
//! at every accepted point.
//!
//! **This fixture is deleted by `codegen-simplification`** — once codegen
//! delegates to `piperine-signal` instead of carrying its own copy of
//! `device/analog/events.rs`, there is only one implementation left and
//! nothing to cross-check.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_signal::events::{CrossDir, EventDetector, Trigger};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// A 1 kHz sine (ideal-forced, so `V(sig, gnd)` equals the formula exactly
/// at every accepted point) into a `@cross` counter that exposes its running
/// count as a voltage. Both the sensed value and the count are known
/// exactly at every accepted time without probing any extra node.
fn run(freq: f64, stop: f64, dt: f64) -> Vec<(f64, f64)> {
    let src = format!(
        "{DISCIPLINE}
mod Sine(inout p: Electrical, inout n: Electrical) {{
    param freq: Real = {freq};
}}
analog Sine {{ V(p, n) <- sin(6.283185307179586 * freq * $abstime); }}
mod Detector(input vin: Electrical, inout cnt: Electrical, inout gnd: Electrical) {{
    var n: Real = 0.0;
}}
analog Detector {{
    V(cnt, gnd) <- n;
    @cross(V(vin, gnd)) {{ n = n + 1.0; }}
}}
mod Top() {{
    wire gnd: Electrical;
    wire sig: Electrical;
    wire cnt: Electrical;
    src: Sine(.p = sig, .n = gnd);
    det: Detector(.vin = sig, .cnt = cnt, .gnd = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    let cnt = info.nets.get("cnt").expect("net `cnt`").clone();

    let opts = piperine_solver::prelude::TransientAnalysisOptions::new(stop, dt);
    let ctx = piperine_solver::prelude::Context::default();
    let result = circuit.transient(opts, ctx).unwrap().solve().expect("transient completes");

    (0..result.len())
        .filter_map(|i| {
            let step = result.get(i)?;
            let v = step.get_node(&cnt)?;
            Some((step.time(), v))
        })
        .collect()
}

#[test]
fn cross_matches_codegens_jit_device_on_the_same_accepted_points() {
    let freq = 1e3;
    let jit = run(freq, 3e-3, 1e-5); // 3 periods, fine resolution

    // Same lag `cross_check_operators.rs` found for `transition`: the
    // kernel's residual at accepted step `i` reads `n` as committed by
    // event detection *before* this step (the counter used to stamp
    // `V(cnt)` this step is the previous step's result); `@cross` detects
    // against *this* step's own accepted solution only afterwards,
    // committing the count the *next* step will read.
    let mut detector = EventDetector::new(0.0);
    let mut committed_count = 0.0_f64;
    for &(t, jit_count) in &jit {
        assert!(
            (committed_count - jit_count).abs() < 1e-9,
            "t={t}: piperine-signal count={committed_count} codegen-jit count={jit_count}"
        );
        let sensed = (std::f64::consts::TAU * freq * t).sin();
        if detector.fired(Trigger::Cross(CrossDir::Either), sensed, t, 0.0) {
            committed_count += 1.0;
        }
    }
    assert!(committed_count >= 5.0, "3 periods of a sine should cross zero at least 5 times, got {committed_count}");
}
