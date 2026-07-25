//! Project discovery for open documents: locate the enclosing
//! `Piperine.toml` and build the same `SourceMap` the CLI builds, so the
//! editor and `piperine build` agree on multi-file resolution.
//!
//! [`ProjectUnit`] (T12/LSP-14) is the multi-file counterpart of a single
//! `DocumentState`: one elaborated `Design` per project source file plus
//! one [`ResolutionIndex`] spanning all of them, keyed by project root in
//! `ServerState.projects`. It is the foundation cross-file goto (T13),
//! cross-file rename (T14), and per-file diagnostic fan-out (T15) build on
//! — this task only lays the shape down.

use piperine_lang::{ResolutionIndex, SourceMap};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

    /// The discovered project root, if any (`None` for a standalone
    /// document outside any `Piperine.toml` — the single-file fallback,
    /// LSP-17).
    pub fn root(&self) -> Option<&PathBuf> {
        self.root.as_ref()
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

/// A project-wide multi-file index (T12/LSP-14): every `.phdl` file under
/// the project's `src/` directory, elaborated against the shared project
/// `SourceMap`, keyed by its path, plus one [`ResolutionIndex`] merging all
/// of their bindings with [`BindingInfo::file`][piperine_lang::BindingInfo]
/// stamped to the owning file's path.
///
/// **SPEC_DEVIATION:** design.md frames this as "the multi-file `Design`"
/// (singular). `piperine-lang` has no cross-file `Design`-merge primitive —
/// `Design` is the output of one elaboration unit (mirrors how
/// `piperine-cli check`'s own `execute()` elaborates every project file
/// independently, `crates/piperine-cli/src/commands/check.rs`). So
/// `ProjectUnit` holds one `Design` per file instead of a single merged
/// one; the actual LSP-14 payload — one binding-identity index spanning
/// every file — is delivered in full via `index`.
///
/// A file that fails to elaborate has no entry in `designs`; its
/// parse/elaboration error(s) are captured in `errors` instead — the
/// source per-file diagnostic fan-out (T15/LSP-16) publishes from, so
/// every project file's own errors surface against its own URI even when
/// only a *different* file changed.
pub struct ProjectUnit {
    pub root: PathBuf,
    pub designs: HashMap<PathBuf, piperine_lang::Design>,
    pub index: ResolutionIndex,
    /// Every discovered `.phdl` file's parse/elaboration errors — empty for
    /// a file that elaborated cleanly. Keyed by the same paths as
    /// `designs`'s keys would be for a failing file (a path is in exactly
    /// one of `designs` or `errors` with a non-empty vec, never both).
    pub errors: HashMap<PathBuf, Vec<crate::state::ParseError>>,
}

impl ProjectUnit {
    /// Elaborate every `.phdl` file under `root/src/` against `source_map`
    /// and merge their `ResolutionIndex`es into one project-wide index.
    pub fn build(root: &Path, source_map: &SourceMap) -> Self {
        let mut designs = HashMap::new();
        let mut index = ResolutionIndex::default();
        let mut errors = HashMap::new();

        for path in discover_phdl_files(root) {
            let Ok(body) = std::fs::read_to_string(&path) else { continue };
            // Mirrors `DocumentState::analyze`'s own pipeline (parse
            // errors, then elaboration) so a project file's errors surface
            // through the same `ParseError` shape a directly-opened
            // document's do.
            let (source_file, parse_errors) = piperine_lang::parse::parse_str_tolerant(&body);
            let mut file_errors: Vec<crate::state::ParseError> = parse_errors
                .into_iter()
                .map(|e| {
                    let code = miette::Diagnostic::code(&e).map(|c| c.to_string());
                    crate::state::ParseError { message: e.to_string(), span: e.span(), code }
                })
                .collect();

            match source_file.elaborate_with_index(source_map) {
                Ok((design, mut file_index)) => {
                    file_index.set_file(path.display().to_string());
                    index.merge(file_index);
                    designs.insert(path.clone(), design);
                }
                Err(e) => {
                    // `ElabError::kind` carries the `#[diagnostic(code(...))]`,
                    // not `ElabError` itself — see state.rs's matching comment.
                    let code = miette::Diagnostic::code(&e.kind).map(|c| c.to_string());
                    file_errors.push(crate::state::ParseError { message: e.to_string(), span: e.span, code });
                }
            }

            if !file_errors.is_empty() {
                errors.insert(path, file_errors);
            }
        }

        Self { root: root.to_path_buf(), designs, index, errors }
    }
}

/// Every `.phdl` file under `root/src/` (recursive) — mirrors
/// `piperine-cli check`'s own project-file discovery
/// (`crates/piperine-cli/src/commands/check.rs::execute`).
fn discover_phdl_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let src_dir = root.join("src");
    if src_dir.exists() {
        let mut stack = vec![src_dir];
        while let Some(dir) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("phdl") {
                        paths.push(p);
                    }
                }
            }
        }
    }
    paths
}
