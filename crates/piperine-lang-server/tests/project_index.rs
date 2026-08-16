//! The project index: one symbol index spanning every project file, and the
//! standalone-document case that has no project unit.
//!
//! **lang-server-vams-migration (T9)**: fixtures migrated to real VAMS.

mod common;
use common::*;

/// LSP-14: a project's `ProjectUnit` covers every `.vams`/`.va` file under
/// `src/` — one `Design` per file, one `ResolutionIndex` spanning both,
/// with every binding's `file` stamped to its owning source path.
///
/// **Blocked** (the feature's documented `span: None` gap, its
/// `index_design` form — see `document_highlight.rs`'s file-level doc):
/// `index_design`'s `insert` skips every span-less binding, and every POM
/// node VAMS lowering produces has `span: None` — so `unit.index` ends up
/// with zero bindings for either file, and the `files` set built from
/// `unit.index.bindings()` is empty. The `ProjectUnit` structure itself
/// (per-file `Design`s, discovered project root) still works correctly —
/// only the cross-file binding index built on top of it is empty.
#[test]
fn project_unit_builds_one_index_spanning_all_project_files() {
    let scratch = ScratchProject::new("multi_file");
    let a_path = scratch.write_src(
        "a.vams",
        "module A(p, n);\n  inout electrical p, n;\n  parameter real gain = 1.0;\nendmodule\n",
    );
    let b_path = scratch.write_src(
        "b.vams",
        "module B(p, n);\n  inout electrical p, n;\n  parameter real gain = 2.0;\nendmodule\n",
    );

    let uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();
    let mut state = piperine_lang_server::state::ServerState::dummy();
    state.documents.insert(uri.clone(), DocumentState::new(std::fs::read_to_string(&a_path).unwrap(), 1));
    state.analyze_document(&uri);

    let doc = state.documents.get(&uri).expect("document must still be present");
    let root = doc.project_root.clone().expect("a.vams must resolve to the scratch project's root");
    assert_eq!(root, scratch.0);

    let unit = state.projects.get(&root).expect("ServerState.projects must hold a unit for the discovered root");
    assert_eq!(unit.designs.len(), 2, "the unit must cover both a.vams and b.vams");
    assert!(unit.designs.contains_key(&a_path));
    assert!(unit.designs.contains_key(&b_path));

    let files: std::collections::HashSet<_> =
        unit.index.bindings().filter_map(|(_, info)| info.file.clone()).collect();
    assert!(
        files.contains(&a_path.display().to_string()),
        "bindings from a.vams must carry a.vams's path, got file set: {files:?}"
    );
    assert!(
        files.contains(&b_path.display().to_string()),
        "bindings from b.vams must carry b.vams's path, got file set: {files:?}"
    );
}

/// LSP-17 (T12's fallback half): a standalone document outside any
/// `Piperine.toml` gets `project_root: None` and no entry in
/// `ServerState.projects` — the existing single-file behavior is
/// unchanged, no regression. Not blocked — this doesn't touch
/// `index_design`/spans at all.
#[test]
fn standalone_document_has_no_project_unit() {
    let uri: Uri = "file:///standalone_lsp_test_file.vams".parse().unwrap();
    let mut state = piperine_lang_server::state::ServerState::dummy();
    state.documents.insert(uri.clone(), DocumentState::new("module Top();\nendmodule\n".to_string(), 1));
    state.analyze_document(&uri);

    let doc = state.documents.get(&uri).unwrap();
    assert!(doc.project_root.is_none(), "a standalone file must not resolve to any project root");
    assert!(state.projects.is_empty(), "no ProjectUnit should be built for a standalone document");
    assert!(doc.design.is_some(), "standalone single-file elaboration must still work (no regression)");
}
