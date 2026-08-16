//! Find-references and rename: both walk the same use index, so both are
//! proven together — including prepare-rename's refusals and the cross-file
//! edit fan-out.
//!
//! **lang-server-vams-migration (T9)**: fixtures migrated to real VAMS.
//! Most of this file is blocked by the feature's documented `span: None`
//! gap: `references`/`rename` are backed by `occurrences_at`
//! (`index_design`, always empty for VAMS — see `document_highlight.rs`'s
//! file-level doc) and `rename` additionally requires `resolution.
//! decl_span` up front (`handlers/rename.rs`, always `None` — see
//! `protocol.rs`'s file-level doc). Cross-file rename is blocked a third
//! way too (no VAMS `use`, no `item_files` population — see
//! `import_goto.rs`'s file-level doc).

mod common;
use common::*;

/// LSP-10: references on a declared binding return only that binding's
/// recorded occurrences — a `// power` comment mention and an unrelated
/// module's own `power` declaration must never appear.
///
/// **Blocked**: `occurrences_at` is always empty for VAMS content.
#[test]
fn references_excludes_comment_and_other_scope_matches() {
    let src = "module A(p, n);\n\
  inout electrical p, n;\n\
  // power is computed elsewhere\n\
  parameter real power = 1.0;\n\
endmodule\n\
module B(p, n);\n\
  inout electrical p, n;\n\
  parameter real power = 2.0;\n\
endmodule\n";

    let a_line = src[..src.find("real power").unwrap()].matches('\n').count() as u32;
    let character = "  parameter real ".chars().count() as u32;

    let locations = lsp_references(src, a_line, character);

    let comment_line = src[..src.find("power is computed").unwrap()].matches('\n').count() as u32;
    let b_line = src[..src.find("module B").unwrap()].matches('\n').count() as u32;

    assert!(!locations.is_empty(), "references must return at least the declaration site");
    for loc in &locations {
        assert_ne!(loc.range.start.line, comment_line, "a `// power` comment must never appear in references");
        assert!(loc.range.start.line < b_line, "module B's own `power` must never appear in module A's references");
    }
}

/// LSP-11: renaming a `power` param declared in module A must not edit
/// module B's own unrelated `power` param.
///
/// **Blocked**: `handle_rename` requires `resolution.decl_span` as its
/// first step; `Param::span` is always `None` for VAMS with no
/// cursor-context fallback.
#[test]
fn rename_edits_only_the_binding_uses_other_scope_untouched() {
    let src = "module A(p, n);\n\
  inout electrical p, n;\n\
  parameter real power = 1.0;\n\
endmodule\n\
module B(p, n);\n\
  inout electrical p, n;\n\
  parameter real power = 2.0;\n\
endmodule\n";

    let a_line = src[..src.find("real power").unwrap()].matches('\n').count() as u32;
    let character = "  parameter real ".chars().count() as u32;
    let b_line = src[..src.find("module B").unwrap()].matches('\n').count() as u32;

    let edit = lsp_rename(src, a_line, character, "gain").expect("rename on A's power must succeed");
    let changes: HashMap<String, _> = edit.changes
        .expect("rename must produce changes")
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let uri: Uri = "file:///rename_test.phdl".parse().unwrap();
    let edits = changes.get(&uri.to_string()).expect("changes must target the open document");

    assert!(!edits.is_empty(), "at least the declaration site must be edited");
    for e in edits {
        assert_eq!(e.new_text, "gain");
        assert!(e.range.start.line < b_line, "module B's own `power` must never be edited by A's rename");
    }
}

/// LSP-11 edge case: prepare-rename declines (returns `None`) on a
/// non-renameable token — here, a numeric literal.
///
/// NOT blocked in the sense of failing — this assertion passes — but for a
/// broader reason than originally intended: `occurrences_at` is always
/// empty for ANY position in VAMS content today (the `span: None` gap), so
/// prepare-rename declines everywhere, not specifically because a numeric
/// literal never resolves. A real regression in "declines only on
/// non-symbols specifically" would not be caught by this test until
/// `occurrences_at` works again for real symbols too.
#[test]
fn prepare_rename_declines_on_literal() {
    let src = "module Top();\nendmodule\ndigital Top { var y: real = 1.0; }";
    let line = src[..src.rfind("1.0").unwrap()].matches('\n').count() as u32;
    let line_start = src[..src.rfind("1.0").unwrap()].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (src.rfind("1.0").unwrap() - line_start) as u32;

    let response = lsp_prepare_rename(src, line, character);
    assert!(response.is_none(), "prepare-rename must decline on a numeric literal, got: {response:?}");
}

/// LSP-12 Independent Test: renaming a module used across a two-file
/// project edits both files via `WorkspaceEdit.document_changes` — the
/// declaration in `a.vams` and the instantiation's type name in `b.vams`.
///
/// **Blocked, doubly**: no VAMS `use` (retargeted onto `` `include ``,
/// the closest equivalent — see `protocol.rs`'s cross-file test for the
/// same substitution), and `handle_rename`'s `resolution.decl_span?`
/// first step is always `None` for `Module` regardless — see this file's
/// header doc.
#[test]
fn cross_file_rename_edits_every_referencing_file() {
    let scratch = ScratchProject::new("rename_cross_file");
    let a_src = "module A(p, n);\n  inout electrical p, n;\n  parameter real gain = 1.0;\nendmodule\n";
    let a_path = scratch.write_src("a.vams", a_src);
    let b_src = "`include \"a.vams\"\nmodule B(p, n);\n  inout electrical p, n;\n  A inst(.p(p), .n(n));\nendmodule\n";
    let b_path = scratch.write_src("b.vams", b_src);

    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();
    // `  A inst(...)` — cursor on the `A` type name.
    let line = b_src[..b_src.find("A inst").unwrap()].matches('\n').count() as u32;
    let character = "  ".chars().count() as u32;

    let edit = lsp_rename_at(&b_uri, b_src, line, character, "Amp")
        .expect("renaming a cross-file module must produce a WorkspaceEdit");

    let document_changes = match edit.document_changes {
        Some(DocumentChanges::Edits(edits)) => edits,
        other => panic!("expected document_changes edits, got: {other:?}"),
    };
    assert_eq!(
        document_changes.len(), 2,
        "cross-file rename must edit both the declaring file and the referencing file"
    );

    let a_uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();

    let a_edit = document_changes.iter().find(|e| e.text_document.uri == a_uri)
        .expect("a.vams (the declaring file) must have an edit");
    assert_eq!(a_edit.edits.len(), 1, "a.vams gets exactly one edit: the module's own name");
    let a_new_text = match &a_edit.edits[0] {
        lsp_types::OneOf::Left(te) => &te.new_text,
        lsp_types::OneOf::Right(ate) => &ate.text_edit.new_text,
    };
    assert_eq!(a_new_text, "Amp");

    let b_edit = document_changes.iter().find(|e| e.text_document.uri == b_uri)
        .expect("b.vams (the referencing file) must have an edit");
    assert_eq!(b_edit.edits.len(), 1, "b.vams gets exactly one edit: the instance's type name");
    let b_new_text = match &b_edit.edits[0] {
        lsp_types::OneOf::Left(te) => &te.new_text,
        lsp_types::OneOf::Right(ate) => &ate.text_edit.new_text,
    };
    assert_eq!(b_new_text, "Amp");

    // The edited range in a.vams must fall on `A`'s own name, not
    // anywhere else in the declaration (e.g. inside the port list).
    let a_edit_start = position_to_byte(a_src, match &a_edit.edits[0] {
        lsp_types::OneOf::Left(te) => te.range.start,
        lsp_types::OneOf::Right(ate) => ate.text_edit.range.start,
    });
    let a_name_offset = a_src.find("module A").unwrap() + "module ".len();
    assert_eq!(a_edit_start, a_name_offset, "a.vams's edit must target `A`'s own name token");
}
