//! The high-sigma policy (dv-gradients T20, DVG-20): a 5–6σ yield estimate
//! without 10⁸ plain-Monte-Carlo samples, via IMPORTANCE SAMPLING over
//! [`crate::SamplePlan`]'s restamp loop (T13).
//!
//! # Estimator choice: mean-shift, not statistical blockade
//!
//! The spec's own wording (DVG-20 AC1) offers a choice: "statistical
//! blockade or scaled-sigma". Statistical blockade trains a classifier
//! (an SVM boundary, typically) on an initial sample batch to identify
//! the tail region before biasing further sampling toward it — extra
//! machinery, an extra dependency surface, and its own approximation
//! error on top of the sampling error this delivery already has to
//! report honestly. A scaled-sigma MEAN SHIFT is the textbook importance
//! sampling estimator: shift the sampling distribution's mean toward the
//! failure region by a caller-named `shift_sigmas`, weight each drawn
//! sample by the likelihood ratio between the ORIGINAL and the SHIFTED
//! density, and the resulting weighted estimator is unbiased for the
//! ORIGINAL failure probability by construction — no classifier, no
//! extra approximation, and a closed-form variance for the reported
//! confidence interval. This delivery implements the mean shift.
//!
//! # What gets shifted (SPEC_DEVIATION, documented)
//!
//! [`crate::SamplePlan::draws_importance`] shifts only `gauss`-declared
//! entries with a resolvable `σ` (`sigma`/`sigma_rel`); a `uniform`
//! declaration draws unshifted (likelihood ratio `1`). A design with only
//! Gaussian `tol` declarations gets the full benefit; a uniform-toleranced
//! parameter still gets an unbiased (if less variance-reduced) draw. A
//! full multivariate/any-distribution shift is future work, not required
//! for a correct estimator today.
//!
//! # Every yield travels with its estimator and confidence interval (DVG-20 AC2)
//!
//! [`HighSigmaResult`] never reports a bare number: [`HighSigmaResult::estimator`]
//! names the method and [`HighSigmaResult::ci`] carries the interval — a
//! yield without its estimator is not a yield, per the spec's own words.

use crate::error::Error;
use crate::sampling::SamplePlan;
use crate::session::{Session, SolverConfig};

/// Which importance-sampling estimator produced a [`HighSigmaResult`]
/// (DVG-20 AC2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighSigmaEstimator {
    /// Mean-shift importance sampling over `gauss`-declared `tol` entries
    /// (see the module docs for why this was chosen over statistical
    /// blockade) — the only estimator this delivery implements.
    MeanShiftImportanceSampling,
}

impl HighSigmaEstimator {
    /// The stable, host-visible name (Python and Rust agree — MD-22).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MeanShiftImportanceSampling => "mean-shift-importance-sampling",
        }
    }
}

impl std::fmt::Display for HighSigmaEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The high-sigma policy's request (T20, DVG-20).
#[derive(Debug, Clone)]
pub struct HighSigmaSpec {
    /// Samples to draw from the SHIFTED distribution.
    pub n: usize,
    /// The seed the importance-sampled draws are drawn from — combined
    /// with a sample's index, replayable via
    /// [`crate::SamplePlan::apply_importance`] (DVG-20 AC3).
    pub seed: u64,
    /// How many σ to shift every `gauss`-declared entry's sampling mean
    /// by, toward the failure region the caller names (positive shifts
    /// up, negative shifts down — the caller must know which direction
    /// the design's tail failure lies in; this delivery does not infer
    /// it automatically).
    pub shift_sigmas: f64,
    /// The two-sided confidence level for [`HighSigmaResult::ci`]
    /// (e.g. `0.95`). Supported exactly: `0.90`, `0.95`, `0.99`; any
    /// other value falls back to the `0.95` z-score (documented on
    /// [`HighSigmaResult::confidence`], which always reports the value
    /// actually requested).
    pub confidence: f64,
}

impl Default for HighSigmaSpec {
    fn default() -> Self {
        Self { n: 200, seed: 1, shift_sigmas: -3.0, confidence: 0.95 }
    }
}

/// The high-sigma policy's result (T20, DVG-20): a yield estimate that
/// NEVER travels without its estimator and confidence interval.
#[derive(Debug, Clone)]
pub struct HighSigmaResult {
    /// The importance-sampling yield estimate (fraction of the ORIGINAL,
    /// unshifted distribution's mass with every watched `require` margin
    /// ≥ 0).
    pub yield_estimate: f64,
    /// The two-sided confidence interval on [`Self::yield_estimate`], at
    /// [`Self::confidence`].
    pub ci: (f64, f64),
    /// Which estimator produced this result (DVG-20 AC2) — always present,
    /// never inferred by the caller.
    pub estimator: HighSigmaEstimator,
    /// The confidence level [`Self::ci`] was built at.
    pub confidence: f64,
    /// Samples drawn.
    pub n: usize,
    /// The seed the run was drawn from.
    pub seed: u64,
    /// The mean shift (in σ) applied to every shiftable entry.
    pub shift_sigmas: f64,
    /// Indices of samples that were a FAILURE under the shifted draw
    /// (a watched `require` violated, or the sample did not converge) —
    /// each replayable via `SamplePlan::apply_importance(session, seed,
    /// index, shift_sigmas)` (DVG-20 AC3): a tail sample is never just a
    /// number, it is a reproducible design point.
    pub failing_indices: Vec<usize>,
}

/// Run the high-sigma policy: [`super::Driver::run`]'s dispatch target
/// for [`super::Policy::HighSigma`]. `config` always carries `collect`
/// posture (forced by [`super::Driver::run`]).
pub(super) fn run(
    session: &mut Session,
    spec: &HighSigmaSpec,
    config: &SolverConfig,
) -> Result<HighSigmaResult, Error> {
    let top = session.module().to_string();
    let design = session.design().clone();
    let plan = SamplePlan::collect(&design, &top)?;
    if plan.is_empty() {
        return Err(Error::Measurement(
            "high-sigma: the design declares no `tol` distributions to sample".to_string(),
        ));
    }

    let n = spec.n.max(1);
    let mut terms = Vec::with_capacity(n);
    let mut failing_indices = Vec::new();
    for index in 0..n {
        let weight = plan.apply_importance(session, spec.seed, index, spec.shift_sigmas)?;
        let failed = match session.op(config, None) {
            Ok(op) => !op.requires_ok(),
            // A non-convergent draw is a sample OUTCOME (a failure), not
            // a crash — the run continues (mirrors T15's monte_carlo
            // edge case).
            Err(_) => true,
        };
        if failed {
            failing_indices.push(index);
        }
        terms.push(if failed { weight } else { 0.0 });
    }
    plan.reset(session)?;

    let nf = n as f64;
    let p_hat = terms.iter().sum::<f64>() / nf;
    let variance = terms.iter().map(|t| (t - p_hat).powi(2)).sum::<f64>() / nf;
    let standard_error = (variance / nf).sqrt();
    let z = two_sided_z_score(spec.confidence);
    let p_lo = (p_hat - z * standard_error).clamp(0.0, 1.0);
    let p_hi = (p_hat + z * standard_error).clamp(0.0, 1.0);

    Ok(HighSigmaResult {
        yield_estimate: 1.0 - p_hat,
        ci: (1.0 - p_hi, 1.0 - p_lo),
        estimator: HighSigmaEstimator::MeanShiftImportanceSampling,
        confidence: spec.confidence,
        n,
        seed: spec.seed,
        shift_sigmas: spec.shift_sigmas,
        failing_indices,
    })
}

/// The two-sided z-score for a two-sided normal confidence interval at
/// `confidence` — a small closed-form table for the levels this
/// delivery's callers use (not a general inverse-normal-CDF
/// implementation). Falls back to the 95% value for anything else.
fn two_sided_z_score(confidence: f64) -> f64 {
    if (confidence - 0.90).abs() < 1e-9 {
        1.644_853_626_951_472_5
    } else if (confidence - 0.99).abs() < 1e-9 {
        2.575_829_303_548_901
    } else {
        1.959_963_984_540_054
    }
}
