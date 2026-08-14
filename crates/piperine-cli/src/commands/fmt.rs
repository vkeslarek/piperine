use std::fs;
use std::path::PathBuf;

pub fn execute(file: Option<String>) {
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.vams")
        } else {
            eprintln!(
                "Error: No Piperine.toml found in current or parent directories. Please provide a file."
            );
            std::process::exit(1);
        }
    };

    println!("Formatting file: {}", path.display());
    let input = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        std::process::exit(1);
    });

    // T37: `piperine_lang::Formatter::format` (T15) replaces the deleted
    // PHDL `Lexer`+`TokenFormatter` pipeline — see
    // `piperine-lang-server/src/handlers/formatting.rs`'s matching retarget.
    let formatted = piperine_lang::Formatter::format(&input).unwrap_or_else(|e| {
        eprintln!("Format error: {e}");
        std::process::exit(1);
    });
    fs::write(&path, formatted).unwrap_or_else(|e| {
        eprintln!("Error writing file: {}", e);
        std::process::exit(1);
    });
}
