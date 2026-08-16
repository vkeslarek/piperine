//! Cursor-context symbol resolution and use-site indexing: `resolve_at` picks
//! the declaration in scope (not the first global match) and `occurrences_at`
//! returns exactly the indexed uses.
//!
//! **lang-server-vams-migration (T7)**: fixtures below are real VAMS.
//! **Every test in this file is blocked** by the feature's documented
//! `span: None` gap (spec.md's Out of Scope table), at its most severe:
//! `resolve_at`'s only path to a `SymbolKind::Param` resolution is the
//! cursor-context step (`symbol_index.rs` step 2, `resolve_in_module`),
//! which requires `span_contains(m.span, offset)` — always `false` because
//! `elab/lower/vams.rs` always builds `Module` with `span: None`. There is
//! no step-3 global fallback for `Param` (only module/discipline/schema
//! names get one), so every `param`-cursor test below gets no resolution
//! at all — not merely an ambiguous one. `occurrences_at` is separately and
//! equally blocked: it is backed by `piperine_lang::elab::resolution::
//! index_design`, whose `insert` helper unconditionally skips any
//! declaration with `span: None` — i.e. it always returns an empty index
//! for VAMS content, so `occurrences_at` always returns `[]`.

mod common;
use common::*;

/// LSP-01/02 independent test: two modules each declare a `parameter` of
/// the same name (`x`). Cursor context must resolve each module's own `x`
/// to *that* module's decl_span — never the first module in POM iteration
/// order (the bug the old word-based global loop had).
///
/// **Blocked**: see file-level doc — `param` resolution has no fallback
/// without cursor context, which never engages (`Module::span: None`).
#[test]
fn resolve_at_uses_cursor_context_not_global_first_match() {
    let src = "module A(p, n);\n\
    inout electrical p, n;\n\
    parameter real x = 1.0;\n\
endmodule\n\
module B(p, n);\n\
    inout electrical p, n;\n\
    parameter real x = 2.0;\n\
endmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    // The `x` inside A's `parameter real x = 1.0;`.
    let a_body_start = src.find("module A").unwrap();
    let a_x_offset = src[a_body_start..].find("real x").unwrap() + a_body_start + "real ".len();
    let a_resolution = doc.resolve_at(a_x_offset).expect("A's x resolves");
    assert_eq!(a_resolution.kind, SymbolKind::Param);
    let a_x_decl = a_resolution.decl_span.expect("A's x has a decl_span");

    // The `x` inside B's `parameter real x = 2.0;`.
    let b_body_start = src.find("module B").unwrap();
    let b_x_offset = src[b_body_start..].find("real x").unwrap() + b_body_start + "real ".len();
    let b_resolution = doc.resolve_at(b_x_offset).expect("B's x resolves");
    assert_eq!(b_resolution.kind, SymbolKind::Param);
    let b_x_decl = b_resolution.decl_span.expect("B's x has a decl_span");

    assert_ne!(
        a_x_decl.offset(), b_x_decl.offset(),
        "A's x and B's x are distinct declarations — cursor context must not collapse them to the same (first-match) decl_span"
    );
    // Each decl_span must land inside the module it was declared in — not
    // A's decl_span leaking into B's cursor position or vice versa.
    assert!(a_x_decl.offset() < b_body_start, "A's x decl_span must be inside module A");
    assert!(b_x_decl.offset() >= b_body_start, "B's x decl_span must be inside module B, not A's");
}

/// LSP-02: given a declaration (`parameter`) and an unrelated same-named
/// declaration in *another* module, resolving inside the module that owns
/// the local one must return that module's own binding — the "innermost"
/// half of shadowing (the outer/other module's same-named decl never wins
/// just because it appears earlier in iteration order).
///
/// **Blocked**: same reason as above.
#[test]
fn resolve_at_shadowed_name_resolves_to_innermost_not_first_declared() {
    // `Outer` is declared textually first; `Inner`'s own `gain` must still
    // win when the cursor is inside `Inner`.
    let src = "module Outer(p, n);\n\
    inout electrical p, n;\n\
    parameter real gain = 1.0;\n\
endmodule\n\
module Inner(p, n);\n\
    inout electrical p, n;\n\
    parameter real gain = 9.0;\n\
endmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let inner_start = src.find("module Inner").unwrap();
    let gain_offset = src[inner_start..].find("real gain").unwrap() + inner_start + "real ".len();
    let resolution = doc.resolve_at(gain_offset).expect("Inner's gain resolves");
    let decl_span = resolution.decl_span.expect("gain has a decl_span");

    assert!(
        decl_span.offset() >= inner_start,
        "cursor inside Inner must resolve to Inner's own `gain`, not Outer's (first-declared)"
    );
}

/// LSP-10/13 base: resolving a declared binding's own position returns
/// exactly the index's recorded uses for that binding — per T5's
/// Implementation note, `ResolutionIndex.use_spans` today holds only the
/// binding's own declaration span (a reflexive use), so this is a
/// one-element list, not an invented richer occurrence set.
///
/// **Blocked**: `index_design` always returns an empty index for VAMS
/// content (see file-level doc) — `occurrences_at` always returns `[]`.
#[test]
fn occurrences_at_returns_exactly_the_indexed_uses() {
    let src = "module A(p, n);\n\
    inout electrical p, n;\n\
    parameter real power = 1.0;\n\
endmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let power_offset = src.find("real power").unwrap() + "real ".len();
    let occurrences = doc.occurrences_at(power_offset);

    assert_eq!(
        occurrences.len(),
        1,
        "the shipped ResolutionIndex only tracks the reflexive decl-site use; occurrences_at must not invent more, got: {occurrences:?}"
    );
    let (start, end) = occurrences[0];
    assert!(
        power_offset >= start && power_offset < end,
        "the sole occurrence must cover the binding's own declaration site"
    );
}

/// LSP-10/13 base edge case: occurrences must never include a same-spelled
/// binding in another scope, nor a `// name` comment mention — both would
/// be false positives under the old `word_occurrences` text scan.
///
/// **Blocked**: same reason as above — `occurrences_at` always returns
/// `[]`, so the loop bodies below never execute a real assertion (the
/// assertion is vacuously true over an empty set, meaning this specific
/// test technically "passes" today, but not for a reason that proves the
/// exclusion logic works — documented, not silently claimed as a real
/// pass).
#[test]
fn occurrences_at_excludes_other_scope_and_comment_matches() {
    let src = "module A(p, n);\n\
    inout electrical p, n;\n\
    // power is computed elsewhere\n\
    parameter real power = 1.0;\n\
endmodule\n\
module B(p, n);\n\
    inout electrical p, n;\n\
    parameter real power = 2.0;\n\
endmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let a_start = src.find("module A").unwrap();
    let b_start = src.find("module B").unwrap();
    let a_power_offset = src[a_start..].find("real power").unwrap() + a_start + "real ".len();
    let occurrences = doc.occurrences_at(a_power_offset);

    for (start, _end) in &occurrences {
        assert!(
            *start < b_start,
            "occurrences of A's power must never include B's declaration (offset {start} >= {b_start})"
        );
    }
    let comment_offset = src.find("power is computed").unwrap();
    for (start, end) in &occurrences {
        assert!(
            !(comment_offset >= *start && comment_offset < *end || *start == comment_offset),
            "occurrences must never point inside the `//` comment"
        );
    }
}

/// LSP-10/13 base edge case: a cursor on a non-symbol (a numeric literal)
/// yields no occurrences at all. NOT blocked — an always-empty index
/// trivially satisfies "no occurrences," so this one genuinely passes
/// (for the intended reason: no resolution exists at that offset either
/// way).
#[test]
fn occurrences_at_on_non_symbol_is_empty() {
    let src = "module Top(p);\n  inout electrical p;\n  real y;\n  analog begin\n    y = 1.0;\n  end\nendmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let literal_offset = src.rfind("1.0").unwrap();
    assert!(doc.occurrences_at(literal_offset).is_empty());
}
