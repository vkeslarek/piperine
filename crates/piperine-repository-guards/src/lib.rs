//! The guards over the repository itself, and the scan they share.
//!
//! This crate is test-only and depends on no other crate of the workspace, so a guard cannot be
//! satisfied by the code it judges. It reads the Rust sources as text and as syntax:
//!
//! - `source_tree`: every `.rs` file of the workspace, parsed once and marked product or test code;
//! - `module_declaration`: the `mod x;` and `#[path]` declarations that connect one file to the next;
//! - `item_scan`: one `ScannedItem` per file, module, type, field, variant, `impl`, method,
//!   function, lint attribute and `unwrap`/`expect` call;
//! - `scan_visitor`: the walk over one file's syntax that produces those items;
//! - `feature_file`, `cargo_command`, `test_targets`, `proof_ledger`: the proofs of every feature
//!   folder read and checked against the tests that exist;
//! - `allowance`, `ownership_rules`, `structure_rules`: the ratchet guards, their allowance files
//!   and the rules they judge;
//! - `naming_rules`, `documentation_rules`: the glossary, the dictionary and the doc-comment rules;
//! - `suite_census`, `worklist`, `worklist_rules`: the census of the suite and the inventories that
//!   tasks give verdicts to;
//! - `repoint`: the rewrite that keeps those proofs true while tests move, merge and are removed.
#![deny(missing_docs)]

mod allowance;
mod cargo_command;
mod documentation_rules;
mod feature_file;
mod item_scan;
mod module_declaration;
mod naming_rules;
mod ownership_rules;
mod proof_ledger;
mod repoint;
mod scan_visitor;
mod source_tree;
mod suite_census;
mod structure_rules;
mod test_targets;
mod worklist;
mod worklist_rules;

pub use item_scan::{FreeFunctionPermit, ScannedKind, ItemScan, CodeOrigin, ScannedItem};
pub use module_declaration::ModuleDeclaration;
pub use source_tree::{RustSource, SourceTree};
pub use cargo_command::CargoTestCommand;
pub use feature_file::{CheckLine, FeatureFile, LoneProofLine, ProofLine, RunLine};
pub use proof_ledger::{DanglingReference, ProofLedger};
pub use repoint::{RepointMap, RepointRow, Repointer, RetiredRow, TestReference};
pub use test_targets::{PackageTests, TestTarget, TestTargets};
pub use allowance::{Allowance, Findings, AllowanceVerdict};
pub use ownership_rules::OwnershipRules;
pub use structure_rules::StructureRules;
pub use documentation_rules::DocumentationRules;
pub use naming_rules::{Dictionary, Glossary, NameWords, NamingRules};
pub use suite_census::{CrateCensus, SuiteCensus};
pub use worklist::Worklist;
pub use worklist_rules::WorklistRules;
