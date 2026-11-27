//! The inventories the worklists hold: long functions, traits with one implementor, items only tests
//! reference, and test names defined in more than one place.

use std::collections::{BTreeMap, BTreeSet};

use crate::item_scan::{FreeFunctionPermit, ItemScan, ScannedItem, ScannedKind};
use crate::source_tree::SourceTree;

/// The length above which a function is on the worklist, in lines (findings.md 1.5: not a limit, a reading list).
const LONG_FUNCTION_LINES: usize = 60;

/// The test-only crate: everything it exports exists for the tests that call it, so it has no test-only items.
const TEST_ONLY_CRATE: &str = "piperine-repository-guards";

/// The inventories.
pub struct WorklistRules;

impl WorklistRules {
    /// The worklist key of an item: its file, its scope and its name.
    fn key(item: &ScannedItem) -> String {
        format!("{}::{}::{}", item.file.display(), item.scope, item.name)
    }

    /// Every function, method and closure of product code over 60 lines.
    pub fn long_functions(scan: &ItemScan) -> BTreeSet<String> {
        let functions = [ScannedKind::Method, ScannedKind::FreeFunction, ScannedKind::NestedFunction, ScannedKind::Closure];
        scan.items().iter().filter(|item| functions.contains(&item.kind) && item.origin == crate::item_scan::CodeOrigin::Product && item.length > LONG_FUNCTION_LINES).map(Self::key).collect()
    }

    /// Every trait of product code with one implementor or none, by simple name.
    pub fn single_implementor_traits(scan: &ItemScan) -> BTreeSet<String> {
        let mut implementors: BTreeMap<&str, usize> = BTreeMap::new();
        for item in scan.of_kind(ScannedKind::Impl) {
            if let Some(trait_name) = &item.trait_name {
                *implementors.entry(trait_name.as_str()).or_default() += 1;
            }
        }
        let traits = scan.of_kind(ScannedKind::Trait).filter(|item| item.origin == crate::item_scan::CodeOrigin::Product);
        traits.filter(|item| implementors.get(item.name.as_str()).copied().unwrap_or(0) <= 1).map(|item| format!("{}::{}", item.file.display(), item.name)).collect()
    }

    /// Every type, function and method of product code whose name no other line of product code
    /// holds, and some line of test code does.
    pub fn test_only_items(tree: &SourceTree, scan: &ItemScan) -> BTreeSet<String> {
        let (product, test) = Self::word_counts(tree, scan);
        let named = [ScannedKind::Type, ScannedKind::Trait, ScannedKind::FreeFunction, ScannedKind::Method];
        let candidates = scan.items().iter().filter(|item| named.contains(&item.kind) && item.origin == crate::item_scan::CodeOrigin::Product && item.crate_name != TEST_ONLY_CRATE && item.trait_name.is_none() && item.permit != Some(FreeFunctionPermit::Main));
        candidates.filter(|item| product.get(item.name.as_str()).copied().unwrap_or(0) <= 1 && test.get(item.name.as_str()).copied().unwrap_or(0) > 0).map(Self::key).collect()
    }

    /// The number of lines of product code and of test code each identifier appears on, comments left out.
    fn word_counts(tree: &SourceTree, scan: &ItemScan) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
        let mut test_lines: BTreeMap<&std::path::Path, BTreeSet<usize>> = BTreeMap::new();
        for item in scan.items().iter().filter(|item| item.origin == crate::item_scan::CodeOrigin::Test && item.kind != ScannedKind::File) {
            test_lines.entry(item.file.as_path()).or_default().extend(item.line..item.line + item.length);
        }
        let mut product: BTreeMap<String, usize> = BTreeMap::new();
        let mut test: BTreeMap<String, usize> = BTreeMap::new();
        for file in tree.files() {
            let inside_test = test_lines.get(file.path());
            for (index, line) in file.text().lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let counts = if file.is_test() || inside_test.is_some_and(|lines| lines.contains(&(index + 1))) { &mut test } else { &mut product };
                let words: BTreeSet<&str> = line.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|word| !word.is_empty()).collect();
                for word in words {
                    *counts.entry(word.to_string()).or_default() += 1;
                }
            }
        }
        (product, test)
    }

    /// Every test function name defined in more than one file, with the files.
    pub fn duplicate_tests(scan: &ItemScan) -> BTreeSet<String> {
        let mut files: BTreeMap<&str, BTreeSet<&std::path::Path>> = BTreeMap::new();
        for item in scan.items().iter().filter(|item| item.permit == Some(FreeFunctionPermit::TestAttribute)) {
            files.entry(item.name.as_str()).or_default().insert(item.file.as_path());
        }
        files.into_iter().filter(|(_, defined)| defined.len() > 1).map(|(name, _)| name.to_string()).collect()
    }
}
