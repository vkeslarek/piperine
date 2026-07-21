use lsp_server::{Connection, Request};
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Range};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<GotoDefinitionParams>(connection) else { return };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state
        .documents
        .get(&uri)
        .and_then(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            let decl_span = doc.resolve_at(offset)?.decl_span?;
            Some(crate::text_pos::byte_range(
                &doc.source,
                decl_span.offset(),
                decl_span.offset() + decl_span.len(),
            ))
        })
        .map(|range| GotoDefinitionResponse::Scalar(Location { uri: uri.clone(), range }));

    connection.respond(id, result);
}

/// Kept for tests that still call this function directly
pub fn find_definition(
    source: &str,
    word: &str,
    design: Option<&piperine_lang::Design>,
) -> Option<Range> {
    let design = design?;
    // Fallback for tests: find the word using basic string search since we don't have byte offset
    let pos = source.find(word)?;
    let resolution = crate::symbol_index::resolve_at(design, source, pos, None)?;
    let decl_span = resolution.decl_span?;
    Some(crate::text_pos::byte_range(source, decl_span.offset(), decl_span.offset() + decl_span.len()))
}
