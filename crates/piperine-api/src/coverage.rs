//! `cover` merge and closure data (dv-tester T34, DVT-43): the host-side
//! accumulation structure a `CoverageReport` (one run's raw counters,
//! `piperine_solver::abi::CoverageReport`) merges into. Coverage
//! **persists across runs and seeds** — unlike a margin, which belongs to
//! one analysis and is discarded at the next (DVC-18) — so this database
//! is host state a caller keeps around across an entire regression,
//! calling [`CoverageDatabase::merge_run`] once per run.

use std::collections::HashMap;

use piperine_solver::abi::{ReportedCoverCross, ReportedCoverPoint};

/// One declared `cover` point's or `cover cross`'s accumulated per-bin hit
/// counts, merged (summed) across every run [`CoverageDatabase::merge_run`]
/// has seen.
#[derive(Debug, Clone, Default)]
pub struct CoverageEntry {
    pub counts: Vec<u64>,
}

impl CoverageEntry {
    /// The bins with zero hits across every merged run, by index.
    pub fn empty_bin_indices(&self) -> Vec<usize> {
        self.counts.iter().enumerate().filter(|&(_, &c)| c == 0).map(|(i, _)| i).collect()
    }

    /// Whether every declared bin has been exercised at least once.
    pub fn fully_covered(&self) -> bool {
        !self.counts.is_empty() && self.counts.iter().all(|&c| c > 0)
    }
}

/// The merged coverage database: every `cover` point's and `cover
/// cross`'s counters, summed (never replaced) across every run merged in.
/// Keyed by `"<instance_path>.<label>"` (a bare `<label>` when the
/// instance path is empty, matching `ValidationFinding`'s own top-level
/// convention) so two instances of the same monitor module accumulate
/// independently.
#[derive(Debug, Clone, Default)]
pub struct CoverageDatabase {
    points: HashMap<String, CoverageEntry>,
    crosses: HashMap<String, CoverageEntry>,
}

fn key(instance_path: &str, label: &str) -> String {
    if instance_path.is_empty() {
        label.to_string()
    } else {
        format!("{instance_path}.{label}")
    }
}

impl CoverageDatabase {
    /// A fresh, empty database — no runs merged yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge one run's reports in: existing counters **sum** with the new
    /// run's counts (never replace); a point/cross not seen before starts
    /// from an all-zero vector sized to this run's report, so a run with
    /// zero accepted points (every count `0`) still creates the entry —
    /// [`Self::empty_bins`] then names every one of its bins as
    /// not-exercised, never silently omitting it (never a vacuous "100%
    /// covered" from an empty database).
    pub fn merge_run(&mut self, points: &[ReportedCoverPoint], crosses: &[ReportedCoverCross]) {
        for p in points {
            let entry = self.points.entry(key(&p.instance_path, &p.label)).or_insert_with(|| CoverageEntry {
                counts: vec![0; p.counts.len()],
            });
            merge_counts(&mut entry.counts, &p.counts);
        }
        for c in crosses {
            let entry = self.crosses.entry(key(&c.instance_path, &c.label)).or_insert_with(|| CoverageEntry {
                counts: vec![0; c.counts.len()],
            });
            merge_counts(&mut entry.counts, &c.counts);
        }
    }

    /// Every `cover` point's accumulated entry, keyed by
    /// `"<instance_path>.<label>"`.
    pub fn points(&self) -> &HashMap<String, CoverageEntry> {
        &self.points
    }

    /// Every `cover cross`'s accumulated entry, keyed the same way.
    pub fn crosses(&self) -> &HashMap<String, CoverageEntry> {
        &self.crosses
    }

    /// Every bin, across every declared `cover` point (not `cover cross`),
    /// that has never been hit across any merged run — named
    /// `"<key>[<bin index>]"`, sorted for determinism. A database with no
    /// runs merged at all has no entries and therefore names nothing
    /// (there is nothing to report yet, not "0 empty bins" — a caller
    /// checking coverage before any run should read [`Self::points`]
    /// being empty as its own signal, distinct from "ran and found gaps").
    pub fn empty_bins(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .points
            .iter()
            .flat_map(|(name, entry)| entry.empty_bin_indices().into_iter().map(move |i| format!("{name}[{i}]")))
            .collect();
        out.sort();
        out
    }
}

fn merge_counts(existing: &mut [u64], new: &[u64]) {
    for (e, n) in existing.iter_mut().zip(new) {
        *e += n;
    }
}
