use piperine_cli::commands::check::check_file;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_all_examples_compile() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf();
    
    let examples_dir = workspace_root.join("examples");
    
    let mut phdl_files = Vec::new();
    if examples_dir.exists() {
        for entry in fs::read_dir(examples_dir).expect("Failed to read examples directory") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "phdl") {
                phdl_files.push(path);
            }
        }
    } else {
        panic!("examples/ directory does not exist at {:?}", examples_dir);
    }
    
    assert!(!phdl_files.is_empty(), "No .phdl files found in examples directory");
    
    phdl_files.sort();
    
    let mut failures = Vec::new();
    for file in phdl_files {
        println!("Testing {:?}", file);
        if let Err(e) = check_file(&file) {
            eprintln!("Failed to check {:?}: {}", file, e);
            failures.push((file, e));
        }
    }
    
    if !failures.is_empty() {
        panic!("Some examples failed to compile: {:?}", failures);
    }
}
