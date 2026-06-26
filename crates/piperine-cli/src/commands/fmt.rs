use std::path::Path;
use piperine_parser::{lexer::tokenize_with_comments, fmt::{format_source, FormatOptions}, parser::bundled_header_dir};

pub fn execute(file: String) {
    let path = Path::new(&file);
    let input = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        std::process::exit(1);
    });

    let mut dirs = Vec::new();
    if let Some(dir) = path.parent() {
        dirs.push(dir.to_path_buf());
    }
    dirs.push(bundled_header_dir());

    let raw_tokens = tokenize_with_comments(&input).unwrap_or_else(|e| {
        eprintln!("Lexer error: {}", e);
        std::process::exit(1);
    });

    let formatted = format_source(&input, &raw_tokens, FormatOptions::default());
    println!("{}", formatted);
}
