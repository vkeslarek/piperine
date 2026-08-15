//! Post-elaboration typechecking over the finished [`Design`].
//!
//! **MD-45 (2026-08-12):** the PHDL-era checks this module ran (GAPS §B.1/
//! §B.2 connection-width and discipline-mismatch checks, pattern-match
//! exhaustiveness over PHDL `Stmt`/`Pattern`) were built directly against
//! `piperine-lang/src/parse/`'s AST types and are deleted with it, not
//! retargeted — none has a 1:1 VAMS shape.
//!
//! **T27 (VAMS-25) — Annex C analog-subset enforcement.** LRM Annex C.7:
//! "No digital behavior or events are supported in Verilog-A" — a digital-
//! only construct used inside an `analog`/`analog initial` behavior body
//! fails loud citing the clause (E4002 `AnnexCSubsetViolation`, declared
//! at T19). Checked today: non-blocking assignment (`<=`, digital-only —
//! LRM §5.5 restricts analog blocks to blocking `=` and contribution
//! `<+`) and `casex`/`casez` (Annex C.7: "`casex` and `casez` are not
//! supported in Verilog-A"). This is deliberately an **ongoing** gate
//! (VAMS-25): later phases (full 1364 digital, ch. 7 mixed-signal) add
//! more Annex-C-restricted constructs to the same walk rather than
//! opening a second mechanism.
//!
//! **T27 (LRMC-C5-05) narrowing:** digital edge/state-change/named-event
//! sensitivity (`@(posedge clk)`, `@(negedge clk)`, `@(some_signal)`,
//! `@(named_event)`) inside `analog` is explicitly legal per §5.10.5 —
//! removed from this gate rather than merely un-enforced (the blanket
//! `posedge`/`negedge` rejection was over-broad from day one, since the
//! LRM's own restriction only ever covered *behavior*, not this specific
//! sensitivity form).

use crate::ast::Stmt;

use crate::pom::{Design, ElabError, ElabErrorKind};

/// Typecheck every module in `design`, accumulating every independent
/// failure instead of stopping at the first (LSP-18/T16 precedent).
/// Returns every Annex C violation found across every analog behavior in
/// the design.
pub fn typecheck_program_accumulating(design: &Design) -> Vec<ElabError> {
    let mut errors = Vec::new();
    for module in design.modules() {
        for behavior in module.behaviors() {
            if !behavior.is_analog() {
                continue;
            }
            for stmt in behavior.body() {
                check_analog_subset(stmt, &mut errors);
            }
        }
    }
    errors
}

/// Recursively walks a statement tree inside an analog behavior body,
/// recording an [`ElabErrorKind::AnnexCSubsetViolation`] for every
/// digital-only construct found. Never stops at the first violation —
/// every offending statement in the tree is reported (the pass's own
/// caller decides whether to treat this as fail-fast or accumulate).
fn check_analog_subset(stmt: &Stmt, errors: &mut Vec<ElabError>) {
    match stmt {
        Stmt::NonBlockingAssign { span, .. } => {
            errors.push(annex_c_error("non-blocking assignment `<=`", *span));
        }
        Stmt::Case { kind, arms, selector: _, span } => {
            use crate::ast::CaseKind;
            if matches!(kind, CaseKind::Casex | CaseKind::Casez) {
                let name = if matches!(kind, CaseKind::Casex) { "`casex`" } else { "`casez`" };
                errors.push(annex_c_error(name, *span));
            }
            for arm in arms {
                check_analog_subset(&arm.body, errors);
            }
        }
        // §5.10.5: "analog behavior can be made sensitive to digital
        // events, including posedge events, negedge events, state change
        // events, and named events" — explicitly legal (LRMC-C5-05), not
        // an Annex C violation. Still recurses into the body for whatever
        // it contains.
        Stmt::EventControl { body, .. } => {
            check_analog_subset(body, errors);
        }
        Stmt::Block { stmts, .. } => stmts.iter().for_each(|s| check_analog_subset(s, errors)),
        Stmt::If { then_branch, else_branch, .. } => {
            check_analog_subset(then_branch, errors);
            if let Some(e) = else_branch {
                check_analog_subset(e, errors);
            }
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. }
        | Stmt::DelayControl { body, .. } => check_analog_subset(body, errors),
        Stmt::Null { .. }
        | Stmt::Assign { .. }
        | Stmt::Contribution { .. }
        | Stmt::Disable { .. }
        | Stmt::TaskCall { .. }
        // §5.10.4: "Named events can be triggered ... from an analog
        // event statement" — explicitly legal inside analog, not an
        // Annex C violation.
        | Stmt::EventTrigger { .. } => {}
    }
}

fn annex_c_error(construct_kind: &str, span: crate::Span) -> ElabError {
    ElabError::from(ElabErrorKind::AnnexCSubsetViolation {
        construct_kind: construct_kind.to_string(),
        context: "analog block".to_string(),
        span: Some(miette::SourceSpan::new(span.start.into(), span.end.saturating_sub(span.start))),
    })
}
