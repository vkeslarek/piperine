//! Project discovery for open documents: locate the enclosing
//! `Piperine.toml` and build the same `SourceMap` the CLI builds, so the
//! editor and `piperine build` agree on multi-file resolution.

use piperine_lang::SourceMap;
use piperine_project::{PiperineToml, resolver::Resolver};
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

    /// Build the `SourceMap` for this project. Without a project root the
    /// map is the single-file dummy the elaborator accepts for standalone
    /// documents.
    pub fn source_map(&self) -> SourceMap {
        match &self.root {
            Some(root) => Self::source_map_at(root),
            None => SourceMap::dummy(),
        }
    }

    fn source_map_at(project_root: &Path) -> SourceMap {
        let src_dir = project_root.join("src");
        let map_root = if src_dir.exists() { src_dir } else { project_root.to_path_buf() };
        let mut source_map = SourceMap::new(map_root);

        // Resolve project dependencies declared in Piperine.toml.
        let toml_path = project_root.join("Piperine.toml");
        if let Ok(toml) = PiperineToml::load(&toml_path) {
            // Register the project's own package name so it can refer to its
            // own modules by name (e.g. `use spice::constants;`).
            source_map.add_namespace(&toml.project.name, source_map.root_path.clone());

            let mut resolver = Resolver::new(project_root, false);
            match resolver.resolve(&toml) {
                Ok(resolved_deps) => {
                    for (name, path) in resolved_deps {
                        let dep_src = path.join("src");
                        if dep_src.exists() {
                            source_map.add_namespace(&name, dep_src);
                        } else {
                            source_map.add_namespace(&name, path);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to resolve dependencies: {}", e);
                }
            }
        }

        // Prelude headers: prefer the project's own `headers/`, falling
        // back to the repo checkout this server was built from (dev builds).
        let mut headers_dir = project_root.join("headers");
        if !headers_dir.exists() {
            headers_dir =
                PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
        }

        if headers_dir.exists() {
            source_map = source_map.with_prelude(headers_dir.join("prelude.phdl"));
            source_map.add_namespace("piperine", headers_dir.clone());
        }

        source_map
    }
}
