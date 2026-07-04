//! Per-document state: parsed design, errors, and version tracking.

use std::collections::HashMap;
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
    /// Span in the source.
    pub span: Option<miette::SourceSpan>,
}

impl ServerState {
    pub fn new() -> Self {
        Self { documents: HashMap::new() }
    }

    /// Create a ServerState for testing (no connection needed).
    #[allow(dead_code)]
    pub fn dummy() -> Self {
        Self { documents: HashMap::new() }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the full lexer+parser+elaborator pipeline and collect all errors
/// with their byte positions extracted from error messages.
pub fn parse_and_collect_errors(source: &str) -> (Option<Design>, Vec<ParseError>) {
    let mut all_errors = Vec::new();
    let (source_file, parse_errors) = piperine_lang::parse::parse_str_tolerant(source);
    
    for e in parse_errors {
        all_errors.push(ParseError {
            message: e.to_string(),
            span: e.span(),
        });
    }

    let design = match source_file.elaborate(&piperine_lang::SourceMap::dummy()) {
        Ok(d) => Some(d),
        Err(e) => {
            all_errors.push(ParseError {
                message: e.to_string(),
                span: e.span,
            });
            None
        }
    };

    (design, all_errors)
}
