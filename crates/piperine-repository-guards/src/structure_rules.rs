//! The structure rules (CC-01, CC-02): every crate says what it is for and lists its modules,
//! and no module is named for no concern.

use syn::Item;

use crate::allowance::Findings;
use crate::item_scan::{ScannedKind, ItemScan, CodeOrigin};
use crate::module_declaration::ModuleDeclaration;
use crate::source_tree::SourceTree;

/// Module names that say nothing about their concern.
const EMPTY_MODULE_NAMES: [&str; 5] = ["utils", "util", "helpers", "helper", "misc"];

/// A module named `common` is as empty as those, outside test support.
const SHARED_NAME: &str = "common";

/// The rules over a crate's shape.
pub struct StructureRules;

impl StructureRules {
    /// A crate's root file opens with a `//!` doc that names every module the root declares.
    pub fn crate_docs(tree: &SourceTree) -> Findings {
        let mut findings = Findings::new();
        for file in tree.files() {
            let Some(crate_name) = Self::root_crate(file.path(), file.crate_name()) else { continue };
            let Ok(syntax) = file.syntax() else { continue };
            let doc = Self::doc_text(syntax);
            if doc.trim().is_empty() {
                findings.add(&crate_name, format!("{}::purpose", file.path().display()));
                continue;
            }
            for item in &syntax.items {
                let Item::Mod(module) = item else { continue };
                if ModuleDeclaration::is_cfg_test(&module.attrs) {
                    continue;
                }
                let name = module.ident.to_string();
                if !doc.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|word| word == name) {
                    findings.add(&crate_name, format!("{}::module {name}", file.path().display()));
                }
            }
        }
        findings
    }

    /// The crate the file is the root of (`src/lib.rs`, or `src/main.rs` of a crate with no `lib.rs`).
    fn root_crate(path: &std::path::Path, crate_name: &str) -> Option<String> {
        let name = path.file_name()?.to_str()?;
        let directory = path.parent()?;
        let in_src = directory.ends_with("src") && directory.parent().is_some_and(|crate_directory| crate_directory.file_name().is_some_and(|n| n == crate_name) || crate_directory.as_os_str().is_empty());
        if !in_src {
            return None;
        }
        match name {
            "lib.rs" => Some(crate_name.to_string()),
            "main.rs" if !path.with_file_name("lib.rs").exists() && !directory.join("lib.rs").exists() => Some(crate_name.to_string()),
            _ => None,
        }
    }

    /// The text of the file's `//!` doc comment.
    fn doc_text(file: &syn::File) -> String {
        let mut text = String::new();
        for attribute in file.attrs.iter().filter(|a| a.path().is_ident("doc")) {
            if let syn::Meta::NameValue(pair) = &attribute.meta {
                if let syn::Expr::Lit(literal) = &pair.value {
                    if let syn::Lit::Str(line) = &literal.lit {
                        text.push_str(&line.value());
                        text.push('\n');
                    }
                }
            }
        }
        text
    }

    /// No module is named `utils`, `helpers`, `misc` or `common` outside test support.
    pub fn empty_module_names(scan: &ItemScan) -> Findings {
        let mut findings = Findings::new();
        for item in scan.of_kind(ScannedKind::Module).filter(|item| item.origin == CodeOrigin::Product) {
            if EMPTY_MODULE_NAMES.contains(&item.name.as_str()) || item.name == SHARED_NAME {
                findings.add(&item.crate_name, format!("{}::{}", item.file.display(), item.name));
            }
        }
        findings
    }
}
