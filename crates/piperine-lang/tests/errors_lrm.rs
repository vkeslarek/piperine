//! VAMS T19 (C8, VAMS-05) — E4xxx LRM-conformance error family.
//!
//! Asserts the rendering contract of every variant in the new E4xxx range
//! (LRM-clause-citing semantic errors, E-STD §C8). Each variant must:
//!
//! 1. carry a numbered `#[diagnostic(code(Exxxx))]` (E-STD 2);
//! 2. cite the LRM clause it implements in its `#[error]` message;
//! 3. carry a descriptive `#[help]` that names the actionable next step
//!    and repeats the LRM clause (E-STD 4/5);
//! 4. label its span field with descriptive text — never bare `"here"`
//!    (E-STD 3);
//! 5. carry at least one span (E-STD 6).
//!
//! E4001 (BranchKindConflict) is exercised by `tests/pom_branch.rs`;
//! this target covers E4002-E4005.

use miette::Diagnostic;
use piperine_lang::pom::{ElabError, ElabErrorKind};

fn span(off: usize, len: usize) -> Option<miette::SourceSpan> {
    Some(miette::SourceSpan::new(off.into(), len))
}

fn render(err: &ElabError) -> (String, Option<String>, Vec<String>) {
    let message = err.kind.to_string();
    let code = Diagnostic::code(&err.kind).map(|c| c.to_string());
    let labels: Vec<String> = Diagnostic::labels(&err.kind)
        .expect("labels iterator should resolve")
        .map(|l| l.label().unwrap_or("").to_string())
        .collect();
    (message, code, labels)
}

#[track_caller]
fn assert_clause_cited(message: &str, clause: &str, variant: &str) {
    assert!(
        message.contains(clause),
        "{variant}: message must cite LRM clause `{clause}`, got: {message}",
    );
}

#[track_caller]
fn assert_help_present(err: &ElabError, variant: &str) {
    let help = Diagnostic::help(&err.kind).map(|h| h.to_string());
    let Some(h) = help else {
        panic!("{variant}: every E4xxx variant must carry a #[help]");
    };
    assert!(!h.is_empty(), "{variant}: #[help] must not be empty");
    // E-STD 5: the help must cite an LRM clause so the user can read the
    // rule they hit. Accepted forms: "LRM §...", "Annex X", "ch. N",
    // bare "§..." — the clause reference is what matters, not its prefix.
    let cites_clause = ["LRM", "Annex ", "ch. ", "§"]
        .iter()
        .any(|marker| h.contains(marker));
    assert!(
        cites_clause,
        "{variant}: #[help] must cite an LRM clause (LRM §..., Annex X, ch. N), got: {h}",
    );
}

#[track_caller]
fn assert_no_bare_here(labels: &[String], variant: &str) {
    // E-STD 3: descriptive #[label] text, never the bare "here".
    for label in labels {
        assert_ne!(
            label, "here",
            "{variant}: bare `here` label violates E-STD 3 — describe what's at the span",
        );
    }
    assert!(
        !labels.is_empty(),
        "{variant}: every E4xxx variant must carry at least one #[label]",
    );
}

#[test]
fn e4002_annex_c_violation_renders_with_clause_label_and_help() {
    let err = ElabError::new(ElabErrorKind::AnnexCSubsetViolation {
        construct_kind: "digital event control @(...)".into(),
        context: "analog block".into(),
        span: span(10, 5),
    });
    let (message, code, labels) = render(&err);
    assert_eq!(code.as_deref(), Some("E4002"));
    assert_clause_cited(&message, "Annex C", "E4002");
    assert_help_present(&err, "E4002");
    assert_no_bare_here(&labels, "E4002");
    // E-STD 3 specifics: the construct kind and its location are named.
    assert!(message.contains("digital event control"), "message must name the construct: {message}");
    assert!(message.contains("analog block"), "message must name the context: {message}");
}

#[test]
fn e4003_annex_f_resolution_failure_renders_with_clause_label_and_help() {
    let err = ElabError::new(ElabErrorKind::DisciplineResolutionFailed {
        net: "n1".into(),
        module: "top".into(),
        method: "compatible-discipline resolution".into(),
        reason: "neighbouring nets carry incompatible disciplines".into(),
        span: span(20, 3),
    });
    let (message, code, labels) = render(&err);
    assert_eq!(code.as_deref(), Some("E4003"));
    assert_clause_cited(&message, "Annex F", "E4003");
    assert_help_present(&err, "E4003");
    assert_no_bare_here(&labels, "E4003");
    // E-STD 6 specifics: the net, module, applied method and failure
    // reason are all carried on the variant.
    assert!(message.contains("n1") && message.contains("top"), "must name net and module");
    assert!(
        message.contains("compatible-discipline resolution"),
        "must name the applied Annex F method",
    );
    assert!(
        message.contains("neighbouring nets carry incompatible disciplines"),
        "must state why resolution failed",
    );
}

#[test]
fn e4004_invalid_nature_binding_renders_with_clause_label_and_help() {
    let err = ElabError::new(ElabErrorKind::InvalidNatureBinding {
        discipline: "electrical".into(),
        nature: "Voltage".into(),
        issue: "nature `Voltage` is undeclared".into(),
        span: span(30, 10),
    });
    let (message, code, labels) = render(&err);
    assert_eq!(code.as_deref(), Some("E4004"));
    assert_clause_cited(&message, "§3.6.2.1", "E4004");
    assert_help_present(&err, "E4004");
    assert_no_bare_here(&labels, "E4004");
    assert!(message.contains("electrical"), "must name the discipline");
    assert!(
        message.contains("nature `Voltage` is undeclared"),
        "must state the binding issue",
    );
}

#[test]
fn e4004_invalid_nature_binding_reports_conservative_clash_as_distinct_issue() {
    // §3.6.2.1 conservative-discipline rule: potential and flow must
    // bind distinct natures. The variant's `issue` field carries the
    // specific binding violation, so callers distinguish undeclared-nature
    // from same-nature clashes.
    let err = ElabError::new(ElabErrorKind::InvalidNatureBinding {
        discipline: "broken".into(),
        nature: "Foo".into(),
        issue: "potential and flow bind the same nature".into(),
        span: None,
    });
    let message = err.kind.to_string();
    assert!(
        message.contains("potential and flow bind the same nature"),
        "must surface the specific binding violation: {message}",
    );
}

#[test]
fn e4005_connect_module_ambiguous_renders_with_both_endpoints_labelled() {
    let err = ElabError::new(ElabErrorKind::ConnectModuleAmbiguous {
        from_discipline: "electrical".into(),
        to_discipline: "logic".into(),
        issue: "is ambiguous — 2 candidates matched".into(),
        from_span: span(40, 5),
        to_span: span(60, 5),
    });
    let (message, code, labels) = render(&err);
    assert_eq!(code.as_deref(), Some("E4005"));
    assert_clause_cited(&message, "ch. 7", "E4005");
    assert_help_present(&err, "E4005");
    // E-STD 6 specifics: the variant carries two labelled spans for the
    // two endpoints of the crossing. miette exposes them as two labels.
    assert!(
        labels.len() >= 2,
        "E4005 must carry at least two #[label]s (one per endpoint), got {labels:?}",
    );
    assert_no_bare_here(&labels, "E4005");
    assert!(message.contains("electrical") && message.contains("logic"));
    assert!(message.contains("ambiguous"));
}

#[test]
fn e4005_connect_module_no_rule_renders_with_same_variant() {
    // The "no matching rule" case uses the same variant with a distinct
    // `issue` value, so callers don't need a separate error type.
    let err = ElabError::new(ElabErrorKind::ConnectModuleAmbiguous {
        from_discipline: "electrical".into(),
        to_discipline: "rotational".into(),
        issue: "matched no rule".into(),
        from_span: span(1, 1),
        to_span: span(2, 2),
    });
    let message = err.kind.to_string();
    assert!(message.contains("matched no rule"));
    assert_eq!(
        Diagnostic::code(&err.kind).map(|c| c.to_string()),
        Some("E4005".into()),
    );
}

#[test]
fn every_e4xxx_variant_carries_a_span_field() {
    // E-STD 6: "Span always carried. Every variant has a span."
    // Construct each variant with None spans and confirm the build
    // succeeds (i.e. the span fields exist) — the structural guarantee.
    let _e1 = ElabErrorKind::BranchKindConflict {
        module: "m".into(),
        branch: "b".into(),
        first: None,
        second: None,
    };
    let _e2 = ElabErrorKind::AnnexCSubsetViolation {
        construct_kind: "x".into(),
        context: "y".into(),
        span: None,
    };
    let _e3 = ElabErrorKind::DisciplineResolutionFailed {
        net: "n".into(),
        module: "m".into(),
        method: "m".into(),
        reason: "r".into(),
        span: None,
    };
    let _e4 = ElabErrorKind::InvalidNatureBinding {
        discipline: "d".into(),
        nature: "n".into(),
        issue: "i".into(),
        span: None,
    };
    let _e5 = ElabErrorKind::ConnectModuleAmbiguous {
        from_discipline: "a".into(),
        to_discipline: "b".into(),
        issue: "i".into(),
        from_span: None,
        to_span: None,
    };
    // If any variant lacked a span field, this would not compile.
}
