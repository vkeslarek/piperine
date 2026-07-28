//! The margin channel's solver half (DVC-10/18): the per-run report of
//! every declared constraint margin's worst (minimum) signed value
//! (`m ≥ 0` ⟺ satisfied) with its argmin — the evaluation coordinate the
//! worst occurred at (the converged point's `0.0` in DC, an accepted
//! step's time in transient, a frequency in AC, a swept coordinate in a
//! sweep). Margins are values; findings are events (design §C4) — this is
//! the continuous channel the findings channel
//! ([`crate::core::validation`]) is generated from, kept separate so the
//! host can read a margin without scraping findings.
//!
//! Only **pointwise** margins travel this channel: reduced-class
//! quantities (`ac_unity_gain_freq`, …) never reach a kernel — they are
//! host-side reductions over the analysis's points (D7), so the host
//! result channel states the kind rather than this report inventing an
//! argmin for them (design §C2).

/// What kind of statement a reported margin came from — a `require` names
/// a hard invariant (its crossing zero is an `Error` finding), a `target`
/// is a soft goal (tracked, never a finding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginKind {
    /// A `require <label> : <pred>;` margin.
    Require,
    /// A `target <expr> <cmp> <level> [tol <scale>];` margin.
    Target,
}

/// One declared pointwise margin's run report: the worst (minimum) signed
/// value seen across the analysis's evaluation points and the coordinate
/// it occurred at. `worst = +inf` with `argmin = None` marks a margin
/// never evaluated in this run (non-finite margins are findings, never
/// extrema) — the host reads that pair as "not exercised", never as a
/// pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportedMargin {
    /// The constraint's authored label (the `require`'s name, a
    /// `target#<index>` position).
    pub label: String,
    /// The statement kind (require vs target).
    pub kind: MarginKind,
    /// The worst signed margin across the run's evaluation points.
    pub worst: f64,
    /// The evaluation coordinate the worst occurred at — `None` when the
    /// margin was never evaluated this run.
    pub argmin: Option<f64>,
    /// The authored hierarchical instance path of the reporting element —
    /// stamped at the element seam, the same pattern as
    /// [`ValidationFinding.instance_path`](crate::core::validation::ValidationFinding).
    pub instance_path: String,
}

/// The margins one element reports at an end-of-run poll. A container,
/// not a bare `Vec`, mirroring
/// [`ValidationReport`](crate::core::validation::ValidationReport) — the
/// channel can grow summary state without an ABI break.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarginReport {
    margins: Vec<ReportedMargin>,
}

impl MarginReport {
    /// A report wrapping `margins` in catalog order.
    pub fn new(margins: Vec<ReportedMargin>) -> Self {
        Self { margins }
    }

    /// The reported margins, in the kernel catalog's authored order.
    pub fn margins(&self) -> &[ReportedMargin] {
        &self.margins
    }

    /// The reported margins mutably — for the element seam that stamps
    /// the authored instance path onto margins its analog half produced
    /// (the `ValidationReport::findings_mut` pattern).
    pub fn margins_mut(&mut self) -> &mut [ReportedMargin] {
        &mut self.margins
    }
}
