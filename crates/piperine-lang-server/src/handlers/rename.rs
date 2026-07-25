use lsp_server::{Connection, Request};
use lsp_types::{
    PrepareRenameResponse, RenameParams, TextDocumentPositionParams, TextEdit, WorkspaceEdit,
};
use std::collections::HashMap;

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

#[allow(clippy::mutable_key_type)]
pub fn handle_rename(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<RenameParams>(connection) else { return };

    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;

    if !is_valid_identifier(&new_name) {
        connection.respond_invalid(id, format!("`{new_name}` is not a valid PHDL identifier"));
        return;
    }

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);
        // occurrences_at (T8) already gates on symbol resolution and
        // returns only the binding's own recorded uses — a same-named
        // identifier in an unrelated scope is never in this set (LSP-11).
        let occurrences = doc.occurrences_at(offset);
        if occurrences.is_empty() {
            return None;
        }

        let edits = occurrences
            .into_iter()
            .map(|(start, end)| TextEdit {
                range: crate::text_pos::byte_range(&doc.source, start, end),
                new_text: new_name.clone(),
            })
            .collect();

        let mut changes = HashMap::new();
        changes.insert(uri.clone(), edits);

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    });

    connection.respond(id, result);
}

pub fn handle_prepare_rename(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<TextDocumentPositionParams>(connection) else { return };

    let uri = params.text_document.uri;
    let pos = params.position;

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);
        // A keyword/literal/comment never resolves to a symbol, so
        // occurrences_at is empty and prepare-rename correctly declines
        // (LSP-11 edge case: decline on keyword/literal).
        doc.occurrences_at(offset)
            .into_iter()
            .find(|&(start, end)| offset >= start && offset <= end)
            .map(|(start, end)| {
                PrepareRenameResponse::Range(crate::text_pos::byte_range(&doc.source, start, end))
            })
    });

    connection.respond(id, result);
}

/// PHDL identifier shape: ASCII letter or `_` first, ASCII alphanumerics
/// and `_` after. Mirrors the lexer's ident rule.
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
