//! The sequencer's findings buffer (groundwork for T21/T22, DVT-29): every
//! side-effecting call a [`super::SequencerProgram`] makes through
//! [`super::SequencerControl`] — an internal-net drive (T21), later
//! `expect`/`warn` (T22) — records here rather than printing. It is the
//! same [`ValidationFinding`] shape `dv-core`'s channel already carries
//! (`piperine-solver/src/core/validation.rs`), so a `Sequencer`'s findings
//! read exactly like any other element's.

use piperine_solver::abi::{ValidationFinding, ValidationReport, ValidationSeverity};

/// An append-only buffer of [`ValidationFinding`]s, drained by
/// `HostElement::validation_reports` (wired in T22) into the run's
/// `dv-core` channel.
#[derive(Debug, Default, Clone)]
pub struct Findings {
    entries: Vec<ValidationFinding>,
}

impl Findings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one finding.
    pub fn push(
        &mut self,
        severity: ValidationSeverity,
        label: impl Into<String>,
        message: impl Into<String>,
        value: f64,
        time: f64,
        instance_path: impl Into<String>,
    ) {
        self.entries.push(ValidationFinding {
            severity,
            label: label.into(),
            message: message.into(),
            value,
            time,
            instance_path: instance_path.into(),
        });
    }

    /// All findings recorded so far, in emission order.
    pub fn as_slice(&self) -> &[ValidationFinding] {
        &self.entries
    }

    /// True while nothing has been recorded — the common case.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Wrap everything recorded so far into a [`ValidationReport`], the
    /// shape `Element::validation_reports` returns.
    pub fn to_report(&self) -> Option<ValidationReport> {
        if self.entries.is_empty() {
            None
        } else {
            Some(ValidationReport::new(self.entries.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_findings_buffer_is_empty_and_reports_none() {
        let f = Findings::new();
        assert!(f.is_empty());
        assert!(f.to_report().is_none());
    }

    #[test]
    fn a_pushed_finding_is_readable_and_reportable() {
        let mut f = Findings::new();
        f.push(ValidationSeverity::Warning, "internal_drive", "driving internal net x", 0.0, 5e-6, "seq0");
        assert!(!f.is_empty());
        assert_eq!(f.as_slice().len(), 1);
        let report = f.to_report().expect("a non-empty buffer reports Some");
        assert_eq!(report.findings().len(), 1);
        assert_eq!(report.findings()[0].instance_path, "seq0");
    }
}
