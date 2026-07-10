//! Project discovery for open documents: locate the enclosing
//! `Piperine.toml` and build the same `SourceMap` the CLI builds, so the
//! editor and `piperine build` agree on multi-file resolution.

use piperine_lang::SourceMap;
use std::path::PathBuf;

/// The project context a document belongs to: its root directory, when one
/// could be discovered by walking up from the document to `Piperine.toml`.
pub struct ProjectContext {
    root: Option<PathBuf>,
}

impl ProjectContext {
    /// Discover the project enclosing `uri` (a `file:` URI).
    pub fn discover(uri: &lsp_types::Uri) -> Self {
        let root = url::Url::parse(uri.as_str())
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .and_then(|p| piperine_project::find_project_root(&p));
        Self { root }
    }

    /// Build the `SourceMap` for this project. Without a project root the
    /// map is the single-file dummy the elaborator accepts for standalone
    /// documents.
    pub fn source_map(&self) -> SourceMap {
        match &self.root {
            Some(root) => piperine_project::project_source_map(root),
            None => SourceMap::dummy(),
        }
    }
}
