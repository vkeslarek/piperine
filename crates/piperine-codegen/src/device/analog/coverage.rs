//! `cover` bin mapping (device side, dv-tester T33, DVT-40/41): at each
//! accepted point the kernel's `cover` functions are evaluated and each
//! result is mapped into a bin index — a counter increment, **no** signed
//! distance and **no** argmin (D9's whole point: this is intentionally
//! simpler than [`super::constraints::MarginTracker`]). `cover cross`
//! entries combine the *same evaluation point's* already-mapped bin
//! indices of their named points into a joint index, incrementing that
//! cross's own counter — never re-evaluating anything.
//!
//! `cover=off` means the bin-mapper kernels are **never called**:
//! evaluation short-circuits before any kernel function runs, exactly the
//! `checks=off` shape [`super::constraints::MarginTracker`] already
//! established.

use crate::resolve::ResolvedCoverBins;
use piperine_solver::abi::CoverPosture;

use super::LoadCtx;

/// Map a raw evaluated value to a bin index, or `None` when it falls
/// outside every declared bin (a value out of range simply does not count
/// — never an error, never a clamp).
fn bin_of(value: f64, bins: &ResolvedCoverBins) -> Option<usize> {
    match bins {
        ResolvedCoverBins::Range { lo, step, hi } => {
            if value < *lo - 1e-9 || value > *hi + 1e-9 {
                return None;
            }
            let count = ((hi - lo) / step + 1e-9).floor() as usize + 1;
            let idx = ((value - lo) / step).round() as i64;
            if idx < 0 {
                return None;
            }
            let idx = idx as usize;
            (idx < count).then_some(idx)
        }
        ResolvedCoverBins::Set(values) => values.iter().position(|v| (v - value).abs() < 1e-9),
    }
}

/// Per-instance coverage runtime: one bin-count vector per declared
/// `cover` point, and one joint-bin-count vector per declared
/// `cover cross`. An empty tracker (no declared `cover`) evaluates to
/// nothing, the same shape a module with no `cover` pays.
pub(super) struct CoverTracker {
    /// Per cover point (kernel catalog order): one counter per bin.
    counts: Vec<Vec<u64>>,
    /// Per cover cross (kernel catalog order): one counter per joint bin
    /// (row-major over its points' own bin indices, in declared order).
    cross_counts: Vec<Vec<u64>>,
    /// How many times `eval_cover` has actually been called (T33's own
    /// "asserted by count, not by output" proof surface: `cover=off`'s
    /// early return keeps this at `0` regardless of whether any evaluated
    /// value would have landed in a bin — a stronger claim than "every bin
    /// stayed empty", which a value falling outside every bin would also
    /// produce even if the kernel *had* run).
    eval_calls: u64,
}

impl CoverTracker {
    /// A fresh tracker: `bin_counts[i]` is point `i`'s declared bin count;
    /// `cross_joint_counts[j]` is cross `j`'s declared joint bin count.
    pub(super) fn new(bin_counts: &[usize], cross_joint_counts: &[usize]) -> Self {
        Self {
            counts: bin_counts.iter().map(|&n| vec![0u64; n]).collect(),
            cross_counts: cross_joint_counts.iter().map(|&n| vec![0u64; n]).collect(),
            eval_calls: 0,
        }
    }

    /// How many times `eval_cover` has been called across this tracker's
    /// lifetime.
    pub(super) fn eval_calls(&self) -> u64 {
        self.eval_calls
    }

    /// Evaluate every declared `cover` point at one accepted point
    /// (`volts`), map each to a bin, increment its counter, then combine
    /// this same instant's per-point hits into every `cover cross`'s
    /// joint counter. `cover=off` returns before any kernel function
    /// runs — "kernels are never called" is this early return.
    pub(super) fn evaluate(&mut self, cx: &LoadCtx<'_>, volts: &[f64], time: f64, posture: CoverPosture, analysis: piperine_solver::abi::AnalysisKind) {
        if posture == CoverPosture::Off {
            return;
        }
        let running = analysis.as_str();
        // This instant's per-point bin index (by label), for the cross
        // combination below — `None` when the point did not evaluate
        // (out of analysis/window scope) or fell outside every bin.
        let mut this_hit: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, info) in cx.kernel.cover_catalog().iter().enumerate() {
            if !info.analyses.is_empty() && !info.analyses.iter().any(|a| a == running) {
                continue;
            }
            if let Some(w) = info.window
                && !w.contains(time)
            {
                continue;
            }
            self.eval_calls += 1;
            let value = cx.kernel.eval_cover(i, volts, cx.params, cx.state, cx.vars, cx.sim);
            if !value.is_finite() {
                continue;
            }
            if let Some(bin) = bin_of(value, &info.bins) {
                self.counts[i][bin] += 1;
                this_hit.insert(info.label.as_str(), bin);
            }
        }
        for (j, cross) in cx.kernel.cover_cross_catalog().iter().enumerate() {
            // Every named point must have hit a bin at this same instant
            // for the cross to count — a point out of scope or out of
            // range disqualifies the cross for this point, not a panic.
            let mut joint = 0usize;
            let mut all_hit = true;
            for point_label in &cross.points {
                let Some(&bin) = this_hit.get(point_label.as_str()) else {
                    all_hit = false;
                    break;
                };
                let point_bin_count = cx
                    .kernel
                    .cover_catalog()
                    .iter()
                    .find(|p| &p.label == point_label)
                    .map(|p| match &p.bins {
                        ResolvedCoverBins::Range { lo, step, hi } => ((hi - lo) / step + 1e-9).floor() as usize + 1,
                        ResolvedCoverBins::Set(v) => v.len(),
                    })
                    .unwrap_or(1);
                joint = joint * point_bin_count + bin;
            }
            if all_hit && joint < self.cross_counts[j].len() {
                self.cross_counts[j][joint] += 1;
            }
        }
    }

    /// The running per-bin hit counts, one `Vec<u64>` per declared `cover`
    /// point, in kernel catalog order.
    pub(super) fn counts(&self) -> &[Vec<u64>] {
        &self.counts
    }

    /// The running per-joint-bin hit counts, one `Vec<u64>` per declared
    /// `cover cross`, in kernel catalog order.
    pub(super) fn cross_counts(&self) -> &[Vec<u64>] {
        &self.cross_counts
    }
}
