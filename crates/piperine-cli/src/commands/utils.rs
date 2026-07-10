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

/// Load the project's plugin host (SPEC Part VI §5). Trust mode comes from
/// `PIPERINE_PLUGIN_TRUST` (`accept` | `reject`), defaulting to the
/// interactive TOFU prompt. A project without `[plugins]` yields an inert
/// host. Load failures are fatal — a requested plugin that cannot load must
/// never silently degrade the run.
pub fn load_plugin_host(project_root: &Path) -> std::rc::Rc<piperine_plugin::PluginHost> {
    use piperine_plugin::TrustMode;
    let mode = match std::env::var("PIPERINE_PLUGIN_TRUST").as_deref() {
        Ok("accept") => TrustMode::AcceptAll,
        Ok("reject") => TrustMode::RejectUntrusted,
        _ => TrustMode::Interactive,
    };
    match piperine_plugin::PluginHost::load_for_project(project_root, mode) {
        Ok(host) => std::rc::Rc::new(host),
        Err(e) => {
            eprintln!("Plugin error: {e}");
            std::process::exit(1);
        }
    }
}
