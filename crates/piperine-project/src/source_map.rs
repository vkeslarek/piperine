//! The one project → `SourceMap` recipe, shared by the CLI and the language
//! server so `piperine build` and the editor agree on multi-file resolution:
//! `src/` (or the root) as the map root, the project's own package name
//! registered for self-reference, resolved dependencies as namespaces, and
//! the `piperine` stdlib prelude from `headers/`.

use std::path::{Path, PathBuf};

use piperine_lang::SourceMap;

use crate::resolver::Resolver;
use crate::PiperineToml;

/// Build the `SourceMap` for the project rooted at `project_root`.
pub fn project_source_map(project_root: &Path) -> SourceMap {
    let src_dir = project_root.join("src");
    let map_root = if src_dir.exists() { src_dir } else { project_root.to_path_buf() };
    let mut source_map = SourceMap::new(map_root);

    // Resolve project dependencies declared in Piperine.toml.
    let toml_path = project_root.join("Piperine.toml");
    if let Ok(toml) = PiperineToml::load(&toml_path) {
        // Register the project's own package name so a package can refer to
        // its own modules by name (e.g. `use spice::constants;` inside the
        // `spice` package). Without this a standalone library cannot
        // self-reference.
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

    // Prelude headers: prefer the project's own `headers/`, falling back to
    // the repo checkout this binary was built from (dev builds).
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
