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
        .and_then(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            let word = crate::text_pos::word_at_position(&doc.source, pos)?;
            // Only report references for something that resolves to a symbol.
            doc.resolve_at(offset)?;

            Some(
                doc.word_occurrences(&word)
                    .into_iter()
                    .map(|(start, end)| Location {
                        uri: uri.clone(),
                        range: crate::text_pos::byte_range(&doc.source, start, end),
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();

    connection.respond(id, locations);
}
