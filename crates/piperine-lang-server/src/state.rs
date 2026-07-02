//! Per-document state: parsed design, errors, and version tracking.

use std::collections::HashMap;

use lsp_server::Connection;
use lsp_types::Uri;

use piperine_lang::Design;

/// Holds the current state of each open document.
pub struct ServerState {
    /// Parsed designs keyed by document URI.
    pub documents: HashMap<Uri, DocumentState>,
}

pub struct DocumentState {
    /// The raw source text of the document.
    pub source: String,
    /// Document version number (from didChange notifications).
    pub version: i32,
    /// The elaborated design, if parsing succeeded.
    pub design: Option<Design>,
    /// Parse/elaboration error messages if any.
    pub errors: Vec<ParseError>,
}

/// A parse or elaboration error with optional source position.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    /// Byte offset into the source, if known.
    pub byte_offset: Option<usize>,
}

impl ServerState {
    pub fn new(_connection: &Connection) -> Self {
        Self { documents: HashMap::new() }
    }

    /// Create a ServerState for testing (no connection needed).
    #[allow(dead_code)]
    pub fn dummy() -> Self {
        Self { documents: HashMap::new() }
    }

    /// Store or update a document's source text, parse it, and collect errors.
    pub fn upsert_document(&mut self, uri: Uri, source: String, version: i32) {
        let (design, errors) = parse_and_collect_errors(&source);
        // Mute eprintln to prevent log spam during typing
        self.documents.insert(
            uri,
            DocumentState { source, version, design, errors },
        );
    }
}

/// Run the full lexer+parser+elaborator pipeline and collect all errors
/// with their byte positions extracted from error messages.
fn parse_and_collect_errors(source: &str) -> (Option<Design>, Vec<ParseError>) {
    let mut all_errors = Vec::new();
    let (source_file, parse_errors) = piperine_lang::parse::parse_str_tolerant(source);
    
    for e in parse_errors {
        all_errors.push(ParseError {
            message: e.to_string(),
            byte_offset: e.byte_offset(),
        });
    }

    let design = match source_file.elaborate() {
        Ok(d) => Some(d),
        Err(e) => {
            all_errors.push(ParseError {
                message: e.to_string(),
                byte_offset: None,
            });
            None
        }
    };

    (design, all_errors)
}

/// Extract a byte offset from a lexer error message ("... at byte N").
fn extract_byte_offset(error: &str) -> Option<usize> {
    error
        .split("at byte ")
        .last()
        .and_then(|s| {
            s.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse::<usize>().ok())
        })
}
