//! The ownership rules (CC-04): a function belongs to a type, an `impl` lives with its type, no
//! `#[allow]` hides a design problem, and every owner the design names exists with its methods.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::allowance::Findings;
use crate::item_scan::{ItemKind, ItemScan, Origin, ScannedItem};

/// The rules over who owns what.
pub struct OwnershipRules;

impl OwnershipRules {
    fn file_key(item: &ScannedItem) -> String {
        format!("{}::{}", item.file.display(), item.name)
    }

    /// Product code defines no free function (or nested function) but `main`, an `extern` function
    /// and a `#[test]`.
    pub fn free_functions(scan: &ItemScan) -> Findings {
        Self::functions_of(scan, Origin::Product)
    }

    /// Test code defines no free helper either: its helpers are methods of fixture types.
    pub fn test_helper_functions(scan: &ItemScan) -> Findings {
        Self::functions_of(scan, Origin::Test)
    }

    fn functions_of(scan: &ItemScan, origin: Origin) -> Findings {
        let mut findings = Findings::new();
        for item in scan.items().iter().filter(|item| item.origin == origin) {
            let loose = item.is_unpermitted_free_function() || item.kind == ItemKind::NestedFunction;
            if loose {
                findings.add(&item.crate_name, Self::file_key(item));
            }
        }
        findings
    }

    /// An inherent `impl T` is in the file that declares `T` or below that file's directory.
    pub fn impls_away_from_their_type(scan: &ItemScan) -> Findings {
        let mut declarations: BTreeMap<(&str, &str), Vec<&Path>> = BTreeMap::new();
        for item in scan.items().iter().filter(|item| matches!(item.kind, ItemKind::Type)) {
            declarations.entry((item.crate_name.as_str(), item.name.as_str())).or_default().push(&item.file);
        }
        let mut findings = Findings::new();
        for item in scan.of_kind(ItemKind::Impl).filter(|item| item.trait_name.is_none()) {
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

    /// No `#[allow]` or `#[expect]` outside the lint list of generated code (which is a string, not an attribute).
    pub fn allow_attributes(scan: &ItemScan) -> Findings {
        let mut findings = Findings::new();
        for item in scan.of_kind(ItemKind::LintAttribute).filter(|item| item.name.starts_with("allow(") || item.name.starts_with("expect(")) {
            findings.add(&item.crate_name, Self::file_key(item));
        }
        findings
    }

    /// Every owner of `owners.tsv` (`crate<TAB>type<TAB>task`) is a type of its crate with at least one method.
    pub fn missing_owners(scan: &ItemScan, owners: &str) -> Findings {
        let mut findings = Findings::new();
        for line in owners.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')) {
            let cells: Vec<&str> = line.split('\t').collect();
            let [crate_name, owner, ..] = cells.as_slice() else { continue };
            let declared = scan.items().iter().any(|i| matches!(i.kind, ItemKind::Type | ItemKind::Trait) && i.crate_name == *crate_name && i.name == *owner);
            let has_methods = scan.of_kind(ItemKind::Method).any(|m| m.crate_name == *crate_name && m.scope == *owner);
            if !(declared && has_methods) {
                findings.add(crate_name, (*owner).to_string());
            }
        }
        findings
    }
}
