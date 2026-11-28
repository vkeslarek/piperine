//! The ownership rules (CC-04): a function belongs to a type, an `impl` lives with its type, no
//! `#[allow]` hides a design problem, and every owner the design names exists with its methods.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::allowance::Findings;
use crate::item_scan::{ScannedKind, ItemScan, CodeOrigin, ScannedItem};
use crate::module_declaration::ModuleDeclaration;
use crate::source_tree::SourceTree;

/// The rules over who owns what.
pub struct OwnershipRules;

impl OwnershipRules {
    /// The allowance key of an item: its file and its name.
    fn file_key(item: &ScannedItem) -> String {
        format!("{}::{}", item.file.display(), item.name)
    }

    /// Product code defines no free function (or nested function) but `main`, an `extern` function
    /// and a `#[test]`.
    pub fn free_functions(scan: &ItemScan) -> Findings {
        Self::functions_of(scan, CodeOrigin::Product)
    }

    /// Test code defines no free helper either: its helpers are methods of fixture types.
    pub fn test_helper_functions(scan: &ItemScan) -> Findings {
        Self::functions_of(scan, CodeOrigin::Test)
    }

    /// Whether the file is a golden: under a crate's `tests/golden/`. A golden is generated transpiler output pinned
    /// byte for byte (CC-09), so the free functions it holds are the generator's, not a test's helpers; this is the
    /// one named exclusion of `test_helper_functions`.
    fn is_golden(file: &Path) -> bool {
        let parts: Vec<_> = file.components().map(|c| c.as_os_str()).collect();
        parts.windows(2).any(|pair| pair[0] == "tests" && pair[1] == "golden")
    }

    /// The free and nested functions of code of the given origin that are not permitted (a golden's are left out).
    fn functions_of(scan: &ItemScan, origin: CodeOrigin) -> Findings {
        let mut findings = Findings::new();
        for item in scan.items().iter().filter(|item| item.origin == origin && !(origin == CodeOrigin::Test && Self::is_golden(&item.file))) {
            let loose = item.is_unpermitted_free_function() || item.kind == ScannedKind::NestedFunction;
            if loose {
                findings.add(&item.crate_name, Self::file_key(item));
            }
        }
        findings
    }

    /// An inherent `impl T` is in the file that declares `T` or below that file's directory.
    pub fn impls_away_from_their_type(scan: &ItemScan) -> Findings {
        let mut declarations: BTreeMap<(&str, &str), Vec<&Path>> = BTreeMap::new();
        for item in scan.items().iter().filter(|item| matches!(item.kind, ScannedKind::Type)) {
            declarations.entry((item.crate_name.as_str(), item.name.as_str())).or_default().push(&item.file);
        }
        let mut findings = Findings::new();
        for item in scan.of_kind(ScannedKind::Impl).filter(|item| item.trait_name.is_none()) {
            let Some(files) = declarations.get(&(item.crate_name.as_str(), item.name.as_str())) else { continue };
            if !files.iter().any(|declared| Self::is_with(&item.file, declared)) {
                findings.add(&item.crate_name, Self::file_key(item));
            }
        }
        findings
    }

    /// Whether `file` is the declaring file or lies below the module the declaring file opens.
    fn is_with(file: &Path, declared: &Path) -> bool {
        if file == declared {
            return true;
        }
        let name = declared.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let module_directory: PathBuf = if matches!(name, "mod.rs" | "lib.rs" | "main.rs") { declared.parent().unwrap_or(Path::new("")).to_path_buf() } else { declared.with_extension("") };
        file.starts_with(module_directory)
    }

    /// No `#[allow]` or `#[expect]` in product code (which the lint list of generated code, a string, is not).
    pub fn allow_attributes(scan: &ItemScan) -> Findings {
        Self::allows_of(scan, CodeOrigin::Product)
    }

    /// No `#[allow]` or `#[expect]` in test code either: a fixture that needs one is shaped wrong.
    pub fn allow_in_test_code(scan: &ItemScan) -> Findings {
        Self::allows_of(scan, CodeOrigin::Test)
    }

    /// The `#[allow]` and `#[expect]` attributes of code of the given origin.
    fn allows_of(scan: &ItemScan, origin: CodeOrigin) -> Findings {
        let mut findings = Findings::new();
        let lints = scan.of_kind(ScannedKind::LintAttribute).filter(|item| item.origin == origin);
        for item in lints.filter(|item| item.name.starts_with("allow(") || item.name.starts_with("expect(")) {
            findings.add(&item.crate_name, Self::file_key(item));
        }
        findings
    }

    /// Test code shares no support file by `#[path = "..."]`: what two targets share is a type of the
    /// fixtures crate, not a file they both include.
    pub fn test_support_shared_by_path(tree: &SourceTree) -> Findings {
        let mut findings = Findings::new();
        for file in tree.files().iter().filter(|file| file.is_test()) {
            let Ok(syntax) = file.syntax() else { continue };
            for declaration in ModuleDeclaration::of_file(syntax).iter().filter(|declaration| declaration.path_attribute.is_some()) {
                findings.add(file.crate_name(), format!("{}::{}", file.path().display(), declaration.name));
            }
        }
        findings
    }

    /// Every owner of `owners.tsv` (`crate<TAB>type<TAB>task`) is a type of its crate with at least one method.
    pub fn missing_owners(scan: &ItemScan, owners: &str) -> Findings {
        let mut findings = Findings::new();
        for line in owners.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')) {
            let cells: Vec<&str> = line.split('\t').collect();
            let [crate_name, owner, ..] = cells.as_slice() else { continue };
            let declared = scan.items().iter().any(|i| matches!(i.kind, ScannedKind::Type | ScannedKind::Trait) && i.crate_name == *crate_name && i.name == *owner);
            let has_methods = scan.of_kind(ScannedKind::Method).any(|m| m.crate_name == *crate_name && m.scope == *owner);
            if !(declared && has_methods) {
                findings.add(crate_name, (*owner).to_string());
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_under_tests_golden_are_left_out_of_the_helper_guard() {
        let scan = ItemScan::of(&SourceTree::of_sources(&[
            ("crates/demo/tests/golden/diode.rs", "fn generated() {}\n"),
            ("crates/demo/tests/other/helper.rs", "fn planted() {}\n"),
            ("crates/demo/tests/golden_like/helper.rs", "fn lookalike() {}\n"),
        ]));
        let findings = OwnershipRules::test_helper_functions(&scan);
        assert_eq!(findings.count(), 2);
        assert!(!OwnershipRules::is_golden(Path::new("crates/demo/tests/other/helper.rs")));
        assert!(OwnershipRules::is_golden(Path::new("crates/demo/tests/golden/diode.rs")));
    }
}
