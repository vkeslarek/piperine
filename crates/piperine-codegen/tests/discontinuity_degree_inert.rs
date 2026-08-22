//! LRM-COVERS: 9.17.1
//! LRMW2-38 (`lrm-conformance-wave2`, T42): `$discontinuity(degree)`'s
//! argument (§9.17.1: `$discontinuity(i)` "implies a discontinuity in the
//! _i_'th derivative of the constitutive equation ... `$discontinuity(0)`
//! indicates a discontinuity in the equation, `$discontinuity(1)`
//! indicates a discontinuity in its slope, etc.") is accepted and stored
//! at the call site (`flatten/analog.rs`'s `discontinuity` arm: `let _ =
//! args;`) but never read again — every fire folds into the same
//! fired-this-evaluation flag regardless of `degree`
//! (`kernel/analog/compile.rs`), and the solver's own consumer
//! (`analyses/transient.rs`) applies one uniform response to any fire at
//! all: `prev_h = 0.0`, dropping the next TR stage to backward Euler.
//!
//! Re-verification + disclosure (this task's explicit escape hatch): the
//! LRM's own words for `$discontinuity` are "This task does not directly
//! specify the behavior of the module" (§9.17.1) — a simulator-quality
//! hint, not a mandated per-degree algorithm. `TrBdf2`
//! (`piperine-solver/src/math/integration.rs`) has exactly one restart
//! rule; it does not have a family of degree-indexed responses to select
//! among. This target proves — rather than fakes — that outcome: two
//! different `degree` values produce the **identical** post-jump step
//! pattern, pinning today's genuinely degree-invariant behavior as a
//! disclosed row (not a bug this task leaves unfixed, and not a
//! regression a future change should reintroduce silently).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const MAX_DT: f64 = 0.01;

fn fixture(degree: i64) -> String {
    format!(
        r#"
    module vsrc (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ ($abstime >= 0.5) ? 1.0 : 0.0;
            if ($abstime >= 0.5) $discontinuity({degree});
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

/// The recorded step times landing in `[0.5, 0.6)` — the window right
/// after the jump — for a given `$discontinuity` degree.
fn post_jump_step_times(degree: i64) -> Vec<f64> {
    let src = fixture(degree);
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("top").expect("compiles");
    let opts = TransientAnalysisOptions::new(1.0, MAX_DT);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    let result = tran.solve().expect("transient completes");
    result.iter().map(|s| s.time()).filter(|&t| (0.5..0.6).contains(&t)).collect()
}

/// Pins today's genuinely degree-invariant behavior: `$discontinuity(0)`
/// (an equation-value jump) and `$discontinuity(5)` (a 5th-derivative
/// jump — a categorically different, much milder discontinuity per the
/// LRM's own reading) drive the identical step-time sequence, because the
/// integrator has exactly one restart response and does not branch on
/// `degree` at all.
#[test]
fn discontinuity_degree_does_not_change_the_step_pattern() {
    let degree_0 = post_jump_step_times(0);
    let degree_5 = post_jump_step_times(5);
    assert!(degree_0.len() >= 2, "degree=0 run should record several post-jump steps: {degree_0:?}");
    assert_eq!(
        degree_0, degree_5,
        "degree is not consulted anywhere downstream of the call site — every $discontinuity fire \
         gets the identical restart response regardless of degree, so the step pattern must match \
         exactly (disclosed, not a regression this task introduces)"
    );
}

/// A negative-friendly sanity check for the same finding, from the other
/// direction: a `degree` this codebase's flattener does not special-case
/// (LRM's `-1`, the `$limit` convergence-refusal spelling, §9.17.3) still
/// compiles and fires exactly like any other degree — it is accepted, not
/// silently dropped, and not treated specially either.
#[test]
fn discontinuity_negative_one_degree_compiles_and_fires_like_any_other() {
    let degree_neg1 = post_jump_step_times(-1);
    let degree_0 = post_jump_step_times(0);
    assert_eq!(degree_neg1, degree_0, "degree=-1 must behave identically to degree=0 (both ignored downstream)");
}
