use lsp_server::{Connection, Request, Response};
use lsp_types::{GotoDefinitionResponse, Location, Range};
use serde_json::from_value;

use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let (id, params) = (req.id, req.params);
    let params = match from_value::<lsp_types::GotoDefinitionParams>(params) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("invalid goto-def params: {e}");
            return;
        }
    };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state
        .documents
        .get(&uri)
        .and_then(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            let resolution = crate::symbol_index::resolve_at(doc.design.as_ref()?, &doc.source, offset)?;
            let decl_span = resolution.decl_span?;
            Some(crate::text_pos::byte_range(&doc.source, decl_span.offset(), decl_span.offset() + decl_span.len()))
        })
        .map(|range| GotoDefinitionResponse::Scalar(Location { uri: uri.clone(), range }));

    let response = Response {
        id,
        result: result.map_or(Some(serde_json::Value::Null), |r| Some(serde_json::to_value(r).unwrap())),
        error: None,
    };
    connection
        .sender
        .send(lsp_server::Message::Response(response))
        .unwrap();
}

/// Kept for tests that still call this function directly
pub fn find_definition(
    source: &str,
    word: &str,
    design: Option<&piperine_lang::Design>,
) -> Option<Range> {
    if let Some(design) = design {
        // Fallback for tests: find the word using basic string search since we don't have byte offset
        let pos = source.find(word)?;
        let resolution = crate::symbol_index::resolve_at(design, source, pos)?;
        let decl_span = resolution.decl_span?;
        return Some(crate::text_pos::byte_range(source, decl_span.offset(), decl_span.offset() + decl_span.len()));
    }
    None
}
