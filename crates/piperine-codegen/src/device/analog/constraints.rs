//! Constraint margin evaluation (device side, DVC-10): at each accepted
//! point the kernel's margin functions are evaluated and every declared
//! margin's worst (minimum) value is tracked with its argmin — the
//! evaluation coordinate (time in transient, frequency in AC) the worst
//! occurred at. Margins are values; findings are events (design §C4): a
//! `require` margin crossing zero and a non-finite margin at an evaluated
//! point are recorded here as `Error` findings (a non-finite margin is a
//! loud violation, never clamped to something finite). The instance path
//! on a finding is stamped by [`PiperineDevice`](crate::device::PiperineDevice)
//! at the element seam — the same pattern as `LimitingReport.device` —
//! because the authored hierarchical path is the element's label, not the
//! analog half's business.
//!
//! `checks=off` means the margin kernels are **never called**: evaluation
//! short-circuits before any kernel function runs, so the off posture is
//! exactly as expensive as a module with no constraint block.

use piperine_solver::abi::{CheckPosture, ValidationFinding, ValidationReport, ValidationSeverity};

use crate::resolve::MarginKind;

use super::LoadCtx;

/// One declared margin's running extremum: the worst (minimum) signed
/// value seen across the run's evaluation points (`m ≥ 0` ⟺ satisfied),
/// and the evaluation coordinate it occurred at. `worst = +inf` with
/// `argmin = None` marks a margin never evaluated — non-finite values
/// are reported as findings, never tracked here.
#[derive(Debug, Clone)]
pub struct MarginExtremum {
    pub worst: f64,
    pub argmin: Option<f64>,
}

/// Per-instance margin runtime: the running worst + argmin per declared
/// margin and the findings recorded at the latest evaluation point
/// (drained by the next [`evaluate`](Self::evaluate), mirroring the
/// `limit_report` caching contract — the solver polls after the accept
/// hook ran). An empty tracker (no declared margins) evaluates to
/// nothing, the same `None` a module without a constraint block pays.
pub(super) struct MarginTracker {
    extrema: Vec<MarginExtremum>,
    pending: Vec<ValidationFinding>,
}

impl MarginTracker {
    /// A fresh tracker for `num_margins` declared margins, all
    /// unevaluated (`+inf`, no argmin).
    pub(super) fn new(num_margins: usize) -> Self {
        Self {
            extrema: vec![MarginExtremum { worst: f64::INFINITY, argmin: None }; num_margins],
            pending: Vec::new(),
        }
    }

    /// Evaluate every declared margin at one accepted point (`volts`),
    /// through the instance's read-only stamping context. `time` is the
    /// evaluation coordinate (time in transient, frequency in AC).
    /// `checks=off` returns before any kernel function runs — "kernels
    /// are never called" is this early return, not a skipped report.
    /// `analysis` is the running analysis kind (DVC-19): a margin with a
    /// non-empty `analyses` set is evaluated only when the running
    /// analysis matches; one whose scope never matches stays at its
    /// never-evaluated sentinel (`+inf`, no argmin) — "not exercised",
    /// never a vacuous pass. A temporal window (DVC-20) further filters
    /// by `time`: a margin whose window excludes every evaluated point is
    /// also "not exercised".
    pub(super) fn evaluate(
        &mut self,
        cx: &LoadCtx<'_>,
        volts: &[f64],
        time: f64,
        posture: CheckPosture,
        analysis: piperine_solver::abi::AnalysisKind,
    ) {
        self.pending.clear();
        if posture == CheckPosture::Off {
            return;
        }
        let running = analysis.as_str();
        for (i, info) in cx.kernel.margin_catalog().iter().enumerate() {
            // DVC-19: an analysis-scoped margin (info.analyses non-empty)
            // is evaluated only when the running analysis matches its
            // scope. An empty analyses set holds in every analysis (D2).
            if !info.analyses.is_empty() && !info.analyses.iter().any(|a| a == running) {
                continue;
            }
            // DVC-20: a temporal-window-scoped margin is evaluated only
            // when the evaluation coordinate falls inside the window. A
            // window that selects no points leaves the margin at its
            // never-evaluated sentinel ("not exercised", never a vacuous
            // pass). DC has time = 0 (or `−1.0` as the analysis's
            // convention for "no time"); we evaluate it only when the
            // window contains 0.
            if let Some(w) = info.window
                && !w.contains(time) {
                continue;
            }
            let margin = cx.kernel.eval_margin(i, volts, cx.params, cx.state, cx.vars, cx.sim);
            if !margin.is_finite() {
                // Loud, never clamped: the finding carries the raw
                // non-finite value and the extremum is left untouched.
                self.pending.push(ValidationFinding {
                    severity: ValidationSeverity::Error,
                    label: info.label.clone(),
                    message: format!("margin is non-finite ({margin}) at an evaluated point"),
                    value: margin,
                    time,
                    instance_path: String::new(),
                });
                continue;
            }
            let extremum = &mut self.extrema[i];
            if margin < extremum.worst {
                extremum.worst = margin;
                extremum.argmin = Some(time);
            }
            // A `require` crossing zero is an `Error` finding (DVC-15);
            // a `target` is a soft goal — tracked above, never a finding.
            if margin < 0.0 && info.kind == MarginKind::Require {
                self.pending.push(ValidationFinding {
                    severity: ValidationSeverity::Error,
                    label: info.label.clone(),
                    message: format!("require violated: margin {margin} < 0"),
                    value: margin,
                    time,
                    instance_path: String::new(),
                });
            }
        }
    }

    /// The running worst + argmin per declared margin, in kernel catalog
    /// order — the device's half of the margin channel the host reduces
    /// into `MarginsResult` (design §D).
    pub(super) fn extrema(&self) -> &[MarginExtremum] {
        &self.extrema
    }

    /// The findings recorded at the latest evaluation point, or `None`
    /// when that point was clean (`Option`, not an empty report — the
    /// `limiting_report()` shape). The instance path is stamped by the
    /// element seam.
    pub(super) fn report(&self) -> Option<ValidationReport> {
        (!self.pending.is_empty()).then(|| ValidationReport::new(self.pending.clone()))
    }

    /// Reset every extremum to its never-evaluated state and drain the
    /// pending findings — margins are a per-run channel (DVC-18), so one
    /// analysis's extrema must never leak into the next run on the same
    /// circuit.
    pub(super) fn reset(&mut self) {
        for extremum in &mut self.extrema {
            extremum.worst = f64::INFINITY;
            extremum.argmin = None;
        }
        self.pending.clear();
    }
}
