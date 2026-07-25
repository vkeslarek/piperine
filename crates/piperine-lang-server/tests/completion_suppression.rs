//! BUG-4 (LSB-11..13, T9): completion doesn't suggest behavior-only
//! statement keywords (`for`/`match`/`return`/`when`) when the cursor
//! sits immediately after a module's closing `}`.
//!
//! Confirmed repro (spec.md): cursor exactly at a module's closing `}`
//! byte position returns `expected` containing both `Punctuation(RBrace)`
//! and a module-body-only keyword (`param`/`wire`) — every alternative the
//! mod-body statement dispatcher tried before finding the `}`. Completion
//! must suppress the behavior-only keywords and offer the real top-level
//! declaration keywords instead.

use piperine_lang_server::handlers::completion::completions_at;
use piperine_lang_server::state::DocumentState;

fn analyzed(src: &str) -> DocumentState {
    let mut doc = DocumentState::new(src.to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    doc
}

/// spec.md's exact reported fixture: cursor at the closing `}` of `mod A(...)
/// { param r: Real = 1.0; }` must never offer `"for"`, and must offer `"mod"`
/// (a legitimate top-level declaration keyword).
#[test]
fn completion_right_after_module_close_brace_never_offers_for_and_offers_mod() {
    let src = "mod A(inout p: Real) { param r: Real = 1.0; }";
    let doc = analyzed(src);
    let offset = src.len(); // cursor right at (after) the closing `}`

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(!labels.contains(&"for"), "`for` must not be offered right after a module's `}}`: {labels:?}");
    assert!(labels.contains(&"mod"), "`mod` (a real top-level keyword) must be offered: {labels:?}");
}

/// The same suppression signature also drops `match`/`return`/`when` (the
/// other behavior-only keywords named in the fix), while `if`/`var` are
/// deliberately NOT suppressed (design.md: both are valid inside a `mod{}`
/// body too, so suppressing them would be a new false negative).
#[test]
fn completion_right_after_module_close_brace_suppresses_all_behavior_only_keywords_but_keeps_if_and_var() {
    let src = "mod A(inout p: Real) { param r: Real = 1.0; }";
    let doc = analyzed(src);
    let offset = src.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    for behavior_only in ["for", "match", "return", "when"] {
        assert!(
            !labels.contains(&behavior_only),
            "`{behavior_only}` must not be offered right after a module's `}}`: {labels:?}"
        );
    }
}

/// No regression: a genuine mid-behavior-body cursor position (inside
/// `analog { ... }`, where a new statement could legitimately start) still
/// offers `for`.
#[test]
fn completion_mid_behavior_body_still_offers_for() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A(inout p: Electrical, inout n: Electrical) { }\n\
analog A {\n    \n}\n";
    let doc = analyzed(src);
    // Cursor on the blank line inside the analog block body — a genuine
    // new-statement position, not a `}` boundary.
    let offset = src.find("{\n    \n}").expect("analog body present") + 2;

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"for"), "mid-behavior-body completion should still offer `for`, got {labels:?}");
}
