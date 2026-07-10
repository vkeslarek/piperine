use piperine_lang::SourceMap;
use piperine_project::{get_current_project_root, PiperineToml};
use std::path::{Path, PathBuf};

pub fn build_source_map() -> (SourceMap, PathBuf) {
    let project_root =
        get_current_project_root().unwrap_or_else(|| std::env::current_dir().unwrap());
    let source_map = piperine_project::project_source_map(&project_root);
    (source_map, project_root)
}

/// Stamp `Piperine.toml` metadata (name, version, dependency names) onto an
/// elaborated design's POM project node. A no-op outside a project.
pub fn stamp_project_meta(design: &mut piperine_lang::Design, project_root: &Path) {
    if let Ok(toml) = PiperineToml::load(&project_root.join("Piperine.toml")) {
        let deps = toml.dependencies.keys().cloned().collect();
        design.set_project_meta(toml.project.name, toml.project.version, deps);
    }
}
