//! MD-63 item 1 for `piperine-numbers`: its source files are embedded verbatim into generated
//! code, so they may use no crate but `core`/`std`/`super`, no `unsafe`, no macro definition, no
//! panic and no module-level function (MD-13 rule 2). The oracle is the text of each file.

use std::fs;
use std::path::PathBuf;

/// Reads the crate's own source files and reports every line that breaks an embedding rule.
struct PurityScan {
    /// Source files as (file name, lines).
    files: Vec<(String, Vec<String>)>,
}

impl PurityScan {
    /// Loads every `src/*.rs` of the crate.
    fn of_crate() -> Self {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        for entry in fs::read_dir(&directory).expect("src directory is readable") {
            let path = entry.expect("directory entry").path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                let text = fs::read_to_string(&path).expect("source file is readable");
                let name = path.file_name().expect("file name").to_string_lossy().to_string();
                files.push((name, text.lines().map(str::to_string).collect()));
            }
        }
        PurityScan { files }
    }

    /// The code lines of each file: comments (`//`, `///`, `//!`) are not code.
    fn code_lines(&self) -> Vec<(String, usize, String)> {
        let mut lines = Vec::new();
        for (name, text) in &self.files {
            for (index, line) in text.iter().enumerate() {
                if !line.trim_start().starts_with("//") {
                    lines.push((name.clone(), index + 1, line.clone()));
                }
            }
        }
        lines
    }

    /// Whether a `use` line names a crate other than `core`, `std`, `super` or `self`.
    fn foreign_use(line: &str) -> bool {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("use ").or_else(|| trimmed.strip_prefix("pub use ")) else {
            return false;
        };
        let root = rest.split("::").next().unwrap_or("").trim();
        !matches!(root, "core" | "std" | "super" | "self" | "alloc")
    }

    /// Whether a line declares a function at module level (no indentation).
    fn module_level_function(line: &str) -> bool {
        ["fn ", "pub fn ", "pub(crate) fn ", "const fn ", "pub const fn "].iter().any(|start| line.starts_with(start))
    }

    /// Every violation, as `file:line: text`.
    fn violations(&self) -> Vec<String> {
        let forbidden = ["unsafe ", "unsafe{", "macro_rules!", "panic!", ".unwrap()", ".expect(", "unreachable!", "todo!", "unimplemented!"];
        let mut found = Vec::new();
        for (name, number, line) in self.code_lines() {
            let broken = Self::foreign_use(&line) || Self::module_level_function(&line) || forbidden.iter().any(|word| line.contains(word));
            if broken {
                found.push(format!("{name}:{number}: {}", line.trim()));
            }
        }
        found
    }
}

#[test]
fn the_numeric_crate_is_embeddable_source() {
    let scan = PurityScan::of_crate();
    assert!(!scan.files.is_empty(), "lib.rs is a source file");
    let violations = scan.violations();
    assert!(violations.is_empty(), "MD-63 item 1 broken:\n{}", violations.join("\n"));
}
