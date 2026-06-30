use std::path::{Path, PathBuf};
use std::fs;
use piperine_ams::{lexer::Lexer, fmt::{TokenFormatter, FormatOptions}, Document};

pub fn execute(file: Option<String>) {
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.vams")
        } else {
            eprintln!("Error: No Piperine.toml found in current or parent directories. Please provide a file.");
            std::process::exit(1);
        }
    };

    println!("Formatting file: {}", path.display());
    let input = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        std::process::exit(1);
    });

    let mut dirs = Vec::new();
    if let Some(dir) = path.parent() {
        dirs.push(dir.to_path_buf());
    }
    dirs.push(Document::bundled_header_dir());

    let raw_tokens = Lexer::tokenize_with_comments(&input).unwrap_or_else(|e| {
        eprintln!("Lexer error: {}", e);
        std::process::exit(1);
    });

    let formatted = TokenFormatter::format_source(&input, &raw_tokens, FormatOptions::default());
    println!("{}", formatted);
}
