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

    // Keep legacy headers mapped for now
    let mut headers_dir = project_root.join("headers");
    if !headers_dir.exists() {
        // Fallback for running locally in the git repo
        if let Ok(repo_root) = std::env::current_dir() {
            let possible_headers = repo_root.join("../../../../crates/piperine-lang/headers");
            if possible_headers.exists() {
                headers_dir = possible_headers;
            } else {
                headers_dir = std::path::PathBuf::from(
                    "/home/keslarek/Git/piperine/crates/piperine-lang/headers",
                );
            }
        }
    }

    if headers_dir.exists() {
        source_map = source_map.with_prelude(headers_dir.join("prelude.phdl"));
        source_map.add_namespace("piperine", headers_dir.clone());
        // spice is now a real dependency, but just in case we need fallback
        let spice_fallback = headers_dir.join("ngspice");
        if spice_fallback.exists() {
            source_map.add_namespace("spice", spice_fallback);
        }
    }

    (source_map, project_root)
}
