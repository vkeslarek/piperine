//! `piperine plugin list` and the plugin-script catch-all (SPEC Part VI §10).

use piperine_project::get_current_project_root;

fn project_root() -> std::path::PathBuf {
    get_current_project_root().unwrap_or_else(|| std::env::current_dir().unwrap())
}

pub fn list() {
    let root = project_root();
    let host = super::utils::load_plugin_host(&root);
    if host.is_empty() {
        println!("No plugins configured ([plugins] in Piperine.toml).");
        return;
    }
    for line in host.describe() {
        println!("{line}");
    }
}

/// Dispatch `piperine <name> [args...]` to a plugin-registered script.
/// An unknown name is P0009 — never a silent no-op.
pub fn script(mut args: Vec<String>) {
    if args.is_empty() {
        eprintln!("Error: empty external subcommand");
        std::process::exit(2);
    }
    let name = args.remove(0);
    let root = project_root();
    let host = super::utils::load_plugin_host(&root);
    match host.run_script(&name, &args) {
        Some(Ok(code)) => std::process::exit(code),
        Some(Err(e)) => {
            eprintln!("Plugin script error: {e}");
            std::process::exit(1);
        }
        None => {
            eprintln!("{}", piperine_plugin::PluginError::UnknownScript(name));
            std::process::exit(2);
        }
    }
}
