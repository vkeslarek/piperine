//! Find-references and rename: both walk the same use index, so both are
//! proven together — including prepare-rename's refusals and the cross-file
//! edit fan-out.

mod common;
use common::*;

/// LSP-10: references on a declared binding return only that binding's
/// recorded occurrences — a `// power` comment mention and an unrelated
/// module's own `power` declaration must never appear.
#[test]
fn references_excludes_comment_and_other_scope_matches() {
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

    let locations = lsp_references(src, a_line, character);

    let comment_line = src[..src.find("power is computed").unwrap()].matches('\n').count() as u32;
    let b_line = src[..src.find("mod B").unwrap()].matches('\n').count() as u32;

    assert!(!locations.is_empty(), "references must return at least the declaration site");
    for loc in &locations {
        assert_ne!(loc.range.start.line, comment_line, "a `// power` comment must never appear in references");
        assert!(loc.range.start.line < b_line, "module B's own `power` must never appear in module A's references");
    }
}

/// LSP-11: renaming a `power` param declared in module A must not edit
/// module B's own unrelated `power` param.
#[test]
fn rename_edits_only_the_binding_uses_other_scope_untouched() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 2.0;\n\
}\n";

    let a_line = src[..src.find("param power").unwrap()].matches('\n').count() as u32;
    let character = "    param ".chars().count() as u32;
    let b_line = src[..src.find("mod B").unwrap()].matches('\n').count() as u32;

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
#[test]
fn prepare_rename_declines_on_literal() {
    let src = "mod Top() {}\ndigital Top { var y: Real = 1.0; }";
    let line = src[..src.rfind("1.0").unwrap()].matches('\n').count() as u32;
    let line_start = src[..src.rfind("1.0").unwrap()].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (src.rfind("1.0").unwrap() - line_start) as u32;

    let response = lsp_prepare_rename(src, line, character);
    assert!(response.is_none(), "prepare-rename must decline on a numeric literal, got: {response:?}");
}

/// LSP-12 Independent Test: renaming a module used across a two-file
/// project (`b.phdl` `use`-imports and instantiates a module declared in
/// `a.phdl`) edits both files via `WorkspaceEdit.document_changes` — the
/// declaration in `a.phdl` and the instantiation's type name in `b.phdl`.
#[test]
fn cross_file_rename_edits_every_referencing_file() {
    let scratch = ScratchProject::new("rename_cross_file");
    let a_src = "pub discipline Electrical { potential v: Real; flow i: Real; }\npub mod A (inout p: Electrical, inout n: Electrical) { param gain: Real = 1.0; }\n";
    let a_path = scratch.write_src("a.phdl", a_src);
    let b_src = "use scratch_proj::a;\nmod B (inout p: Electrical, inout n: Electrical) {\n    inst: A(.p = p, .n = n);\n}\n";
    let b_path = scratch.write_src("b.phdl", b_src);

    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();
    // `    inst: A(...)` — cursor on the `A` type name.
    let line = 2u32;
    let character = "    inst: ".chars().count() as u32;

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
        .expect("a.phdl (the declaring file) must have an edit");
    assert_eq!(a_edit.edits.len(), 1, "a.phdl gets exactly one edit: the module's own name");
    let a_new_text = match &a_edit.edits[0] {
        lsp_types::OneOf::Left(te) => &te.new_text,
        lsp_types::OneOf::Right(ate) => &ate.text_edit.new_text,
    };
    assert_eq!(a_new_text, "Amp");

    let b_edit = document_changes.iter().find(|e| e.text_document.uri == b_uri)
        .expect("b.phdl (the referencing file) must have an edit");
    assert_eq!(b_edit.edits.len(), 1, "b.phdl gets exactly one edit: the instance's type name");
    let b_new_text = match &b_edit.edits[0] {
        lsp_types::OneOf::Left(te) => &te.new_text,
        lsp_types::OneOf::Right(ate) => &ate.text_edit.new_text,
    };
    assert_eq!(b_new_text, "Amp");

    // The edited range in a.phdl must fall on `A`'s own name, not
    // anywhere else in the declaration (e.g. inside the port list).
    let a_edit_start = position_to_byte(a_src, match &a_edit.edits[0] {
        lsp_types::OneOf::Left(te) => te.range.start,
        lsp_types::OneOf::Right(ate) => ate.text_edit.range.start,
    });
    let a_name_offset = a_src.find("pub mod A").unwrap() + "pub mod ".len();
    assert_eq!(a_edit_start, a_name_offset, "a.phdl's edit must target `A`'s own name token");
}
