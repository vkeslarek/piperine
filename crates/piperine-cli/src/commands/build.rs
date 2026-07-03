use std::path::{PathBuf, Path};
use std::fs;
use include_dir::{include_dir, Dir};

static HEADERS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../piperine-lang/headers");

pub fn execute(file: Option<String>) {
    let root = if let Some(r) = piperine_project::get_current_project_root() {
        r
    } else {
        eprintln!("Error: No Piperine.toml found. Please run this command inside a project.");
        std::process::exit(1);
    };

    let target_dir = root.join("target");
    let toolchain_headers = target_dir.join("toolchain").join("headers");

    if !toolchain_headers.exists() {
        println!("Setting up toolchain headers in target/toolchain/headers...");
        fs::create_dir_all(&toolchain_headers).unwrap();
        HEADERS_DIR.extract(&toolchain_headers).expect("Failed to extract headers to target directory");
    }

    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        root.join("src").join("main.phdl")
    };
    
    println!("Building design for: {}", path.display());
    // TODO: call compiler/elaborator
}

