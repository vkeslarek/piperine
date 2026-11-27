//! The structure guards (CC-01, CC-02): every crate says what it is for and lists its modules in the
//! doc of its root, and no module is named for no concern. Ratchets over `allowances/<crate>/<guard>.tsv`.

use piperine_repository_guards::{ItemScan, SourceTree, StructureRules};

#[test]
fn every_crate_states_its_purpose_and_module_map() {
    let tree = SourceTree::of_workspace();
    StructureRules::crate_docs(&tree).judge("crate_docs").assert_clean("crate_docs");
}

#[test]
fn no_module_is_named_for_no_concern() {
    let scan = ItemScan::of(&SourceTree::of_workspace());
    StructureRules::empty_module_names(&scan).judge("empty_module_names").assert_clean("empty_module_names");
}
