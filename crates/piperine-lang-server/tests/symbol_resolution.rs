//! Cursor-context symbol resolution and use-site indexing: `resolve_at` picks
//! the declaration in scope (not the first global match) and `occurrences_at`
//! returns exactly the indexed uses.

mod common;
use common::*;

/// LSP-01/02 independent test: two modules each declare a `param` of the
/// same name (`x`). Cursor context must resolve each module's own `x` to
/// *that* module's decl_span — never the first module in POM iteration
/// order (the bug the old word-based global loop had).
#[test]
fn resolve_at_uses_cursor_context_not_global_first_match() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    param x: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param x: Real = 2.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    // The `x` inside A's `param x: Real = 1.0;`.
    let a_body_start = src.find("mod A").unwrap();
    let a_x_offset = src[a_body_start..].find("param x").unwrap() + a_body_start + "param ".len();
    let a_resolution = doc.resolve_at(a_x_offset).expect("A's x resolves");
    assert_eq!(a_resolution.kind, SymbolKind::Param);
    let a_x_decl = a_resolution.decl_span.expect("A's x has a decl_span");

    // The `x` inside B's `param x: Real = 2.0;`.
    let b_body_start = src.find("mod B").unwrap();
    let b_x_offset = src[b_body_start..].find("param x").unwrap() + b_body_start + "param ".len();
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

/// LSP-02: given a declaration (`param`) and an unrelated same-named
/// declaration in *another* module, resolving inside the module that owns
/// the local one must return that module's own binding — the "innermost"
/// half of shadowing (the outer/other module's same-named decl never wins
/// just because it appears earlier in iteration order).
#[test]
fn resolve_at_shadowed_name_resolves_to_innermost_not_first_declared() {
    // `Outer` is declared textually first; `Inner`'s own `gain` must still
    // win when the cursor is inside `Inner`.
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod Outer (inout p: Electrical, inout n: Electrical) {\n\
    param gain: Real = 1.0;\n\
}\n\
mod Inner (inout p: Electrical, inout n: Electrical) {\n\
    param gain: Real = 9.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let inner_start = src.find("mod Inner").unwrap();
    let gain_offset = src[inner_start..].find("param gain").unwrap() + inner_start + "param ".len();
    let resolution = doc.resolve_at(gain_offset).expect("Inner's gain resolves");
    let decl_span = resolution.decl_span.expect("gain has a decl_span");

    assert!(
        decl_span.offset() >= inner_start,
        "cursor inside Inner must resolve to Inner's own `gain`, not Outer's (first-declared)"
    );
}

/// LSP-10/13 base: resolving a declared binding's own position returns
/// exactly the index's recorded uses for that binding — per T5's
/// SPEC_DEVIATION, `ResolutionIndex.use_spans` today holds only the
/// binding's own declaration span (a reflexive use), so this is a
/// one-element list, not an invented richer occurrence set.
#[test]
fn occurrences_at_returns_exactly_the_indexed_uses() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 1.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let power_offset = src.find("param power").unwrap() + "param ".len();
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
#[test]
fn occurrences_at_excludes_other_scope_and_comment_matches() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    // power is computed elsewhere\n\
    param power: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 2.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let a_start = src.find("mod A").unwrap();
    let b_start = src.find("mod B").unwrap();
    let a_power_offset = src[a_start..].find("param power").unwrap() + a_start + "param ".len();
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
/// yields no occurrences at all.
#[test]
fn occurrences_at_on_non_symbol_is_empty() {
    let src = "mod Top() {}\ndigital Top { var y: Real = 1.0; }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let literal_offset = src.rfind("1.0").unwrap();
    assert!(doc.occurrences_at(literal_offset).is_empty());
}
