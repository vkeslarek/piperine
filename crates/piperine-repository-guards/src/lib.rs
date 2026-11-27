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
//! - `repoint`: the rewrite that keeps those proofs true while tests move, merge and are removed.
#![deny(missing_docs)]

mod cargo_command;
mod feature_file;
mod item_scan;
mod module_declaration;
mod proof_ledger;
mod repoint;
mod scan_visitor;
mod source_tree;
mod test_targets;

pub use item_scan::{FreeFunctionPermit, ItemKind, ItemScan, Origin, ScannedItem};
pub use module_declaration::ModuleDeclaration;
pub use source_tree::{SourceFile, SourceTree};
pub use cargo_command::CargoTestCommand;
pub use feature_file::{CheckLine, FeatureFile, LoneProofLine, ProofLine, RunLine};
pub use proof_ledger::{DanglingReference, ProofLedger};
pub use repoint::{RepointMap, RepointRow, Repointer, RetiredRow, TestReference};
pub use test_targets::{PackageTests, TestTarget, TestTargets};
