//! Document formatting handler.

use lsp_server::{Connection, Request};
use lsp_types::{DocumentFormattingParams, Position, Range, TextEdit};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<DocumentFormattingParams>(connection) else { return };

    let uri = params.text_document.uri;

    let result = state.documents.get(&uri).and_then(|doc| {
        // A document that does not lex must not be formatted — formatting
        // an empty token stream would replace the text with garbage.
        let tokens = piperine_lang::parse::lexer::Lexer::new(&doc.source)
            .tokenize_all()
            .ok()?;
        let options = piperine_lang::parse::format::FormatOptions::default();
        let formatted = piperine_lang::parse::format::TokenFormatter::format_source(
            &doc.source,
            &tokens,
            options,
        );

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
