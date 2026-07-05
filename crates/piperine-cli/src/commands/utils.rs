use piperine_lang::SourceMap;
use piperine_project::{PiperineToml, get_current_project_root, resolver::Resolver};
use std::path::PathBuf;

pub fn build_source_map() -> (SourceMap, PathBuf) {
    let project_root =
        get_current_project_root().unwrap_or_else(|| std::env::current_dir().unwrap());

    let src_dir = project_root.join("src");
    let map_root = if src_dir.exists() {
        src_dir.clone()
    } else {
        project_root.clone()
    };
    let mut source_map = SourceMap::new(map_root);

    // Resolve project dependencies
    let toml_path = project_root.join("Piperine.toml");
    if let Ok(toml) = PiperineToml::load(&toml_path) {
        // Register the project's own package name so a package can refer to its
        // own modules by name (e.g. `use spice::constants;` inside the `spice`
        // package). Without this a standalone library cannot self-reference.
        source_map.add_namespace(&toml.project.name, source_map.root_path.clone());

        let mut resolver = Resolver::new(&project_root, false);
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

    // The `piperine` stdlib prelude ships with the compiler.
    let mut headers_dir = project_root.join("headers");
    if !headers_dir.exists() {
        headers_dir =
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    }

    if headers_dir.exists() {
        source_map = source_map.with_prelude(headers_dir.join("prelude.phdl"));
        source_map.add_namespace("piperine", headers_dir.clone());
    }

    (source_map, project_root)
}
