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
//!
//! **lang-server-vams-migration (T7)**: fixtures below are real VAMS.
//! **Two of the three tests below are permanently blocked** by a separate,
//! already-shipped architecture change discovered during migration — NOT
//! the feature's documented `span: None` gap, and NOT a PHDL-syntax issue:
//! `handlers/completion.rs`'s own module doc states plainly that the
//! PHDL-era predictive parser this cursor-aware suppression depended on
//! (`piperine_lang::parse::predict`) was deleted at T21 (MD-45), and
//! `completions_at` now returns a **static, non-context-aware** keyword
//! list unconditionally (confirmed: no `BUG-4`/`suppress`/`RBrace` string
//! exists anywhere in `piperine-lang-server/src/` anymore). `for`/`match`/
//! `return`/`when` are always in that list, at every cursor position — so
//! "never offers `for` right after `}`" cannot pass under the current
//! architecture. This is out of scope for this feature (a lang-server
//! wiring fix, not new-capability work — see spec.md's Out of Scope
//! table's "New LSP capabilities... not already implemented").

use piperine_lang_server::handlers::completion::completions_at;
use piperine_lang_server::state::DocumentState;

fn analyzed(src: &str) -> DocumentState {
    let mut doc = DocumentState::new(src.to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    doc
}

/// spec.md's exact reported fixture, migrated: cursor right after
/// `endmodule` of `module A(p); parameter real r = 1.0; endmodule` must
/// never offer `"for"`, and must offer `"module"` (a legitimate top-level
/// declaration keyword — VAMS spells this `module`, not PHDL's `mod`).
///
/// **Blocked**: see file-level doc — cursor-aware suppression was removed
/// with the PHDL predictive parser (T37/MD-45); `completions_at` always
/// offers `for`.
#[test]
fn completion_right_after_module_close_never_offers_for_and_offers_module() {
    let src = "module A(p);\n  input real p;\n  parameter real r = 1.0;\nendmodule\n";
    let doc = analyzed(src);
    let offset = src.len(); // cursor right after `endmodule`

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(!labels.contains(&"for"), "`for` must not be offered right after a module's close: {labels:?}");
    assert!(labels.contains(&"module"), "`module` (a real top-level keyword) must be offered: {labels:?}");
}

/// The same suppression signature also drops `match`/`return`/`when` (the
/// other behavior-only keywords named in the fix), while `if`/`var` are
/// deliberately NOT suppressed (design.md: both are valid inside a module
/// body too, so suppressing them would be a new false negative).
///
/// **Blocked**: same reason as above.
#[test]
fn completion_right_after_module_close_suppresses_all_behavior_only_keywords_but_keeps_if_and_var() {
    let src = "module A(p);\n  input real p;\n  parameter real r = 1.0;\nendmodule\n";
    let doc = analyzed(src);
    let offset = src.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    for behavior_only in ["for", "match", "return", "when"] {
        assert!(
            !labels.contains(&behavior_only),
            "`{behavior_only}` must not be offered right after a module's close: {labels:?}"
        );
    }
}

/// No regression: a genuine mid-behavior-body cursor position (inside
/// `analog begin ... end`, where a new statement could legitimately start)
/// still offers `for`. Not blocked — `completions_at` offers `for`
/// unconditionally today, so this trivially (and correctly) still holds.
#[test]
fn completion_mid_behavior_body_still_offers_for() {
    let src = "module A(p, n);\n  inout electrical p, n;\n  analog begin\n    \n  end\nendmodule\n";
    let doc = analyzed(src);
    // Cursor on the blank line inside the analog block body — a genuine
    // new-statement position, not a `}`/`end` boundary.
    let offset = src.find("begin\n    \n  end").expect("analog body present") + "begin\n".len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"for"), "mid-behavior-body completion should still offer `for`, got {labels:?}");
}
