//! Aging as parameter drift (dv-gradients T21, DVG-21): the host computes
//! a scalar STRESS proxy from an already-solved analysis (an OP or a
//! transient run), drifts declared `(label, param)` pairs proportionally
//! to `stress · time` through the ORDINARY restamp path
//! ([`crate::Session::set`] — the same mechanism T13's sampling uses, no
//! new mutation route), re-verifies at each scanned lifetime point, and
//! reports the MARGIN DELTA: which declared constraint goes negative
//! first, and after how long.
//!
//! # This is a placeholder drift model, not MOSRA/RelXpert (documented limitation)
//!
//! The gap versus a foundry reliability tool (MOSRA, RelXpert, …) is the
//! model **data** — the per-device stress equations (NBTI/PBTI/HCI/EM,
//! calibrated to silicon) — not the mechanism. Piperine's shipped
//! ngspice-faithful device models carry no such equations (the same
//! documented gap `dv-core` states for SOA limits), so any "stress"
//! computed here is necessarily a caller-supplied LINEAR PROXY
//! (`drift = rate_per_stress · stress · t`), never a physically
//! calibrated aging model. A device author who wants real aging supplies
//! a calibrated `rate_per_stress`; this module supplies the
//! restamp/re-verify/report LOOP, not the physics — a stated limitation,
//! not a bug to solve (DVG-21 AC4).
//!
//! # Rebuild-class drift fails loud (DVG-21 edge case)
//!
//! A drift that would carry a parameter across a rebuild-class
//! (`Invalidation::Rebuild`) boundary fails loud rather than silently
//! re-elaborating. This reuses [`crate::Session::set`]'s EXISTING
//! admissibility check (`crates/piperine-api/src/session/entry.rs`) — the
//! same classification T3/DVG-03 established for the sensitivity ABI —
//! rather than inventing a second one: [`age`] simply calls `Session::set`
//! for every drift step and lets its `Invalidation::Rebuild` refusal
//! propagate.
//!
//! # No new result type (DVG-21 AC2)
//!
//! [`AgingResult`] is a thin composition of the EXISTING
//! [`crate::MarginsResult`] channel — `baseline` and `aged` are read with
//! [`crate::MarginsResult::get`] exactly like any other margins channel;
//! [`AgingResult::delta`] is `aged.get(name)?.scalar() -
//! baseline.get(name)?.scalar()`, not a dedicated delta type.

use crate::error::Error;
use crate::margins::MarginsResult;
use crate::session::{Session, SolverConfig};

/// Where [`AgingSpec::stress`] reads its scalar stress proxy from
/// (DVG-21: "an OP or transient run").
#[derive(Debug, Clone)]
pub enum StressSource {
    /// A metric read from a DC operating point — a declared margin/var
    /// name or a solved net name, resolved the same way
    /// [`crate::Session::gradient`] resolves a metric.
    Op {
        /// The metric name.
        metric: String,
    },
    /// The peak absolute value of a net over a transient run — the usual
    /// stress proxy for a switching/dynamic stress (e.g. peak `Vgs`
    /// during a transition).
    TranPeak {
        /// The net to read.
        net: String,
        /// The transient's stop time.
        stop: f64,
        /// The transient's fixed step, or `None` for the solver's
        /// adaptive default.
        step: Option<f64>,
    },
}

/// One declared parameter's drift rate (T21): a PLACEHOLDER linear model,
/// `value(t) = from + rate_per_stress · stress · t` — see the module
/// docs' "not MOSRA/RelXpert" note. `from`/`rate_per_stress` are
/// caller-supplied; this module has no built-in per-device physics.
///
/// `from` is the CALLER-NAMED baseline to drift from — [`age`] does not
/// look up the design's authored default (unlike [`crate::SamplePlan`],
/// which owns that walk for `tol` distributions): an aging campaign
/// already knows the process baseline it is drifting from (a foundry PDK
/// nominal, a prior corner point, …), and requiring it explicitly here
/// also means a drift target does not need a POM-resolvable default at
/// all — including a param whose declared default is `none` (a
/// presence-guarded optional), which is exactly the shape that exercises
/// the rebuild-class refusal below.
#[derive(Debug, Clone)]
pub struct DriftRate {
    /// The instance label to drift.
    pub label: String,
    /// The param name to drift.
    pub param: String,
    /// The baseline value to drift from (`t = 0`) and restore to when
    /// [`age`] returns.
    pub from: f64,
    /// Drift per unit stress per unit time.
    pub rate_per_stress: f64,
}

/// The aging request (T21, DVG-21).
#[derive(Debug, Clone)]
pub struct AgingSpec {
    /// Where the stress proxy is read from.
    pub stress: StressSource,
    /// Declared drift rates — which `(label, param)` pairs drift, and by
    /// how much per unit stress per unit time.
    pub drifts: Vec<DriftRate>,
    /// The declared constraints to watch for the FIRST one to go
    /// negative (qualified or bare, as [`crate::MarginsResult::get`]
    /// resolves them).
    pub constraints: Vec<String>,
    /// The lifetime horizon to scan.
    pub lifetime: f64,
    /// How many equal steps to scan the horizon in — the resolution of
    /// the reported "after how long".
    pub steps: usize,
}

/// The aging result (T21, DVG-21) — see the module docs' "no new result
/// type" note.
#[derive(Debug, Clone)]
pub struct AgingResult {
    /// The stress proxy value the drift was computed from.
    pub stress: f64,
    /// The margins channel at `t = 0` (before any drift).
    pub baseline: MarginsResult,
    /// The margins channel at the examined lifetime point — the time of
    /// the first violation if one was found within `lifetime`, else the
    /// channel at the full `lifetime` horizon.
    pub aged: MarginsResult,
    /// The first watched constraint ([`AgingSpec::constraints`]) to go
    /// negative, and the lifetime it happened at — `None` when no watched
    /// constraint went negative within `lifetime`.
    pub first_violation: Option<(String, f64)>,
    /// The lifetime horizon that was scanned.
    pub lifetime: f64,
}

impl AgingResult {
    /// `name`'s margin delta: `aged − baseline`. Negative means the
    /// margin shrank under drift. `None` when `name` is not a declared
    /// margin in both channels.
    pub fn delta(&self, name: &str) -> Option<f64> {
        let base = self.baseline.get(name).ok()?.scalar();
        let aged = self.aged.get(name).ok()?.scalar();
        Some(aged - base)
    }
}

/// Run the aging drift (T21): compute the stress proxy, restamp
/// `spec.drifts` at increasing lifetime fractions through the ordinary
/// restamp path, re-verify at each point, and stop at the first watched
/// constraint that goes negative (or scan the full `spec.lifetime` if
/// none does). `session` is restored to its pre-drift (nominal) values
/// before this returns — same "no residue" discipline
/// [`crate::SamplePlan::reset`] established for sampling (T14).
///
/// A drift that would carry a parameter across a rebuild-class boundary
/// fails loud (the module docs' "rebuild-class drift" note) — this
/// propagates straight from [`crate::Session::set`], not a second check.
///
/// Forces `collect` posture on every internal `op`/`tran` call
/// ([`crate::driver::Driver::run`]'s own T16 invariant, reused): the
/// whole point of aging is to WATCH a `require` margin cross zero, so a
/// `Strict` config (the common default) must not abort the run the
/// instant that happens — every other field of `config` reaches the
/// solver unchanged.
pub fn age(session: &mut Session, spec: &AgingSpec, config: &SolverConfig) -> Result<AgingResult, Error> {
    let mut collect_config = config.clone();
    collect_config.checks = piperine_solver::prelude::CheckPosture::Collect;
    let config = &collect_config;

    let baseline_op = session.op(config, None)?;
    let baseline = baseline_op.margins().clone();
    drop(baseline_op);

    let stress = match &spec.stress {
        StressSource::Op { metric } => {
            let op = session.op(config, None)?;
            crate::driver::read_metric(&op, metric)?
        }
        StressSource::TranPeak { net, stop, step } => {
            let trace = session.tran(*stop, *step, 0.0, config, None, false, &[])?;
            let wf = trace.v(net.as_str())?;
            wf.max().abs().max(wf.min().abs())
        }
    };

    let steps = spec.steps.max(1);
    let dt = spec.lifetime / steps as f64;
    let mut aged = baseline.clone();
    let mut first_violation = None;

    for step in 1..=steps {
        let t = dt * step as f64;
        for drift in &spec.drifts {
            let drifted = drift.from + drift.rate_per_stress * stress * t;
            // A drift crossing a rebuild-class boundary fails loud right
            // here — `Session::set`'s existing `Invalidation::Rebuild`
            // refusal, reused rather than reclassified (module docs).
            session.set(&drift.label, &drift.param, drifted)?;
        }
        let op = session.op(config, None)?;
        aged = op.margins().clone();
        let violated = spec.constraints.iter().find_map(|name| {
            let entry = op.margins().get(name).ok()?;
            (!entry.is_satisfied()).then(|| name.clone())
        });
        if let Some(name) = violated {
            first_violation = Some((name, t));
            break;
        }
    }

    // Restore every drifted param to its caller-named baseline — no
    // residue, reusing T14's discipline.
    for drift in &spec.drifts {
        session.set(&drift.label, &drift.param, drift.from)?;
    }

    Ok(AgingResult { stress, baseline, aged, first_violation, lifetime: spec.lifetime })
}
