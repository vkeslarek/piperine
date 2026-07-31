//! Monte Carlo reporting (dv-gradients T15, DVG-18): the yield, per-metric
//! spread, and worst-sample surface built on [`SamplePlan`] (T13/T14).
//!
//! [`SamplePlan::monte_carlo`] restamps `n` samples on the ALREADY-compiled
//! session (MD-18: one compile serves the whole run — a 10³-sample run is
//! 10³ restamps, never 10³ elaborations) and reads each sample's margins
//! channel (`dv-core`) to report:
//!
//! - **yield**: the fraction of samples with every margin ≥ 0
//! - **σ_i**: the standard deviation of each constraint's scalar margin
//!   value across the samples that converged
//! - the **worst sample per constraint**, named by its index so it is
//!   replayable ([`SamplePlan::apply`], T14) via `(seed, index)`
//!
//! A sample that fails to converge is recorded as [`SampleOutcome::Failed`]
//! — the run continues; it is a per-sample outcome, never a crash that
//! aborts the whole run (spec edge case). The session is reset to nominal
//! ([`SamplePlan::reset`]) before `monte_carlo` returns, so a subsequent
//! nominal analysis sees no residue (DVG-18 AC5's "no residue" carried
//! forward from T14).

use std::collections::HashMap;

use crate::error::Error;
use crate::sampling::SamplePlan;
use crate::session::{Session, SolverConfig};

/// One sample's outcome (DVG-18 edge case: a draw that makes the circuit
/// fail to converge is a reported outcome, not an analysis crash).
#[derive(Debug, Clone)]
pub enum SampleOutcome {
    /// The sample converged: every declared constraint's scalar margin
    /// value, keyed the same way [`crate::MarginsResult::get`] resolves a
    /// qualified name (`"instance_path.label"`).
    Converged { margins: HashMap<String, f64> },
    /// The sample failed to converge (or otherwise errored while solving)
    /// — the run continues past it; this outcome names why.
    Failed { error: String },
}

impl SampleOutcome {
    /// `true` for [`SampleOutcome::Converged`].
    pub fn is_converged(&self) -> bool {
        matches!(self, SampleOutcome::Converged { .. })
    }
}

/// One `monte_carlo` run's report (T15, DVG-18): the seed and sample count
/// that produced it, and every sample's outcome in draw order.
#[derive(Debug, Clone)]
pub struct MonteCarloResult {
    /// The seed the run was drawn from — combined with an outcome's index,
    /// replayable via [`SamplePlan::apply`].
    pub seed: u64,
    /// The number of samples requested (== `outcomes.len()`).
    pub n: usize,
    /// Per-sample outcome, index-ordered: `outcomes[i]` is sample `i`.
    pub outcomes: Vec<SampleOutcome>,
}

impl MonteCarloResult {
    /// Yield (DVG-18): the fraction of samples with every reported margin
    /// ≥ 0. A sample that failed to converge counts against yield — it is
    /// not a pass.
    pub fn yield_fraction(&self) -> f64 {
        if self.outcomes.is_empty() {
            return 0.0;
        }
        let passing = self
            .outcomes
            .iter()
            .filter(|o| match o {
                SampleOutcome::Converged { margins } => margins.values().all(|&m| m >= 0.0),
                SampleOutcome::Failed { .. } => false,
            })
            .count();
        passing as f64 / self.outcomes.len() as f64
    }

    /// σ_i (DVG-18): the population standard deviation of `name`'s scalar
    /// margin value across the CONVERGED samples that reported it. `None`
    /// when fewer than two converged samples reported the constraint (a
    /// spread needs at least two points).
    pub fn sigma(&self, name: &str) -> Option<f64> {
        let values: Vec<f64> = self
            .outcomes
            .iter()
            .filter_map(|o| match o {
                SampleOutcome::Converged { margins } => margins.get(name).copied(),
                SampleOutcome::Failed { .. } => None,
            })
            .collect();
        if values.len() < 2 {
            return None;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        Some(var.sqrt())
    }

    /// The worst (minimum) converged sample for constraint `name`: its
    /// index — replayable via `SamplePlan::apply(session, self.seed,
    /// index)` (T14) — and its scalar margin value. `None` when no
    /// converged sample reported the constraint.
    pub fn worst(&self, name: &str) -> Option<(usize, f64)> {
        self.outcomes
            .iter()
            .enumerate()
            .filter_map(|(i, o)| match o {
                SampleOutcome::Converged { margins } => margins.get(name).map(|&v| (i, v)),
                SampleOutcome::Failed { .. } => None,
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).expect("margin values are finite"))
    }

    /// The indices of every sample that failed to converge — a reader
    /// checks this before trusting `yield_fraction`/`sigma`/`worst` blindly
    /// on a run with non-convergent samples.
    pub fn failed_indices(&self) -> Vec<usize> {
        self.outcomes
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.is_converged())
            .map(|(i, _)| i)
            .collect()
    }
}

impl SamplePlan {
    /// Run `n` Monte Carlo samples (T15, DVG-18) over `session`'s ALREADY
    /// compiled circuit — MD-18: restamps only, zero recompiles across the
    /// whole run regardless of `n`. Each sample applies its `(seed, index)`
    /// draws ([`SamplePlan::apply`]), solves an operating point, and
    /// records its margins; a sample that fails to converge is recorded as
    /// [`SampleOutcome::Failed`] and the run continues (never a crash that
    /// aborts the whole run). `session` is reset to nominal
    /// ([`SamplePlan::reset`]) before this returns, win or lose, so a
    /// subsequent nominal analysis sees no residue.
    pub fn monte_carlo(
        &self,
        session: &mut Session,
        n: usize,
        seed: u64,
        config: &SolverConfig,
    ) -> Result<MonteCarloResult, Error> {
        let mut outcomes = Vec::with_capacity(n);
        for index in 0..n {
            self.apply(session, seed, index)?;
            match session.op(config, None) {
                Ok(op) => {
                    let margins = op
                        .margins()
                        .entries()
                        .iter()
                        .map(|e| (format!("{}.{}", e.instance_path, e.label), e.scalar()))
                        .collect();
                    outcomes.push(SampleOutcome::Converged { margins });
                }
                Err(e) => outcomes.push(SampleOutcome::Failed { error: e.to_string() }),
            }
        }
        self.reset(session)?;
        Ok(MonteCarloResult { seed, n, outcomes })
    }
}
