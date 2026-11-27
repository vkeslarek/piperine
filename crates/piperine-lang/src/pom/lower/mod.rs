//! Stage 1: the syntax tree to the arenas. A pure function of the tree: it
//! reads no scope and resolves nothing. Only this module pushes to the arenas.
//!
//! `lowerer.rs` holds the walk state and the declaration spine; `body.rs` lowers
//! paths, types, patterns, expressions and statements; `items.rs` lowers the
//! value items.

mod body;
mod files;
mod hardware;
mod items;
mod lowerer;
mod meta;

use std::path::PathBuf;

use super::design::Design;
use super::ids::{DeclId, FileId};
use super::source::SourceProvider;

/// The state of one lowering walk: the design under construction, the file
/// being read and the nearest enclosing declaration.
pub(crate) struct Lowerer<'a> {
    pub(crate) design: &'a mut Design,
    pub(crate) file: FileId,
    pub(crate) owner: DeclId,
    /// Where `mod b;` looks for its file.
    pub(crate) provider: &'a dyn SourceProvider,
    /// The path of the file being lowered, and whether it is a root file.
    pub(crate) path: PathBuf,
    pub(crate) is_root: bool,
    /// The enclosing inline modules of the current file, outermost first.
    pub(crate) inline: Vec<String>,
    /// The canonical paths of the files being loaded, outermost first.
    pub(crate) chain: Vec<PathBuf>,
}
