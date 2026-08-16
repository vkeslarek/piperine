//! Document highlight: the same-scope occurrence set, with other scopes and
//! comment matches excluded.
//!
//! **lang-server-vams-migration (T6)**: migrated to real Verilog-AMS.
//!
//! **Fully blocked** (a stronger form of the feature's documented
//! `span: None` gap, discovered during migration): `document_highlight`'s
//! handler (`handlers/document_highlight.rs`) calls `DocumentState::
//! occurrences_at`, which is backed entirely by `piperine_lang::elab::
//! resolution::index_design` (`ResolutionIndex`). `index_design`'s own
//! `insert` helper (`elab/resolution.rs`) unconditionally skips any
//! declaration whose POM `span` is `None` (`let Some(span) = span else {
//! return };`) — and *every* POM node kind VAMS lowering produces
//! (`Module`/`Port`/`Param`/`Wire`/`Var`/`Instance`/`Behavior`) is built
//! with a literal `span: None` in `elab/lower/vams.rs`. So for any VAMS
//! document, `index_design` returns a completely EMPTY index — not
//! "sometimes ambiguous," always empty — and `occurrences_at` always
//! returns `[]`, unconditionally, regardless of fixture. This is the same
//! out-of-scope `Module`/`Port`/`Behavior::span` gap named in spec.md's Out
//! of Scope table, just showing up at its most severe: nothing in this
//! file can pass until that elaborator gap closes.

mod common;
use common::*;

/// LSP-13: highlighting module A's `power` must never highlight module B's
/// own unrelated `power` declaration or a `// power` comment mention —
/// same binding-identity source as references (T9), not a text scan.
///
/// **Blocked**: see the file-level doc — `occurrences_at` always returns
/// `[]` for VAMS content (`index_design` skips every span-less binding,
/// and VAMS lowering never populates any POM span), so `highlights` is
/// always empty and the first assertion below can never pass until
/// `Module`/`Port`/`Behavior::span` population lands (out of scope for
/// this feature).
#[test]
fn document_highlight_excludes_other_scope_and_comment_matches() {
    let src = "module A(inout electrical p, n);\n\
    // power is computed elsewhere\n\
    parameter real power = 1.0;\n\
endmodule\n\
module B(inout electrical p, n);\n\
    parameter real power = 2.0;\n\
endmodule\n";

    let a_line = src[..src.find("parameter real power").unwrap()].matches('\n').count() as u32;
    let character = "    parameter real ".chars().count() as u32;
    let comment_line = src[..src.find("power is computed").unwrap()].matches('\n').count() as u32;
    let b_line = src[..src.find("module B").unwrap()].matches('\n').count() as u32;

    let highlights = lsp_document_highlight(src, a_line, character);

    assert!(!highlights.is_empty(), "highlight must return at least the declaration site");
    for h in &highlights {
        assert_ne!(h.range.start.line, comment_line, "a `// power` comment must never be highlighted");
        assert!(h.range.start.line < b_line, "module B's own `power` must never be highlighted from A's cursor");
    }
}
