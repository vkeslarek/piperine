//! The validation findings channel (DVC-13): the structured,
//! severity-tagged reports every element origin — PHDL-compiled, plugin, or
//! OSDI — speaks through one ABI hook. A constraint violation, a model
//! self-check, a monitor hit: all are [`ValidationFinding`]s. Shaped after
//! [`LimitingReport`](crate::core::element::LimitingReport) — the report is
//! an `Option` because "nothing to say" is the common case.
//!
//! Margins are values; findings are events (design §C4). A margin crossing
//! zero *generates* a finding here; the continuous margin itself travels its
//! own channel. Digital verification rides findings only — there is no
//! signed distance for a `Bit` or a `Quad`.

/// How loud a finding is. Only [`ValidationSeverity::Error`] aborts an
/// analysis under the strict posture; a `Warning` is always recorded and
/// never aborts (DVC-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Recorded, never fatal.
    Warning,
    /// Recorded; aborts the analysis under the strict posture.
    Error,
}

/// One structured validation finding (DVC-13): what fired (`label`), a
/// human-readable `message`, the offending `value`, the `time` (or
/// evaluation coordinate) it fired at, and the authored `instance_path` of
/// the element that reported it — the hierarchical path, never a flattened
/// label.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationFinding {
    pub severity: ValidationSeverity,
    /// The check's authored label (the `require`/`target` name, the model
    /// self-check name).
    pub label: String,
    /// Human-readable detail for transcripts and host diagnostics.
    pub message: String,
    /// The offending value (e.g. the margin that crossed zero).
    pub value: f64,
    /// The evaluation coordinate the finding fired at (time in transient,
    /// the converged point's `0.0` in DC, frequency in AC).
    pub time: f64,
    /// The authored hierarchical instance path of the reporting element.
    pub instance_path: String,
}

/// The findings an element reports at one poll (DVC-13). A container, not a
/// bare `Vec`, so the channel can grow summary state without an ABI break.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationReport {
    findings: Vec<ValidationFinding>,
}

impl ValidationReport {
    /// A report wrapping `findings` in emission order.
    pub fn new(findings: Vec<ValidationFinding>) -> Self {
        Self { findings }
    }

    /// The findings, in emission order.
    pub fn findings(&self) -> &[ValidationFinding] {
        &self.findings
    }

    /// The findings mutably, in emission order — for the element seam
    /// that stamps the authored instance path onto findings its analog
    /// half produced (the `LimitingReport.device` pattern).
    pub fn findings_mut(&mut self) -> &mut [ValidationFinding] {
        &mut self.findings
    }

    /// Whether any finding is an [`ValidationSeverity::Error`] — the
    /// strict-posture abort gate (DVC-15).
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == ValidationSeverity::Error)
    }
}
