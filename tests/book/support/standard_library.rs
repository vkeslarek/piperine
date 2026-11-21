//! `StandardLibrary`: the text of the standard library's own files, which a chapter says it shows,
//! normalized so a declaration compares with the Book's however the file lays it out (the formatter's
//! trailing commas included).

use std::path::Path;

/// The standard library's source files, `crates/piperine-lang/std`.
pub struct StandardLibrary;

impl StandardLibrary {
    /// The text of the files `files` of `std`, joined and normalized (see [`StandardLibrary::normalize`]).
    pub fn normalized(files: &[&str]) -> String {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/piperine-lang/std");
        let text: String = files.iter().map(|file| std::fs::read_to_string(directory.join(file)).unwrap_or_else(|error| panic!("{file}: {error}"))).collect();
        Self::normalize(&text)
    }

    /// `text` without its line comments, its `pub` keywords and the comma before a closing brace (the
    /// formatter writes one after the last field, the Book may not), whitespace collapsed to single blanks.
    pub fn normalize(text: &str) -> String {
        let without_comments: Vec<&str> = text.lines().map(|line| line.split("//").next().unwrap_or("")).collect();
        let joined = without_comments.join(" ").replace("pub ", "").split_whitespace().collect::<Vec<_>>().join(" ");
        joined.replace(", }", " }")
    }
}
