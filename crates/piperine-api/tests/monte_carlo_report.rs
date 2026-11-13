//! dv-gradients T15 (DVG-18) — `SamplePlan::monte_carlo`: the reporting
//! surface built on `SamplePlan` (T13/T14): yield, per-metric `σ_i`, the
//! worst sample per constraint (named and replayable), the compile-once
//! proof for the whole sampling loop (MD-18), and non-convergent samples
//! reported as outcomes rather than aborting the run.
//!
//! **Migrated** (random-variable-catalogue): the fixtures run against the
//! live VAMS `tol gauss(…)`/`tol uniform(…)` parameter-distribution
//! surface and the in-module `constraint begin … end` producer — `tol`/
//! `global` are contextual keywords and the keyed args (`sigma_rel`,
//! `rel`) are const-evaluated in module scope.
//!

use piperine_api::prelude::CheckPosture;
use piperine_api::{MonteCarloResult, SampleOutcome, SamplePlan, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A resistive divider with a `require` and a mismatch `tol` on `r1`:
/// nominal `V(out) = dc·1e3/(r1+1e3) = 0.5 V` (dc=1, r1=1e3), and
/// `require head : V(out) <= hi` (hi=0.55) has margin `hi - V(out)`,
/// nominally satisfied (0.05) but reachable-negative for a low-enough
/// `r1` draw — a fixture with both passing and failing samples.
const DIVIDER_TOL: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3 tol gauss(sigma_rel = 0.3);
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.55;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// A fixture that reaches non-convergence for roughly a quarter of its
/// draws: `r` is uniform over `[-1e3, 3e3]` (default 1e3, `rel = 2.0`),
/// so a negative draw feeds `sqrt(r)` a negative argument, producing
/// `NaN` and a solve that fails to converge — a real per-sample failure,
/// not a contrived error path.
const NONCONVERGENT: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3 tol uniform(rel = 2.0);
    analog begin
        I(p, n) <+ V(p, n) / sqrt(r);
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.99;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

fn design(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// DVG-18: `yield_fraction` counts the fraction of samples with every
/// margin ≥ 0. The divider fixture's `head` constraint fails when `r1`
/// draws low; scanning a run of 100 samples for seed 7 must produce a
/// yield strictly between 0 and 1 (some pass, some fail — not a
/// trivially all-pass or all-fail run), and the count must match a
/// hand-count of `head`'s sign over the same outcomes.
#[test]
fn yield_fraction_is_the_share_of_samples_with_every_margin_nonnegative() {
    let divider_design = design(DIVIDER_TOL);
    let mut session = Session::compile(&divider_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&divider_design, "Top").expect("plan collects");
    let config = collect_config();

    let result = plan.monte_carlo(&mut session, 200, 7, &config).expect("monte carlo runs");

    let hand_count = result
        .outcomes
        .iter()
        .filter(|o| match o {
            SampleOutcome::Converged { margins } => margins.values().all(|&m| m >= 0.0),
            SampleOutcome::Failed { .. } => false,
        })
        .count();
    assert_eq!(
        result.yield_fraction(),
        hand_count as f64 / 200.0,
        "yield_fraction matches a hand-count of all-margins-nonnegative outcomes"
    );
    assert!(
        result.yield_fraction() > 0.0 && result.yield_fraction() < 1.0,
        "the fixture's sigma_rel=0.3 tol must produce a mixed run (both passes and failures), \
         got yield={}",
        result.yield_fraction()
    );
}

/// DVG-18: `sigma("dut1.head")` reports the spread of the constraint's
/// scalar margin value across the converged samples — matching a
/// hand-computed population standard deviation of the same values.
#[test]
fn sigma_reports_the_spread_of_a_constraints_margin_value() {
    let divider_design = design(DIVIDER_TOL);
    let mut session = Session::compile(&divider_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&divider_design, "Top").expect("plan collects");
    let config = collect_config();

    let result = plan.monte_carlo(&mut session, 100, 11, &config).expect("monte carlo runs");

    let values: Vec<f64> = result
        .outcomes
        .iter()
        .filter_map(|o| match o {
            SampleOutcome::Converged { margins } => margins.get("dut1.head").copied(),
            SampleOutcome::Failed { .. } => None,
        })
        .collect();
    assert!(values.len() > 50, "the fixture always converges, expected ~100 converged samples");
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let hand_sigma =
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt();

    let sigma = result.sigma("dut1.head").expect("sigma is reported for a constraint with samples");
    assert!(
        (sigma - hand_sigma).abs() < 1e-9,
        "sigma({}) should match the hand-computed population std dev ({hand_sigma})",
        sigma
    );
    assert!(sigma > 0.0, "a tol-driven constraint has nonzero spread");
}

/// DVG-18: the worst sample per constraint is named (its index) and
/// replayable (T14) — applying `(seed, worst_index)` through a fresh
/// `SamplePlan`/`Session` reproduces the exact worst margin value found
/// during the run.
#[test]
fn the_worst_sample_is_named_and_replayable() {
    let divider_design = design(DIVIDER_TOL);
    let mut session = Session::compile(&divider_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&divider_design, "Top").expect("plan collects");
    let config = collect_config();
    let seed = 23u64;

    let result = plan.monte_carlo(&mut session, 150, seed, &config).expect("monte carlo runs");
    let (worst_index, worst_value) =
        result.worst("dut1.head").expect("a worst sample exists for `dut1.head`");

    // It really is the minimum across every converged sample.
    for outcome in &result.outcomes {
        if let SampleOutcome::Converged { margins } = outcome
            && let Some(&v) = margins.get("dut1.head")
        {
            assert!(v >= worst_value, "worst() must report the minimum margin value, found {v} < {worst_value}");
        }
    }

    // Replay: an independent session/plan, driven by ONLY (seed, worst_index).
    let mut replay = Session::compile(&divider_design, "Top").expect("replay compiles");
    let replay_plan = SamplePlan::collect(&design(DIVIDER_TOL), "Top").expect("replay plan collects");
    replay_plan.apply(&mut replay, seed, worst_index).expect("replay applies");
    let replay_op = replay.op(&config, None).expect("replay op solves");
    let replay_value = replay_op.margins().get("head").expect("head margin").scalar();

    assert_eq!(
        replay_value.to_bits(),
        worst_value.to_bits(),
        "replaying (seed={seed}, index={worst_index}) must reproduce the exact worst margin value"
    );
}

/// Edge case (spec + DVG-18): a sampled draw that makes the circuit fail
/// to converge is reported as a sample OUTCOME, not an analysis crash —
/// the run continues past it and every other sample is still reported.
#[test]
fn a_non_convergent_sample_is_reported_as_an_outcome_and_the_run_continues() {
    let nonconvergent_design = design(NONCONVERGENT);
    let mut session = Session::compile(&nonconvergent_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&nonconvergent_design, "Top").expect("plan collects");
    let config = collect_config();

    let result: MonteCarloResult =
        plan.monte_carlo(&mut session, 60, 3, &config).expect("monte carlo run does not abort");

    assert_eq!(result.outcomes.len(), 60, "the run reports all 60 samples, none skipped");
    let failed = result.failed_indices();
    assert!(
        !failed.is_empty(),
        "the fixture's uniform(rel=2.0) tol on `r` must produce at least one negative \
         draw (sqrt(negative) = NaN, non-convergent) within 60 samples"
    );
    // Failed samples are distinguishable from converged ones and excluded
    // from yield's numerator (they are not a pass).
    for &i in &failed {
        assert!(matches!(result.outcomes[i], SampleOutcome::Failed { .. }));
    }
    assert!(
        result.yield_fraction() < 1.0,
        "at least one sample failed to converge, so yield must be < 1.0"
    );

    // Post-run nominal analysis sees no residue (DVG-18 AC5, carried from
    // T14) even after a run containing failures.
    plan.reset(&mut session).expect("reset applies nominal values");
    let nominal = session.op(&config, None).expect("nominal op solves after a run with failures");
    assert!(nominal.requires_ok(), "the nominal design (r=1e3 > 0) satisfies `head`");
}
