//! Diagnostics: publish errors and provide logging.

use lsp_server::{Connection, Notification};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, PublishDiagnosticsParams, Range,
    LogMessageParams, MessageType,
};
use lsp_types::notification::Notification as _;

use crate::state::{ParseError, ServerState};
use crate::text_pos;

pub fn log_message(connection: &Connection, typ: MessageType, message: String) {
    let params = LogMessageParams { typ, message };
    let not = Notification {
        method: lsp_types::notification::LogMessage::METHOD.into(),
        params: serde_json::to_value(params).unwrap(),
    };
    let _ = connection.sender.send(lsp_server::Message::Notification(not));
}

/// Run parsing + elaboration on the document and publish any errors.
pub fn publish_diagnostics(state: &ServerState, uri: &lsp_types::Uri, connection: &Connection) {
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
        params: serde_json::to_value(params).ok().unwrap_or(serde_json::Value::Null),
    };
    if connection
        .sender
        .send(lsp_server::Message::Notification(not))
        .is_err()
    {
        // Receiver gone — server is shutting down.
    }
}

/// Convert a ParseError into an LSP Diagnostic with position if available.
fn parse_error_to_diagnostic(source: &str, error: &ParseError) -> Diagnostic {
    let range = error.span.map_or(
        Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 1 },
        },
        |span: miette::SourceSpan| {
            text_pos::byte_range(source, span.offset(), span.offset() + span.len())
        },
    );

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("parse-error".into())),
        source: Some("piperine".into()),
        message: error.message.clone(),
        ..Default::default()
    }
}

/// Extract a byte range from an error message. Test-support surface: the
/// integration tests are the only consumers.
#[allow(dead_code)]
pub fn extract_error_range(source: &str, error: &str) -> Range {
    let pe = ParseError {
        message: error.into(),
        span: error
            .split("at byte ")
            .last()
            .and_then(|s| {
                s.split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|n| n.parse::<usize>().ok())
            })
            .map(|offset| miette::SourceSpan::new(offset.into(), 1)),
    };
    parse_error_to_diagnostic(source, &pe).range
}
