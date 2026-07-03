use piperine_project::{PiperineToml, get_current_project_root, resolver::Resolver};
use std::env;

pub fn execute() {
    let project_root = get_current_project_root().unwrap_or_else(|| env::current_dir().unwrap());
    let toml_path = project_root.join("Piperine.toml");

    if !toml_path.exists() {
        eprintln!("Error: Piperine.toml not found in the current directory.");
        std::process::exit(1);
    }

    let toml = match PiperineToml::load(&toml_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to parse Piperine.toml: {}", e);
            std::process::exit(1);
        }
    };

    println!("{} v{}", toml.project.name, toml.project.version);

    let mut resolver = Resolver::new(&project_root, false);

    match resolver.resolve(&toml) {
        Ok(map) => {
            if map.is_empty() {
                println!("No dependencies.");
            } else {
                for (name, path) in map {
                    println!("├── {} ({})", name, path.display());
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to resolve dependencies: {}", e);
            std::process::exit(1);
        }
    }
}
