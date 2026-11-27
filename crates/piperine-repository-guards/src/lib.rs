//! The guards over the repository itself, and the scan they share.
//!
//! This crate is test-only and depends on no other crate of the workspace, so a guard cannot be
//! satisfied by the code it judges. It reads the Rust sources as text and as syntax:
//!
//! - `source_tree`: every `.rs` file of the workspace, parsed once and marked product or test code;
//! - `module_declaration`: the `mod x;` and `#[path]` declarations that connect one file to the next;
//! - `item_scan`: one `ScannedItem` per file, module, type, field, variant, `impl`, method,
//!   function, lint attribute and `unwrap`/`expect` call;
//! - `scan_visitor`: the walk over one file's syntax that produces those items.
#![deny(missing_docs)]

mod item_scan;
mod module_declaration;
mod scan_visitor;
mod source_tree;

pub use item_scan::{FreeFunctionPermit, ItemKind, ItemScan, Origin, ScannedItem};
pub use module_declaration::ModuleDeclaration;
pub use source_tree::{SourceFile, SourceTree};
