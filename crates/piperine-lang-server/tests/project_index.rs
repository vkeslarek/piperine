//! The project index: one symbol index spanning every project file, and the
//! standalone-document case that has no project unit.

mod common;
use common::*;

/// LSP-14: a project's `ProjectUnit` covers every `.phdl` file under
/// `src/` — one `Design` per file, one `ResolutionIndex` spanning both,
/// with every binding's `file` stamped to its owning source path.
#[test]
fn project_unit_builds_one_index_spanning_all_project_files() {
    let scratch = ScratchProject::new("multi_file");
    let a_path = scratch.write_src(
        "a.phdl",
        "discipline Electrical { potential v: Real; flow i: Real; }\nmod A (inout p: Electrical, inout n: Electrical) { param gain: Real = 1.0; }\n",
    );
    let b_path = scratch.write_src(
        "b.phdl",
        "discipline Electrical { potential v: Real; flow i: Real; }\nmod B (inout p: Electrical, inout n: Electrical) { param gain: Real = 2.0; }\n",
    );

    let uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();
    let mut state = piperine_lang_server::state::ServerState::dummy();
    state.documents.insert(uri.clone(), DocumentState::new(std::fs::read_to_string(&a_path).unwrap(), 1));
    state.analyze_document(&uri);

    let doc = state.documents.get(&uri).expect("document must still be present");
    let root = doc.project_root.clone().expect("a.phdl must resolve to the scratch project's root");
    assert_eq!(root, scratch.0);

    let unit = state.projects.get(&root).expect("ServerState.projects must hold a unit for the discovered root");
    assert_eq!(unit.designs.len(), 2, "the unit must cover both a.phdl and b.phdl");
    assert!(unit.designs.contains_key(&a_path));
    assert!(unit.designs.contains_key(&b_path));

    let files: std::collections::HashSet<_> =
        unit.index.bindings().filter_map(|(_, info)| info.file.clone()).collect();
    assert!(
        files.contains(&a_path.display().to_string()),
        "bindings from a.phdl must carry a.phdl's path, got file set: {files:?}"
    );
    assert!(
        files.contains(&b_path.display().to_string()),
        "bindings from b.phdl must carry b.phdl's path, got file set: {files:?}"
    );
}

/// LSP-17 (T12's fallback half): a standalone document outside any
/// `Piperine.toml` gets `project_root: None` and no entry in
/// `ServerState.projects` — the existing single-file behavior is
/// unchanged, no regression.
#[test]
fn standalone_document_has_no_project_unit() {
    let uri: Uri = "file:///standalone_lsp_test_file.phdl".parse().unwrap();
    let mut state = piperine_lang_server::state::ServerState::dummy();
    state.documents.insert(uri.clone(), DocumentState::new("mod Top() {}".to_string(), 1));
    state.analyze_document(&uri);

    let doc = state.documents.get(&uri).unwrap();
    assert!(doc.project_root.is_none(), "a standalone file must not resolve to any project root");
    assert!(state.projects.is_empty(), "no ProjectUnit should be built for a standalone document");
    assert!(doc.design.is_some(), "standalone single-file elaboration must still work (no regression)");
}
