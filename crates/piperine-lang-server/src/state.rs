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
        if let Some(ref e) = errors.first() {
            eprintln!("parse error in {uri:?}: {}", e.message);
        }
        self.documents.insert(
            uri,
            DocumentState { source, version, design, errors },
        );
    }
}

/// Run the full lexer+parser+elaborator pipeline and collect all errors
/// with their byte positions extracted from error messages.
fn parse_and_collect_errors(source: &str) -> (Option<Design>, Vec<ParseError>) {
    // Phase 1: Lexer — gives byte positions for lexer errors.
    let mut lexer = piperine_lang::Lexer::new(source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            let offset = extract_byte_offset(&e);
            return (None, vec![ParseError { message: e, byte_offset: offset }]);
        }
    };

    // Phase 2: Parser — errors have the form "Expected X, found Y".
    let source_file = match piperine_lang::parse_str(source) {
        Ok(sf) => sf,
        Err(e) => {
            // Try to find the position from the error message or token position.
            let msg_offset = extract_byte_offset(&e);
            let tok_offset = if msg_offset.is_none() {
                if let Some(found) = e.rsplit("found ").next() {
                    let token_text = found
                        .trim()
                        .strip_prefix("Some(")
                        .and_then(|s| s.strip_suffix(')'))
                        .or_else(|| found.trim().strip_prefix('`').and_then(|s| s.strip_suffix('`')));
                    token_text.and_then(|tt| {
                        tokens
                            .iter()
                            .find(|tok| {
                                format!("{:?}", tok.tok).contains(tt)
                                    || format!("{:?}", tok.tok).to_lowercase() == tt.to_lowercase()
                            })
                            .map(|tok| tok.start)
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let offset = msg_offset.or(tok_offset).or_else(|| tokens.last().map(|t| t.end));
            return (None, vec![ParseError { message: e, byte_offset: offset }]);
        }
    };

    // Phase 3: Elaboration.
    match source_file.elaborate() {
        Ok(design) => (Some(design), vec![]),
        Err(elab_err) => {
            let msg = elab_err.to_string();
            (None, vec![ParseError { message: msg, byte_offset: None }])
        }
    }
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
