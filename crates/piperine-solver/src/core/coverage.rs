//! The coverage channel's solver half (dv-tester T33/T34, DVT-40/43): the
//! per-run report of every declared `cover` point's per-bin hit counts and
//! every `cover cross`'s per-joint-bin hit counts — the same end-of-run
//! poll shape [`crate::core::margin::MarginReport`] uses (this run's raw
//! counters; **merging across runs is the host's job**, T34, since
//! coverage persists across runs and a margin does not — this report is
//! deliberately per-run, stateless data, never accumulated here).

/// One declared `cover` point's run report: its authored label, its bin
/// count, and this run's per-bin hit counts (parallel arrays: bin `i`'s
/// count is `counts[i]`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReportedCoverPoint {
    pub label: String,
    pub counts: Vec<u64>,
    /// The authored hierarchical instance path of the reporting element —
    /// same pattern as [`crate::core::margin::ReportedMargin::instance_path`].
    pub instance_path: String,
}

/// One declared `cover cross`'s run report: its authored label, the
/// points it joins, and this run's per-joint-bin hit counts.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportedCoverCross {
    pub label: String,
    pub points: Vec<String>,
    pub counts: Vec<u64>,
    pub instance_path: String,
}

/// The coverage one element reports at an end-of-run poll — a container,
/// not a bare `Vec`, mirroring [`crate::core::margin::MarginReport`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoverageReport {
    points: Vec<ReportedCoverPoint>,
    crosses: Vec<ReportedCoverCross>,
}

impl CoverageReport {
    pub fn new(points: Vec<ReportedCoverPoint>, crosses: Vec<ReportedCoverCross>) -> Self {
        Self { points, crosses }
    }

    pub fn points(&self) -> &[ReportedCoverPoint] {
        &self.points
    }

    pub fn crosses(&self) -> &[ReportedCoverCross] {
        &self.crosses
    }

    /// Mutable access — for the element seam that stamps the authored
    /// instance path onto entries its analog half produced.
    pub fn points_mut(&mut self) -> &mut [ReportedCoverPoint] {
        &mut self.points
    }

    pub fn crosses_mut(&mut self) -> &mut [ReportedCoverCross] {
        &mut self.crosses
    }
}
