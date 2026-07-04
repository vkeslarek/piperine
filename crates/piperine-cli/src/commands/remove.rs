use piperine_project::{PiperineToml, get_current_project_root, resolver::Resolver};
use std::env;
use std::fs;
use toml_edit::DocumentMut;

pub fn execute(name: String) {
    let project_root = get_current_project_root().unwrap_or_else(|| env::current_dir().unwrap());
    let toml_path = project_root.join("Piperine.toml");

    if !toml_path.exists() {
        eprintln!("Error: Piperine.toml not found in the current directory.");
        std::process::exit(1);
    }

    let toml_content = fs::read_to_string(&toml_path).expect("Failed to read Piperine.toml");
    let mut doc = toml_content
        .parse::<DocumentMut>()
        .expect("Failed to parse Piperine.toml");

    let mut removed = false;
    if let Some(deps) = doc.get_mut("dependencies") {
        if let Some(deps_table) = deps.as_table_mut() {
            if deps_table.remove(&name).is_some() {
                removed = true;
            }
        }
    }

    if !removed {
        eprintln!("Error: Dependency '{}' not found in Piperine.toml.", name);
        std::process::exit(1);
    }

    fs::write(&toml_path, doc.to_string()).expect("Failed to write Piperine.toml");
    println!("Removed '{}' from Piperine.toml.", name);

    let piperine_toml =
        PiperineToml::load(&toml_path).expect("Failed to parse updated Piperine.toml");
    let mut resolver = Resolver::new(&project_root, false);

    match resolver.resolve(&piperine_toml) {
        Ok(resolved) => {
            let deps_dir = project_root.join("target").join("deps");
            let dep_path = deps_dir.join(&name);
            // Optionally, check if it's still a transitive dependency
            if !resolved.contains_key(&name) {
                if dep_path.exists() {
                    if let Err(e) = fs::remove_dir_all(&dep_path) {
                        eprintln!(
                            "Warning: Failed to clean up cached dependency {}: {}",
                            name, e
                        );
                    } else {
                        println!("Cleaned up cached dependency '{}'.", name);
                    }
                }
            } else {
                println!(
                    "Note: '{}' is still retained in cache as it is a transitive dependency.",
                    name
                );
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to resolve tree after removal: {}", e);
        }
    }
}
