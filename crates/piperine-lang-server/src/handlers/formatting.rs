//! Document formatting handler.

use lsp_server::{Connection, Request};
use lsp_types::{DocumentFormattingParams, Position, Range, TextEdit};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<DocumentFormattingParams>(connection) else { return };

    let uri = params.text_document.uri;

    let result = state.documents.get(&uri).and_then(|doc| {
        // A document that does not lex/parse must not be formatted —
        // `piperine_vams::Formatter::format` (T15) fails loud on either,
        // matching the old "do not replace with garbage" guard.
        let formatted = piperine_vams::Formatter::format(&doc.source).ok()?;

        if formatted == doc.source {
            return Some(Vec::new());
        }

        let range = Range {
            start: Position { line: 0, character: 0 },
            end: crate::text_pos::byte_to_position(&doc.source, doc.source.len()),
        };
        Some(vec![TextEdit { range, new_text: formatted }])
    });

    connection.respond(id, result);
}
