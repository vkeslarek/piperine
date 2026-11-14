//! The manifest, `Piperine.toml`: read into spanned TOML, then walked by hand so that every fault
//! is a coded diagnostic at the text it is about.

use std::path::Path;

use piperine_lang::msl::{Diagnostic, Span};
use toml::de::DeTable;

use crate::error::PackageError;

/// The file name of a manifest.
pub const MANIFEST: &str = "Piperine.toml";

/// A manifest as read: valid TOML. Its tables are read by the tasks that follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {}

impl Manifest {
    /// Read `dir/Piperine.toml`. A missing file, text that is not UTF-8 and text that is not
    /// TOML are each one `E0170`.
    pub fn load(dir: &Path) -> Result<Manifest, Vec<PackageError>> {
        let file = dir.join(MANIFEST);
        let fail = |text: String, diagnostic: Diagnostic| vec![PackageError { file: file.clone(), text, diagnostic }];
        let bytes = std::fs::read(&file).map_err(|e| {
            let message = format!("cannot read the manifest `{}`: {e}", file.display());
            fail(String::new(), Diagnostic::new("E0170", message, Span::new(0, 0), "a package holds a `Piperine.toml`"))
        })?;
        match String::from_utf8(bytes) {
            Ok(text) => Manifest::parse_at(&file, &text),
            Err(e) => {
                let at = e.utf8_error().valid_up_to();
                // The lossy text holds one U+FFFD (three bytes) for the offending sequence.
                let text = String::from_utf8_lossy(e.as_bytes()).into_owned();
                let message = format!("`{}` is not UTF-8 text", file.display());
                Err(fail(text, Diagnostic::new("E0170", message, Span::new(at, at + '\u{FFFD}'.len_utf8()), "not valid UTF-8")))
            }
        }
    }

    /// Read manifest text; its errors name the file `Piperine.toml`.
    pub fn parse(text: &str) -> Result<Manifest, Vec<PackageError>> {
        Manifest::parse_at(Path::new(MANIFEST), text)
    }

    fn parse_at(file: &Path, text: &str) -> Result<Manifest, Vec<PackageError>> {
        match DeTable::parse(text) {
            Ok(_) => Ok(Manifest {}),
            Err(e) => {
                let span = widen(text, e.span().map_or(0..0, |r| r));
                let diagnostic = Diagnostic::new("E0170", format!("the manifest is not valid TOML: {}", e.message()), span, "not valid TOML");
                Err(vec![PackageError { file: file.to_path_buf(), text: text.to_string(), diagnostic }])
            }
        }
    }
}

/// A span `toml` reports empty (the place something was expected) covers the next character, so
/// that a report can underline it; at the end of the text, the last one.
fn widen(text: &str, range: std::ops::Range<usize>) -> Span {
    if range.start != range.end {
        return Span::new(range.start, range.end);
    }
    let at = range.start.min(text.len());
    match text[at..].chars().next() {
        Some(c) => Span::new(at, at + c.len_utf8()),
        None => match text[..at].chars().next_back() {
            Some(c) => Span::new(at - c.len_utf8(), at),
            None => Span::new(0, 0),
        },
    }
}
