//! Document highlight: the same-scope occurrence set, with other scopes and
//! comment matches excluded.

mod common;
use common::*;

/// LSP-13: highlighting module A's `power` must never highlight module B's
/// own unrelated `power` declaration or a `// power` comment mention —
/// same binding-identity source as references (T9), not a text scan.
#[test]
fn document_highlight_excludes_other_scope_and_comment_matches() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    // power is computed elsewhere\n\
    param power: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 2.0;\n\
}\n";

    let a_line = src[..src.find("param power").unwrap()].matches('\n').count() as u32;
    let character = "    param ".chars().count() as u32;
    let comment_line = src[..src.find("power is computed").unwrap()].matches('\n').count() as u32;
    let b_line = src[..src.find("mod B").unwrap()].matches('\n').count() as u32;

    let highlights = lsp_document_highlight(src, a_line, character);

    assert!(!highlights.is_empty(), "highlight must return at least the declaration site");
    for h in &highlights {
        assert_ne!(h.range.start.line, comment_line, "a `// power` comment must never be highlighted");
        assert!(h.range.start.line < b_line, "module B's own `power` must never be highlighted from A's cursor");
    }
}
