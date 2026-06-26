use std::io::{self, Write};
use std::path::Path;
use std::fs;

pub fn execute(name: Option<String>) {
    let project_name = if let Some(n) = name {
        n
    } else {
        print!("Project name: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.trim().to_string()
    };

    if project_name.is_empty() {
        eprintln!("Error: Project name cannot be empty.");
        std::process::exit(1);
    }

    let path = Path::new(&project_name);
    
    // Check if Piperine.toml already exists
    if path.join("Piperine.toml").exists() || (path.components().count() == 0 && Path::new("Piperine.toml").exists()) {
        eprintln!("Error: Piperine.toml already exists.");
        std::process::exit(1);
    }

    // Create directories
    fs::create_dir_all(path.join("src")).unwrap();

    // Create Piperine.toml
    let toml = piperine_project::PiperineToml::new(&project_name);
    fs::write(path.join("Piperine.toml"), toml.to_string_pretty().unwrap()).unwrap();

    // Write template
    let main_vams = include_bytes!("../../templates/main.vams");
    fs::write(path.join("src/main.vams"), main_vams).unwrap();

    println!("Created piperine project `{}`", project_name);
}
