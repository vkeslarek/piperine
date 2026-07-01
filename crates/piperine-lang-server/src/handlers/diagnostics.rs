//! Diagnostics: parse `.phdl` files on open/change and publish errors.
//!
//! On `textDocument/didOpen` and `textDocument/didChange`, the document
//! is re-parsed and elaborated. Any errors are published as LSP diagnostics
//! with their source positions when available.

use lsp_server::{Connection, Notification};
use lsp_types::notification::Notification as _;
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, NumberOrString, Position, PublishDiagnosticsParams, Range,
};
use serde_json::from_value;

use crate::state::{ParseError, ServerState};

/// Handle `textDocument/didOpen`: store the document, parse it, publish diagnostics.
pub fn handle_open(state: &mut ServerState, not: Notification, connection: &Connection) {
    let params: DidOpenTextDocumentParams =
        from_value(not.params).expect("invalid didOpen params");
    let uri = params.text_document.uri;
    let source = params.text_document.text;
    let version = params.text_document.version;

    state.upsert_document(uri.clone(), source, version);
    publish_diagnostics(state, &uri, connection);
}

/// Handle `textDocument/didChange`: update the document, re-parse, publish diagnostics.
pub fn handle_change(state: &mut ServerState, not: Notification, connection: &Connection) {
    let params: DidChangeTextDocumentParams =
        from_value(not.params).expect("invalid didChange params");
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    if let Some(change) = params.content_changes.into_iter().last() {
        state.upsert_document(uri.clone(), change.text, version);
        publish_diagnostics(state, &uri, connection);
    }
}

/// Handle `textDocument/didSave`: re-parse from disk (or use cached source).
pub fn handle_save(state: &mut ServerState, not: Notification, connection: &Connection) {
    let params: DidSaveTextDocumentParams =
        from_value(not.params).expect("invalid didSave params");
    let uri = params.text_document.uri;
    // If the file content was provided, use it; otherwise re-parse cached source.
    if let Some(text) = params.text {
        state.upsert_document(uri.clone(), text, 0);
    }
    publish_diagnostics(state, &uri, connection);
}

/// Handle `textDocument/didClose`: remove the document from state.
pub fn handle_close(state: &mut ServerState, not: Notification) {
    let params: lsp_types::DidCloseTextDocumentParams =
        from_value(not.params).expect("invalid didClose params");
    state.documents.remove(&params.text_document.uri);
}

/// Run parsing + elaboration on the document and publish any errors.
fn publish_diagnostics(state: &ServerState, uri: &lsp_types::Uri, connection: &Connection) {
    let doc = match state.documents.get(uri) {
        Some(d) => d,
        None => return,
    };

    let diagnostics: Vec<Diagnostic> = doc
        .errors
        .iter()
        .map(|e| parse_error_to_diagnostic(&doc.source, e))
        .collect();

    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics,
        version: Some(doc.version),
    };

    let not = Notification {
        method: lsp_types::notification::PublishDiagnostics::METHOD.into(),
        params: serde_json::to_value(params).unwrap(),
    };
    connection
        .sender
        .send(lsp_server::Message::Notification(not))
        .unwrap();
}

/// Convert a ParseError into an LSP Diagnostic with position if available.
fn parse_error_to_diagnostic(source: &str, error: &ParseError) -> Diagnostic {
    let range = error
        .byte_offset
        .map(|offset| {
            let (line, col) = byte_to_line_col(source, offset);
            Range {
                start: Position { line, character: col },
                end: Position { line, character: col + 1 },
            }
        })
        .unwrap_or_else(|| Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 1 },
        });

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("parse-error".into())),
        source: Some("piperine".into()),
        message: error.message.clone(),
        ..Default::default()
    }
}

/// Convert a byte offset to a (line, column) pair.
pub fn byte_to_line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let offset = byte_offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() as u32;
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = (offset - last_newline) as u32;
    (line, col)
}

/// Extract a byte range from an error message (for tests).
pub fn extract_error_range(source: &str, error: &str) -> Range {
    let pe = ParseError {
        message: error.into(),
        byte_offset: {
            error
                .split("at byte ")
                .last()
                .and_then(|s| {
                    s.split(|c: char| !c.is_ascii_digit())
                        .next()
                        .and_then(|n| n.parse::<usize>().ok())
                })
        },
    };
    parse_error_to_diagnostic(source, &pe).range
}
