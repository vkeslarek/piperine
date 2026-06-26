use std::path::PathBuf;

pub fn execute(file: Option<String>) {
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.vams")
        } else {
            eprintln!("Error: No Piperine.toml found. Please provide a file.");
            std::process::exit(1);
        }
    };
    println!("Building design for: {}", path.display());
    // TODO: call compiler/elaborator
}
