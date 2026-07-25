use lsp_server::{Connection, Request};
use lsp_types::{Location, ReferenceParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<ReferenceParams>(connection) else { return };

    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;

    let locations = state
        .documents
        .get(&uri)
        .map(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            // occurrences_at (T8) already gates on symbol resolution and
            // returns only the binding's own recorded uses — no text scan,
            // so comments/strings/other-scope same-spelled identifiers
            // never appear (LSP-10).
            doc.occurrences_at(offset)
                .into_iter()
                .map(|(start, end)| Location {
                    uri: uri.clone(),
                    range: crate::text_pos::byte_range(&doc.source, start, end),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    connection.respond(id, locations);
}
