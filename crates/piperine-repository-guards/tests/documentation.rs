//! The documentation guards (CC-10): every type, trait, function and method of product code carries a
//! doc comment, and every crate root denies missing docs. Ratchets over `allowances/<crate>/<guard>.tsv`.

use piperine_repository_guards::{DocumentationRules, ItemScan, SourceTree};

#[test]
fn every_item_is_documented() {
    let scan = ItemScan::of(&SourceTree::of_workspace());
    DocumentationRules::undocumented_items(&scan).judge("undocumented_items").assert_clean("undocumented_items");
}

#[test]
fn every_crate_denies_missing_docs() {
    let tree = SourceTree::of_workspace();
    let scan = ItemScan::of(&tree);
    DocumentationRules::crates_without_the_deny(&tree, &scan).judge("missing_docs_deny").assert_clean("missing_docs_deny");
}
