use std::path::PathBuf;

pub fn execute(file: Option<String>) {
    crate::commands::build::execute(file.clone());
    
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.phdl")
        } else {
            eprintln!("Error: No Piperine.toml found. Please provide a file.");
            std::process::exit(1);
        }
    };
    println!("Running simulation for: {}", path.display());
    // TODO: call simulator
}
