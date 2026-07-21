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
        // Builtin `spice` stdlib namespace — only if no project package or
        // dependency already claimed the name (project packages win).
        if !source_map.namespaces.contains_key("spice") {
            source_map.add_namespace("spice", headers_dir.join("spice"));
        }
    }

    source_map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique scratch project directory (removed on drop).
    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("piperine-source-map-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// SPICE-01: with no `Piperine.toml` at all, the builtin `spice`
    /// namespace resolves `use spice::diode;` from `headers/spice/`.
    #[test]
    fn builtin_spice_namespace_without_piperine_toml() {
        let scratch = ScratchDir::new("builtin");
        let map = project_source_map(&scratch.0);

        let spice_dir = map.namespaces.get("spice").expect("builtin `spice` namespace registered");
        assert!(spice_dir.join("diode.phdl").exists(), "headers/spice/diode.phdl reachable at {spice_dir:?}");

        let src = "
            use piperine::disciplines;
            use spice::sources;
            use spice::passives;
            use spice::diode;
            mod Top() {
                wire gnd: Electrical; wire vin: Electrical; wire out: Electrical;
                v1: vsrc (.p=vin,.n=gnd) { .dc = 5.0 };
                r1: res  (.p=vin,.n=out) { .r = 1.0e3 };
                d1: dio  (.p=out,.n=gnd) { };
            }
        ";
        let design = piperine_lang::parse_and_elaborate(src, &map)
            .expect("use spice::diode; must elaborate through the builtin path");
        assert!(design.module("Top").is_some());
    }

    /// SPICE-04: a project whose `Piperine.toml` names it `spice` shadows
    /// the builtin — `use spice::…` resolves to the project's own `src/`.
    #[test]
    fn project_named_spice_shadows_builtin() {
        let scratch = ScratchDir::new("shadow");
        std::fs::write(
            scratch.0.join("Piperine.toml"),
            "[project]\nname = \"spice\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
        )
        .unwrap();
        let src_dir = scratch.0.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("diode.phdl"), "// project-local diode\n").unwrap();

        let map = project_source_map(&scratch.0);
        let spice_dir = map.namespaces.get("spice").expect("`spice` namespace registered");
        assert_eq!(spice_dir, &src_dir, "project `spice` package must win over the builtin headers");
    }
}
