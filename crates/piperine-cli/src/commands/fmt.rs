use std::fs;
use std::path::{Path, PathBuf};
use piperine_lang::parse::format::{TokenFormatter, FormatOptions};
use piperine_lang::parse::lexer::Lexer;

pub fn execute(file: Option<String>) {
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.phdl")
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
    
    let mut lexer = Lexer::new(&input);
    let raw_tokens = lexer.tokenize_all().unwrap_or_else(|e| {
        eprintln!("Lexer error: {}", e);
        std::process::exit(1);
    });

    let formatted = TokenFormatter::format_source(&input, &raw_tokens, FormatOptions::default());
    println!("{}", formatted);
}
