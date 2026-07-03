use std::fs;

pub fn execute() {
    if let Some(root) = piperine_project::get_current_project_root() {
        let target_dir = root.join("target");
        if target_dir.exists() {
            println!("Cleaning target directory...");
            fs::remove_dir_all(target_dir)
                .unwrap_or_else(|e| eprintln!("Failed to clean target: {}", e));
        } else {
            println!("Target directory already clean.");
        }
    } else {
        eprintln!("Error: No Piperine.toml found.");
    }
}
