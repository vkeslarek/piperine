//! The worklists (CC-06, CC-07, CC-12 AC 2): the inventory of each finding is a file of rows, each row
//! with the verdict a task gave it (`split`, `removed`, `kept: <reason>`, or `-` while it is open).
//! A guard fails on an inventory item with no row and on a row whose item is gone; the open rows are
//! the allowance, and a task closes one by doing what its verdict says.

use std::sync::OnceLock;

use piperine_repository_guards::{ItemScan, SourceTree, Worklist, WorklistRules};

/// The scan of the workspace, taken once for the guards of this target.
static SCAN: OnceLock<ItemScan> = OnceLock::new();

/// Reads the shared scan and judges an inventory against its worklist.
struct WorklistGuards;

impl WorklistGuards {
    /// The shared scan.
    fn scan() -> &'static ItemScan {
        SCAN.get_or_init(|| ItemScan::of(&SourceTree::of_workspace()))
    }

    /// Fails when the inventory and the worklist differ.
    fn assert_clean(name: &str, inventory: std::collections::BTreeSet<String>) {
        let verdict = Worklist::read(name).judge(&inventory);
        assert!(
            verdict.unlisted.is_empty() && verdict.stale.is_empty(),
            "worklists/{name}.tsv: items with no row (add `-`, then give it a verdict):\n{}\nrows with no item (the finding is closed: delete the row):\n{}",
            verdict.unlisted.join("\n"),
            verdict.stale.join("\n")
        );
    }
}

#[test]
fn every_long_function_of_the_inventory_has_a_verdict() {
    WorklistGuards::assert_clean("long_functions", WorklistRules::long_functions(WorklistGuards::scan()));
}

#[test]
fn every_single_implementor_trait_is_removed_or_justified() {
    WorklistGuards::assert_clean("single_implementor_traits", WorklistRules::single_implementor_traits(WorklistGuards::scan()));
}

#[test]
fn every_test_only_item_has_a_verdict() {
    let tree = SourceTree::of_workspace();
    WorklistGuards::assert_clean("test_only_items", WorklistRules::test_only_items(&tree, WorklistGuards::scan()));
}

#[test]
fn every_duplicate_of_the_inventory_has_a_verdict() {
    WorklistGuards::assert_clean("duplicate_tests", WorklistRules::duplicate_tests(WorklistGuards::scan()));
}
