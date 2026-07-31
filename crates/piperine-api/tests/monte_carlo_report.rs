//! dv-gradients T15 (DVG-18) — `SamplePlan::monte_carlo`: the reporting
//! surface built on `SamplePlan` (T13/T14): yield, per-metric `σ_i`, the
//! worst sample per constraint (named and replayable), the compile-once
//! proof for the whole sampling loop (MD-18), and non-convergent samples
//! reported as outcomes rather than aborting the run.

use piperine_api::prelude::CheckPosture;
use piperine_api::{MonteCarloResult, SampleOutcome, SamplePlan, Session, SolverConfig};
use piperine_lang::SourceMap;

/// A resistive divider with a `require` and a mismatch `tol` on `r1`:
/// nominal `V(out) = dc·1e3/(r1+1e3) = 0.5 V` (dc=1, r1=1e3), and
/// `require head : V(out) <= hi` (hi=0.55) has margin `hi - V(out)`,
/// nominally satisfied (0.05) but reachable-negative for a low-enough
/// `r1` draw — a fixture with both passing and failing samples.
const DIVIDER_TOL_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3 tol gauss(sigma_rel = 0.3);
}
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.55; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require head : V(p, n) <= hi; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

/// A fixture that reaches non-convergence for roughly a quarter of its
/// draws: `r` is uniform over `[-1e3, 3e3]` (default 1e3, `rel = 2.0`),
/// so a negative draw feeds `sqrt(r)` a negative argument, producing
/// `NaN` and a solve that fails to converge — a real per-sample failure,
/// not a contrived error path.
const NONCONVERGENT_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3 tol uniform(rel = 2.0);
}
analog R { I(p, n) <+ V(p, n) / sqrt(r); }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.99; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require head : V(p, n) <= hi; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

fn design(phdl: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(phdl, &SourceMap::dummy()).expect("elaborates")
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
    let phdl_design = design(DIVIDER_TOL_PHDL);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&phdl_design, "Top").expect("plan collects");
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
    let phdl_design = design(DIVIDER_TOL_PHDL);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&phdl_design, "Top").expect("plan collects");
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
    let phdl_design = design(DIVIDER_TOL_PHDL);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&phdl_design, "Top").expect("plan collects");
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
    let mut replay = Session::compile(&phdl_design, "Top").expect("replay compiles");
    let replay_plan = SamplePlan::collect(&design(DIVIDER_TOL_PHDL), "Top").expect("replay plan collects");
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
    let phdl_design = design(NONCONVERGENT_PHDL);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&phdl_design, "Top").expect("plan collects");
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
