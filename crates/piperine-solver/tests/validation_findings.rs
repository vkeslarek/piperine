//! The validation findings channel (DVC-13): `ValidationFinding` /
//! `ValidationReport` shape, the cross-cutting `Element::validation_reports`
//! hook (not `AnalogDevice`-gated), and the MD-17 two-tier export (both
//! `abi::` and `prelude::` reach the types). A recording test element emits
//! one `Warning` and one `Error` at known times; both are readable through
//! the hook with every field intact.

use piperine_solver::abi::{
    AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect, ValidationFinding,
    ValidationReport, ValidationSeverity,
};
use piperine_solver::prelude::{ValidationFinding as PreludeFinding, ValidationReport as PreludeReport};

/// A minimal element that reports one Warning and one Error at known times.
struct Recording;

impl AnalogDevice for Recording {}
impl DigitalDevice for Recording {}
impl Introspect for Recording {}
impl Element for Recording {
    fn name(&self) -> &str {
        "rec"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
    fn validation_reports(&self) -> Option<ValidationReport> {
        Some(ValidationReport::new(vec![
            ValidationFinding {
                severity: ValidationSeverity::Warning,
                label: "warn_check".to_string(),
                message: "approaching the limit".to_string(),
                value: 0.25,
                time: 1e-9,
                instance_path: "/top/rec".to_string(),
            },
            ValidationFinding {
                severity: ValidationSeverity::Error,
                label: "hard_check".to_string(),
                message: "limit exceeded".to_string(),
                value: -0.5,
                time: 2e-9,
                instance_path: "/top/rec".to_string(),
            },
        ]))
    }
}

/// A minimal element that never overrides the hook — the inherited default.
struct Silent;

impl AnalogDevice for Silent {}
impl DigitalDevice for Silent {}
impl Introspect for Silent {}
impl Element for Silent {
    fn name(&self) -> &str {
        "silent"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
    }
}

/// DVC-13 AC1: a recording element's Warning and Error are both readable
/// through `validation_reports()` with every field intact — severity, label,
/// message, value, time, instance path — in emission order.
#[test]
fn recording_element_emits_warning_and_error_through_the_hook() {
    let report = Recording.validation_reports().expect("a recording element has a report");
    let findings = report.findings();
    assert_eq!(findings.len(), 2, "one Warning and one Error");

    let warning = &findings[0];
    assert_eq!(warning.severity, ValidationSeverity::Warning);
    assert_eq!(warning.label, "warn_check");
    assert_eq!(warning.message, "approaching the limit");
    assert_eq!(warning.value, 0.25);
    assert_eq!(warning.time, 1e-9);
    assert_eq!(warning.instance_path, "/top/rec");

    let error = &findings[1];
    assert_eq!(error.severity, ValidationSeverity::Error);
    assert_eq!(error.label, "hard_check");
    assert_eq!(error.message, "limit exceeded");
    assert_eq!(error.value, -0.5);
    assert_eq!(error.time, 2e-9);
    assert_eq!(error.instance_path, "/top/rec");

    assert!(report.has_errors(), "an Error finding sets the abort gate");
}

/// DVC-13 AC1: the hook is cross-cutting — a digital-only element reports
/// through it (the `Recording` fixture above declares only `DIGITAL`), and
/// an element that never overrides it inherits `None` (nothing to say is
/// the common case; `Option`, not `Vec`, after `limiting_report()`).
#[test]
fn the_hook_is_cross_cutting_and_defaults_to_none() {
    // `Recording` declares DIGITAL only and still reports (see above) — the
    // hook lives on `Element`, not on `AnalogDevice`.
    assert!(Recording.validation_reports().is_some());
    assert!(Silent.validation_reports().is_none(), "the default report is None");

    let empty = ValidationReport::default();
    assert!(empty.findings().is_empty());
    assert!(!empty.has_errors(), "an empty report never aborts");
}

/// MD-17 two-tier surface: the findings types are reachable from both
/// `abi::` (the element-implementor surface) and `prelude::` (the host
/// surface), naming the same types.
#[test]
fn findings_types_export_on_both_tiers() {
    let from_abi: ValidationReport = ValidationReport::new(vec![ValidationFinding {
        severity: ValidationSeverity::Warning,
        label: "l".to_string(),
        message: "m".to_string(),
        value: 1.0,
        time: 0.0,
        instance_path: "/i".to_string(),
    }]);
    let from_prelude: PreludeReport = from_abi.clone();
    assert_eq!(from_abi, from_prelude, "both tiers name the same report type");
    let finding: PreludeFinding = from_prelude.findings()[0].clone();
    assert_eq!(finding.label, "l");
}
