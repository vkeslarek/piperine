//! The margins channel (DVC-18, D3): constraint margins as their own
//! result channel, separate from waveform rows. Two shapes travel here
//! (design §C2): a **pointwise** margin carries the worst (minimum)
//! signed value across the analysis's evaluation points with its argmin
//! — the time, frequency, or swept coordinate it occurred at — while a
//! **reduced** margin carries one value and **no** argmin (never an
//! invented `t = 0`). `m ≥ 0` ⟺ satisfied for both.
//!
//! The channel is built from the solver's per-run reports
//! ([`ReportedMargin`]) for pointwise margins and from host-side
//! reductions (D7) for reduced ones. `m < 0` on a `require` is what
//! [`MarginsResult::requires_ok`] / [`MarginsResult::first_violation`]
//! read.

use piperine_solver::prelude::{MarginKind, ReportedMargin};

use crate::error::Error;

/// Which shape a margin carries (design §C2 — the channel states the
/// kind rather than inventing an argmin for a reduced quantity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginShape {
    /// Evaluable at every evaluation point — worst + argmin.
    Pointwise,
    /// Exists once per completed analysis — one value, no argmin.
    Reduced,
}

/// One margin's value, stating its shape (D3): a pointwise margin has a
/// worst value and the argmin it occurred at; a reduced margin has one
/// value per analysis and no argmin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarginValue {
    /// A pointwise margin: the worst signed value across the analysis's
    /// evaluation points and the coordinate it occurred at (`None` when
    /// the margin was never evaluated — "not exercised", never a pass).
    Pointwise { worst: f64, argmin: Option<f64> },
    /// A reduced margin: one value per completed analysis, computed
    /// host-side (D7) — no argmin exists, and none is invented.
    Reduced { value: f64 },
}

/// One declared constraint margin on one instance: its authored label,
/// statement kind, and shape-tagged value.
#[derive(Debug, Clone, PartialEq)]
pub struct MarginEntry {
    /// The constraint's authored label (a `require`'s name, a
    /// `target#<index>` position, a `var`'s name).
    pub label: String,
    /// The authored hierarchical instance path of the element the margin
    /// belongs to.
    pub instance_path: String,
    /// The statement kind (require vs target).
    pub kind: MarginKind,
    /// The margin's shape-tagged value.
    pub value: MarginValue,
}

impl MarginEntry {
    /// `true` when the margin is satisfied (`m ≥ 0`). A never-evaluated
    /// pointwise margin (`worst = +inf`, no argmin) reads as satisfied
    /// here — "not exercised" is a separate, louder signal the scoping
    /// work (DVC-19/20) reports; it is never a violation.
    pub fn is_satisfied(&self) -> bool {
        match self.value {
            MarginValue::Pointwise { worst, .. } => worst >= 0.0,
            MarginValue::Reduced { value } => value >= 0.0,
        }
    }

    /// The margin's scalar value (worst for pointwise, the value for
    /// reduced) — what `r.margins["name"]` reads as a number.
    pub fn scalar(&self) -> f64 {
        match self.value {
            MarginValue::Pointwise { worst, .. } => worst,
            MarginValue::Reduced { value } => value,
        }
    }

    /// Which shape this margin carries (design §C2) — a reader never has
    /// to guess whether an argmin exists.
    pub fn shape(&self) -> MarginShape {
        match self.value {
            MarginValue::Pointwise { .. } => MarginShape::Pointwise,
            MarginValue::Reduced { .. } => MarginShape::Reduced,
        }
    }

    /// The evaluation coordinate the worst occurred at (time, frequency,
    /// or swept coordinate). `None` for a reduced margin — a reduced
    /// quantity exists once per analysis, so no argmin exists and none is
    /// invented (never a fake `t = 0`).
    pub fn argmin(&self) -> Option<f64> {
        match self.value {
            MarginValue::Pointwise { argmin, .. } => argmin,
            MarginValue::Reduced { .. } => None,
        }
    }
}

/// The margins channel of one analysis result (D3): every declared
/// margin on every instance, pointwise and reduced, keyed by authored
/// label. Margins never appear as waveform rows — this channel is the
/// only place they live.
#[derive(Debug, Clone, Default)]
pub struct MarginsResult {
    entries: Vec<MarginEntry>,
}

impl MarginsResult {
    /// Build the channel from the solver's per-run pointwise reports.
    pub fn from_reported(reported: Vec<ReportedMargin>) -> Self {
        let entries = reported
            .into_iter()
            .map(|m| MarginEntry {
                label: m.label,
                instance_path: m.instance_path,
                kind: m.kind,
                value: MarginValue::Pointwise { worst: m.worst, argmin: m.argmin },
            })
            .collect();
        Self { entries }
    }

    /// Build the channel from ready entries — the host-side reductions
    /// (D7) construct their reduced entries directly.
    pub fn from_entries(entries: Vec<MarginEntry>) -> Self {
        Self { entries }
    }

    /// The worst-across-sweep reduction (DVC-11): one entry per declared
    /// margin, carrying the worst (minimum) value across the swept points
    /// and the swept coordinate it occurred at as the argmin — the row
    /// the optimizer, corner runs, and Monte Carlo all consume (design
    /// §C3). `points` pairs each swept coordinate with that point's
    /// channel. A reduced margin swept across points becomes the
    /// pointwise-over-sweep shape — the swept coordinate is a real
    /// argmin, not an invented one.
    pub fn worst_across(points: &[(f64, MarginsResult)]) -> Self {
        let mut entries: Vec<MarginEntry> = Vec::new();
        for (coord, point) in points {
            for entry in &point.entries {
                let scalar = entry.scalar();
                let slot = entries
                    .iter_mut()
                    .find(|e| e.instance_path == entry.instance_path && e.label == entry.label);
                match slot {
                    Some(e) => {
                        if scalar < e.scalar() {
                            e.value = MarginValue::Pointwise { worst: scalar, argmin: Some(*coord) };
                        }
                    }
                    None => entries.push(MarginEntry {
                        label: entry.label.clone(),
                        instance_path: entry.instance_path.clone(),
                        kind: entry.kind,
                        value: MarginValue::Pointwise { worst: scalar, argmin: Some(*coord) },
                    }),
                }
            }
        }
        Self { entries }
    }

    /// Merge additional entries into the channel (host-side reductions
    /// append their reduced entries after the solver's pointwise ones).
    pub(crate) fn extend(&mut self, extra: impl IntoIterator<Item = MarginEntry>) {
        self.entries.extend(extra);
    }

    /// Every entry, pointwise and reduced, in catalog order.
    pub fn entries(&self) -> &[MarginEntry] {
        &self.entries
    }

    /// The number of margins in the channel.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when no margins were declared (or checks were off).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read one margin by name: the bare authored label (`"head"`) when
    /// it is unique across instances, or the qualified
    /// `"instance.path.label"` form. An unknown name fails loud listing
    /// the candidates; an ambiguous bare name fails loud listing the
    /// qualified forms.
    pub fn get(&self, name: &str) -> Result<&MarginEntry, Error> {
        let candidates = || {
            self.entries
                .iter()
                .map(|e| format!("{}.{}", e.instance_path, e.label))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| format!("{}.{}", e.instance_path, e.label) == name)
        {
            return Ok(entry);
        }
        let mut bare = self.entries.iter().filter(|e| e.label == name);
        match (bare.next(), bare.next()) {
            (Some(entry), None) => Ok(entry),
            (None, _) => Err(Error::Measurement(format!(
                "unknown constraint `{name}` (candidates: {})",
                candidates()
            ))),
            (Some(_), Some(_)) => Err(Error::Measurement(format!(
                "constraint `{name}` is ambiguous — qualify it: {}",
                candidates()
            ))),
        }
    }

    /// `true` when every `require` margin is satisfied — the common
    /// assertion shape (`target`s are soft goals and never gate this).
    pub fn requires_ok(&self) -> bool {
        self.entries
            .iter()
            .filter(|e| e.kind == MarginKind::Require)
            .all(MarginEntry::is_satisfied)
    }

    /// The first violated `require` margin in catalog order, if any —
    /// the common "show me what failed" read.
    pub fn first_violation(&self) -> Option<&MarginEntry> {
        self.entries
            .iter()
            .find(|e| e.kind == MarginKind::Require && !e.is_satisfied())
    }
}
