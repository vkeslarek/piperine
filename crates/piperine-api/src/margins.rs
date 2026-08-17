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

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::pom::Design;
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
    /// here — "not exercised" is a separate, louder signal
    /// ([`Self::is_not_exercised`], DVC-19/20) reports; it is never a
    /// violation.
    pub fn is_satisfied(&self) -> bool {
        match self.value {
            MarginValue::Pointwise { worst, .. } => worst >= 0.0,
            MarginValue::Reduced { value } => value >= 0.0,
        }
    }

    /// `true` when the margin was never evaluated across this result —
    /// `worst = +inf` and no argmin. The case (DVC-19): a `require`
    /// scoped to `@ dc { … }` read from a transient-only result. Never a
    /// silent vacuous pass: [`MarginsResult::requires_ok`] returns `false`
    /// when any `require` is not exercised, and
    /// [`MarginsResult::first_not_exercised`] finds it.
    pub fn is_not_exercised(&self) -> bool {
        match self.value {
            MarginValue::Pointwise { worst, argmin } => worst.is_infinite() && argmin.is_none(),
            MarginValue::Reduced { .. } => false,
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

/// The declared-constraint catalog one analysis result reads against
/// (DVC-15/18): every constraint label the compiled design declares that is
/// **absent** from the result's margins channel, split by *why* it is
/// absent, so [`MarginsResult::get`] fails loud with the real reason
/// instead of a misleading "unknown constraint":
///
/// - `not_defined` — a reduced `var`/`target`, computed once per completed
///   AC sweep (D7): defined on AC results, "not defined in this analysis"
///   everywhere else.
/// - `digital` — a statement whose instance has no analog half: margins are
///   an analog notion (design §C4), so there is no margin to read.
///
/// Names are stored qualified (`"instance.path.label"`), the same form
/// [`MarginsResult::get`]'s candidate listing uses.
#[derive(Debug, Clone, Default)]
pub struct ConstraintCatalog {
    not_defined: Vec<String>,
    digital: Vec<String>,
}

impl ConstraintCatalog {
    /// Walk the flat top module's instances and classify every declared
    /// constraint label that produces no entry in a pointwise margins
    /// channel. An instance absent from `info.instances` has no analog
    /// kernel (digital-only — the builder registers analog instances only),
    /// so its requires are digital; an analog instance's reduced
    /// `var`/`target` is host-computed on AC results only (deferred with
    /// dv-core's other statements — nothing lands in `not_defined` until
    /// they exist, but the classification site stays). Pointwise `var`s
    /// travel inside the margins that read them and are never readable by
    /// name — they stay "unknown", catalogued nowhere.
    ///
    /// Core scope (`require` only, constraints-on-vams): every require on
    /// an analog instance reaches the kernel's margin catalog, so the
    /// digital list is the one that can fire — `MarginsResult::get` on a
    /// digital-instance require then fails loud naming it rather than
    /// claiming the constraint is unknown.
    pub fn compute(design: &Design, info: &CircuitBuildInfo, top: &str) -> Self {
        let mut catalog = Self::default();
        let Some(flat) = design.flat_module(top) else { return catalog };
        for instance in flat.instances() {
            let Some(module) = design.module(instance.module_name()) else { continue };
            if module.constraints().is_empty() {
                continue;
            }
            let analog = info.instances.iter().find(|i| i.label == instance.name());
            for req in module.constraints() {
                let qualified = format!("{}.{}", instance.name(), req.name);
                let Some(built) = analog else {
                    catalog.digital.push(qualified);
                    continue;
                };
                let in_kernel = built.kernel.margin_catalog().iter().any(|m| m.label == req.name);
                if !in_kernel {
                    // A require that lowered but missed the kernel would be
                    // a silent gap — record it as digital (no analog margin
                    // exists to read) rather than letting it fall to
                    // "unknown constraint" downstream.
                    catalog.digital.push(qualified);
                }
            }
        }
        catalog
    }

    /// `true` when `name` — qualified (`"x1.ugbw"`) or bare (`"ugbw"`) —
    /// matches a catalogued qualified name.
    fn declares(list: &[String], name: &str) -> bool {
        list.iter().any(|q| q == name || q.rsplit_once('.').is_some_and(|(_, label)| label == name))
    }
}

/// The margins channel of one analysis result (D3): every declared
/// margin on every instance, pointwise and reduced, keyed by authored
/// label. Margins never appear as waveform rows — this channel is the
/// only place they live.
#[derive(Debug, Clone, Default)]
pub struct MarginsResult {
    entries: Vec<MarginEntry>,
    /// Declared reduced `var`/`target` names not defined by this result's
    /// analysis (DVC-15/18) — the [`ConstraintCatalog`] half that turns a
    /// cross-analysis read into "not defined in this analysis".
    not_defined: Vec<String>,
    /// Declared names whose instance has no analog half — asking for one
    /// is loud: margins are an analog notion.
    digital: Vec<String>,
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
        Self { entries, ..Self::default() }
    }

    /// Build the channel from ready entries — the host-side reductions
    /// (D7) construct their reduced entries directly.
    pub fn from_entries(entries: Vec<MarginEntry>) -> Self {
        Self { entries, ..Self::default() }
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
        Self { entries, ..Self::default() }
    }

    /// Attach the declared-constraint catalog (DVC-15/18) — the session
    /// computes it once per analysis so [`Self::get`] fails loud with the
    /// real reason a declared name has no entry here.
    pub fn with_catalogs(mut self, catalog: &ConstraintCatalog) -> Self {
        self.not_defined = catalog.not_defined.clone();
        self.digital = catalog.digital.clone();
        self
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
    /// qualified forms. A declared name the design carries but this
    /// analysis does not define fails loud with the reason (DVC-15/18):
    /// a reduced quantity is "not defined in this analysis", and a
    /// constraint on a digital-only instance has no margin at all —
    /// margins are an analog notion.
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
            (Some(_), Some(_)) => Err(Error::Measurement(format!(
                "constraint `{name}` is ambiguous — qualify it: {}",
                candidates()
            ))),
            (None, _) => {
                if ConstraintCatalog::declares(&self.not_defined, name) {
                    return Err(Error::Measurement(format!(
                        "constraint `{name}` is not defined in this analysis — it is a reduced \
                         quantity, evaluated once per completed AC sweep"
                    )));
                }
                if ConstraintCatalog::declares(&self.digital, name) {
                    return Err(Error::Measurement(format!(
                        "constraint `{name}` has no margin — margins are an analog notion \
                         (its instance has no analog half)"
                    )));
                }
                Err(Error::Measurement(format!(
                    "unknown constraint `{name}` (candidates: {})",
                    candidates()
                )))
            }
        }
    }

    /// `true` when every `require` margin is satisfied **and** every
    /// `require` margin was exercised — the common assertion shape
    /// (`target`s are soft goals and never gate this). A scoped
    /// `require` whose analysis never ran (DVC-19) makes this `false`:
    /// never a silent vacuous pass. Use
    /// [`first_violation`](Self::first_violation) /
    /// [`first_not_exercised`](Self::first_not_exercised) to see why.
    pub fn requires_ok(&self) -> bool {
        self.entries
            .iter()
            .filter(|e| e.kind == MarginKind::Require)
            .all(|e| e.is_satisfied() && !e.is_not_exercised())
    }

    /// The first violated `require` margin in catalog order, if any —
    /// the common "show me what failed" read.
    pub fn first_violation(&self) -> Option<&MarginEntry> {
        self.entries
            .iter()
            .find(|e| e.kind == MarginKind::Require && !e.is_satisfied())
    }

    /// The first `require` margin that was never exercised across this
    /// result (DVC-19) — a scoped `require` whose analysis never ran.
    /// Such a margin is never a silent vacuous pass: `requires_ok`
    /// returns `false` when one exists.
    pub fn first_not_exercised(&self) -> Option<&MarginEntry> {
        self.entries
            .iter()
            .find(|e| e.kind == MarginKind::Require && e.is_not_exercised())
    }
}
