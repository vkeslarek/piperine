//! The documentation rules (CC-10): every type, trait, function and method of product code carries a
//! doc comment, and every crate denies missing docs for its public items.

use crate::allowance::Findings;
use crate::item_scan::{ScannedKind, ItemScan, CodeOrigin};
use crate::source_tree::SourceTree;

/// The rules over doc comments.
pub struct DocumentationRules;

impl DocumentationRules {
    const DOCUMENTED: [ScannedKind; 5] = [ScannedKind::Type, ScannedKind::Trait, ScannedKind::Method, ScannedKind::FreeFunction, ScannedKind::NestedFunction];

    /// Every type, trait, function and method of product code is documented; a method of a trait
    /// `impl` is documented by the trait.
    pub fn undocumented_items(scan: &ItemScan) -> Findings {
        let mut findings = Findings::new();
        let undocumented = scan.items().iter().filter(|item| item.origin == CodeOrigin::Product && Self::DOCUMENTED.contains(&item.kind) && !item.documented && !item.is_documentation_exempt());
        for item in undocumented {
            findings.add(&item.crate_name, format!("{}::{}::{}", item.file.display(), item.scope, item.name));
        }
        findings
    }

    /// Every crate root carries `#![deny(missing_docs)]`.
    pub fn crates_without_the_deny(tree: &SourceTree, scan: &ItemScan) -> Findings {
        let mut findings = Findings::new();
        for file in tree.files().iter().filter(|file| file.path().file_name().is_some_and(|name| name == "lib.rs") && file.path().parent().is_some_and(|directory| directory.ends_with("src"))) {
            let denied = scan.of_kind(ScannedKind::LintAttribute).any(|item| item.file == file.path() && item.detail == "inner" && item.name == "deny(missing_docs)");
            if !denied {
                findings.add(file.crate_name(), file.path().display().to_string());
            }
        }
        findings
    }
}
