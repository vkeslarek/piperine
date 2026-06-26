use std::path::{Path, PathBuf};

pub fn execute(file: Option<String>) {
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            let toml_path = root.join("Piperine.toml");
            match piperine_project::PiperineToml::load(&toml_path) {
                Ok(toml) => {
                    println!("Loaded project: {} v{}", toml.project.name, toml.project.version);
                    // For now, default to src/main.vams
                    root.join("src").join("main.vams")
                }
                Err(e) => {
                    eprintln!("Error loading Piperine.toml: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Error: No Piperine.toml found in current or parent directories. Please provide a file.");
            std::process::exit(1);
        }
    };

    println!("Checking file: {}", path.display());
    
    match piperine_parser::parse_file(&path) {
        Ok(doc) => {
            println!("Parse successful! Captured models:");
            println!("  Modules: {}", doc.modules.len());
            for m in &doc.modules {
                println!("    - {} (ports: {}, parameters: {}, variables: {}, analog_blocks: {}, instances: {}, tasks: {}, continuous_assigns: {})", 
                    m.name, m.ports.len(), m.parameters.len(), m.variables.len(), m.analog_blocks.len(), m.instances.len(), m.tasks.len(), m.continuous_assigns.len());
            }
            println!("  Disciplines: {}", doc.disciplines.len());
            println!("  Natures: {}", doc.natures.len());
            println!("  Paramsets: {}", doc.paramsets.len());
            println!("  Connectrules: {}", doc.connectrules.len());
            println!("  Configs: {}", doc.configs.len());
            println!("  Primitives: {}", doc.primitives.len());
        }
        Err(e) => {
            eprintln!("Parse failed: {}", e);
            std::process::exit(1);
        }
    }
}
