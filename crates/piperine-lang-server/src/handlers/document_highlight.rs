use lsp_server::{Connection, Request};
use lsp_types::{DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<DocumentHighlightParams>(connection) else { return };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let highlights = state
        .documents
        .get(&uri)
        .and_then(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            let word = crate::text_pos::word_at_position(&doc.source, pos)?;
            doc.resolve_at(offset)?;

            Some(
                doc.word_occurrences(&word)
                    .into_iter()
                    .map(|(start, end)| DocumentHighlight {
                        range: crate::text_pos::byte_range(&doc.source, start, end),
                        kind: Some(DocumentHighlightKind::TEXT),
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();

    connection.respond(id, highlights);
}
