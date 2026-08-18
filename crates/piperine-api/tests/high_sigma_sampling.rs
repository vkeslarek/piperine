//! dv-gradients T20 (DVG-20) — the high-sigma policy: mean-shift
//! importance sampling over `SamplePlan`'s restamp loop (T13), the third
//! policy of the one `Driver` (T16). Covers: every yield travels with its
//! estimator and confidence interval (AC2), a tail sample is replayable
//! from `(seed, index)` (AC3), and — on a bitcell-shaped fixture with a
//! tight margin — the estimate agrees with a long plain Monte Carlo
//! within the reported interval (AC4, the independent test criterion).
//!
//! **Migrated** (random-variable-catalogue): the fixture runs against the
//! live VAMS `tol gauss(sigma_rel = …)` parameter-distribution surface
//! and the in-module `constraint begin … end` producer; the
//! importance-sampling estimator machinery (`SamplePlan::draws_importance`
//! / `apply_importance`, `Policy::HighSigma`) is live in
//! `crates/piperine-api/src/sampling.rs` and `driver/high_sigma.rs`.

use piperine_api::prelude::CheckPosture;
use piperine_api::{Driver, DriverOutcome, HighSigmaEstimator, HighSigmaSpec, Policy, SamplePlan, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A bitcell-shaped fixture (design §T20's own allowance: "two
/// back-to-back inverters OR a simple ... fixture with a tight margin" —
/// this is the latter): a resistive divider standing in for a storage
/// node's read path, with a mismatch `tol` on the pull-up leg and a
/// `require` a few σ away from nominal — the tight-margin shape that
/// makes plain Monte Carlo expensive and importance sampling worth it.
/// `r1 = 1e3 · gauss(sigma_rel = 0.1)` puts `σ(V(out)) ≈ 0.0227` around
/// nominal `V(out) = 0.5`; `hi = 0.55` sits ≈ 2.2σ above nominal, so the
/// `head` failure (`V(out) > hi`) is a genuine TAIL event (a few percent
/// under plain sampling) — small enough that importance sampling with a
/// mean shift toward low `r1` (which raises `V(out)`) meaningfully
/// concentrates draws in the failure region.
const BITCELL: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3 tol gauss(sigma_rel = 0.1);
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

fn design(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// AC1/AC2: the high-sigma policy runs as `Driver`'s third policy and its
/// yield NEVER travels without an estimator name and a confidence
/// interval — the interval brackets the point estimate by construction
/// (both are read off the same Wald interval on the underlying failure
/// probability), and the estimator is always the named mean-shift method.
#[test]
fn high_sigma_yield_always_carries_its_estimator_and_a_confidence_interval() {
    let bitcell_design = design(BITCELL);
    let mut session = Session::compile(&bitcell_design, "Top").expect("session compiles");

    let spec = HighSigmaSpec { n: 300, seed: 11, shift_sigmas: -2.5, confidence: 0.95 };
    let driver = Driver::new(Policy::HighSigma(spec));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("high-sigma runs");
    let DriverOutcome::HighSigma(result) = outcome else { panic!("expected HighSigma outcome") };

    assert_eq!(
        result.estimator,
        HighSigmaEstimator::MeanShiftImportanceSampling,
        "DVG-20 AC2: the result names the estimator that produced it"
    );
    assert_eq!(result.confidence, 0.95);
    let (lo, hi) = result.ci;
    assert!(lo <= hi, "the confidence interval is ordered: lo={lo} <= hi={hi}");
    assert!(
        lo <= result.yield_estimate && result.yield_estimate <= hi,
        "DVG-20 AC2: the point estimate must lie within its own reported interval: \
         yield={} ci=({lo}, {hi})",
        result.yield_estimate
    );
    assert!((0.0..=1.0).contains(&result.yield_estimate), "yield is a fraction");
    assert!(lo >= 0.0 && hi <= 1.0, "the interval is clamped to a valid yield range");
}

/// AC3: a tail sample (one of the reported failing draws) is replayable
/// from `(seed, index)` — the same importance-shifted draw reproduces
/// byte-identical values, and reapplying it to a fresh session reproduces
/// the same margin violation.
#[test]
fn a_tail_sample_is_replayable_from_seed_and_index() {
    let bitcell_design = design(BITCELL);
    let mut session = Session::compile(&bitcell_design, "Top").expect("session compiles");

    let spec = HighSigmaSpec { n: 200, seed: 42, shift_sigmas: -3.0, confidence: 0.95 };
    let driver = Driver::new(Policy::HighSigma(spec.clone()));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("high-sigma runs");
    let DriverOutcome::HighSigma(result) = outcome else { panic!("expected HighSigma outcome") };

    assert!(
        !result.failing_indices.is_empty(),
        "a -3sigma shift toward the failure direction must produce at least one failing draw \
         within 200 samples"
    );
    let tail_index = result.failing_indices[0];

    // Replay: an independent plan/session, driven by ONLY (seed, index,
    // shift_sigmas) — the campaign parameters, not internal driver state.
    let plan_a = SamplePlan::collect(&bitcell_design, "Top").expect("plan collects");
    let draw_a = plan_a.draws_importance(spec.seed, tail_index, spec.shift_sigmas);

    let bitcell_design_b = design(BITCELL);
    let plan_b = SamplePlan::collect(&bitcell_design_b, "Top").expect("replay plan collects");
    let draw_b = plan_b.draws_importance(spec.seed, tail_index, spec.shift_sigmas);

    assert_eq!(draw_a.draws, draw_b.draws, "(seed, index, shift_sigmas) reproduces byte-identical draws");
    assert_eq!(draw_a.weight.to_bits(), draw_b.weight.to_bits(), "the importance weight replays identically");

    // Applying the replayed draw to a fresh session reproduces the same
    // margin violation the original run counted as a failure.
    let mut replay_session = Session::compile(&bitcell_design_b, "Top").expect("replay session compiles");
    plan_b
        .apply_importance(&mut replay_session, spec.seed, tail_index, spec.shift_sigmas)
        .expect("replay applies");
    let replay_op = replay_session.op(&collect_config(), None).expect("replay op solves");
    assert!(
        !replay_op.requires_ok(),
        "replaying a reported failing sample must reproduce its margin violation"
    );
}

/// AC4 (independent test criterion): on the bitcell-shaped fixture, the
/// importance-sampling estimate agrees with a long plain Monte Carlo
/// within the reported confidence interval.
#[test]
fn importance_sampling_agrees_with_a_long_plain_monte_carlo_within_the_reported_interval() {
    let bitcell_design = design(BITCELL);
    let mut session = Session::compile(&bitcell_design, "Top").expect("session compiles");
    let config = collect_config();

    // The long plain oracle.
    let plan = SamplePlan::collect(&bitcell_design, "Top").expect("plan collects");
    let plain_mc = plan.monte_carlo(&mut session, 20_000, 999, &config).expect("plain monte carlo runs");
    let plain_yield = plain_mc.yield_fraction();
    assert!(
        plain_yield < 1.0 && plain_yield > 0.9,
        "sanity: the tight margin must be a genuine (rare) tail event, got plain yield={plain_yield}"
    );

    // The importance-sampling estimate, shifted toward low r1 (which
    // raises V(out) toward and past `hi`).
    let spec = HighSigmaSpec { n: 400, seed: 5, shift_sigmas: -2.5, confidence: 0.95 };
    let driver = Driver::new(Policy::HighSigma(spec));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("high-sigma runs");
    let DriverOutcome::HighSigma(result) = outcome else { panic!("expected HighSigma outcome") };

    let (lo, hi) = result.ci;
    assert!(
        plain_yield >= lo && plain_yield <= hi,
        "the importance-sampling estimate must agree with the long plain Monte Carlo within its \
         reported interval: plain_yield={plain_yield} is_yield={} ci=({lo}, {hi})",
        result.yield_estimate
    );

    // Post-run nominal analysis sees no residue, same discipline T14/T15
    // established for the ordinary Monte Carlo path.
    plan.reset(&mut session).expect("reset applies nominal values");
    let nominal = session.op(&config, None).expect("nominal op solves after a high-sigma run");
    assert!(nominal.requires_ok(), "the nominal design (r=1e3, hi=0.55) satisfies `head`");
}
