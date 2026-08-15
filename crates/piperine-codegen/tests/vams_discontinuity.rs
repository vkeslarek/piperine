//! LRMC-C7-11 (T42): `$discontinuity` has a real numerical effect —
//! §8.4.4/§9.5.3: "the simulator shall not use information prior to the
//! current time step in any prediction for time steps after this one."
//!
//! Fixture: an ideal source forces `V(vout)` through a hard step at
//! `$abstime >= 0.5` (0 → 1), driving a capacitor. Without
//! `$discontinuity`, the TR-stage companion model reuses the
//! pre-jump previous-derivative term across the step, producing an LTE
//! spike right after the jump that forces the step controller to shrink
//! hard and crawl back up (mis-stepping); with `$discontinuity(0)`
//! announced at the jump, `prev_h` resets (`analyses/transient.rs`) and
//! the next TR stage degrades to backward Euler instead of re-deriving
//! across the jump — the controller sails through at the full step size.
//! This is exactly `TrBdf2::stage_coeffs`' own documented restart
//! convention (`math/integration.rs`), exercised here end-to-end instead
//! of at the coefficient-formula level alone.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const MAX_DT: f64 = 0.01;

fn fixture(with_discontinuity: bool) -> String {
    let disc_stmt = if with_discontinuity { "if ($abstime >= 0.5) $discontinuity(0);" } else { "" };
    format!(
        r#"
    module vsrc (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ ($abstime >= 0.5) ? 1.0 : 0.0;
            {disc_stmt}
        end
    endmodule

    module capacitor (inout electrical p, inout electrical n);
        parameter real c = 1e-6;
        analog begin
            I(p, n) <+ ddt(c * V(p, n));
        end
    endmodule

    module top ();
        electrical vout, gnd;
        vsrc v1 (vout, gnd);
        capacitor c1 (vout, gnd);
    endmodule
    "#
    )
}

/// Runs the transient, returning `(time, dt-to-next-step)` pairs for every
/// recorded step landing in `[0.5, 0.6)` — the window right after the jump.
fn post_jump_step_sizes(with_discontinuity: bool) -> Vec<f64> {
    let src = fixture(with_discontinuity);
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("top").expect("compiles");
    let opts = TransientAnalysisOptions::new(1.0, MAX_DT);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    let result = tran.solve().expect("transient completes");
    let times: Vec<f64> = result.iter().map(|s| s.time()).filter(|&t| (0.5..0.6).contains(&t)).collect();
    times.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn discontinuity_prevents_step_control_mis_stepping_across_the_jump() {
    let with = post_jump_step_sizes(true);
    let without = post_jump_step_sizes(false);

    assert!(with.len() >= 2, "with-discontinuity run should record several post-jump steps: {with:?}");
    assert!(without.len() >= 2, "without-discontinuity run should record several post-jump steps: {without:?}");

    // With $discontinuity: the controller is undisturbed — every post-jump
    // step stays at (or very near) the max step size.
    let with_first = with[0];
    assert!(
        (with_first - MAX_DT).abs() < MAX_DT * 0.1,
        "with $discontinuity, the first post-jump step should stay near max_dt={MAX_DT}, got {with_first}: {with:?}"
    );

    // Without $discontinuity: the stale previous-derivative term produces
    // an LTE spike right after the jump, forcing the controller to shrink
    // the step well below max_dt (mis-stepping) before it recovers.
    let without_first = without[0];
    assert!(
        without_first < MAX_DT * 0.5,
        "without $discontinuity, the first post-jump step should mis-step (shrink well below \
         max_dt={MAX_DT}), got {without_first}: {without:?}"
    );

    assert!(
        with_first > without_first * 2.0,
        "$discontinuity's first post-jump step ({with_first}) should be markedly larger than \
         the undeclared run's ({without_first}) — the numerical effect this task adds"
    );
}

#[test]
fn discontinuity_call_compiles_without_error() {
    let src = fixture(true);
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let kernel = piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["vsrc"]).expect("compiles");
    assert!(kernel.has_discontinuity(), "the compiled kernel reports a live $discontinuity flag");
}
