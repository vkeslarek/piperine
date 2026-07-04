use lsp_server::{Connection, Request, Response};
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};
use serde_json::from_value;

use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let (id, params) = (req.id, req.params);
    let params = match from_value::<lsp_types::HoverParams>(params) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("invalid hover params: {e}");
            return;
        }
    };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state
        .documents
        .get(&uri)
        .and_then(|doc| doc.design.as_ref())
        .and_then(|design| resolve_hover(design, &doc_text(&uri, state), pos));

    let response = Response {
        id,
        result: result.map_or(Some(serde_json::Value::Null), |h| Some(serde_json::to_value(h).unwrap())),
        error: None,
    };
    connection.sender.send(lsp_server::Message::Response(response)).unwrap();
}

fn doc_text(uri: &lsp_types::Uri, state: &ServerState) -> String {
    state.documents.get(uri).map(|d| d.source.clone()).unwrap_or_default()
}

fn resolve_hover(
    design: &piperine_lang::Design,
    source: &str,
    position: Position,
) -> Option<Hover> {
    let offset = crate::text_pos::position_to_byte(source, position);
    let resolution = crate::symbol_index::resolve_at(design, source, offset)?;
    
    let info = match resolution.kind {
        crate::symbol_index::SymbolKind::Module => format!("**module** `{}`", resolution.name),
        crate::symbol_index::SymbolKind::Port => format!("**port** `{}`\n\n{}", resolution.name, resolution.type_info.as_deref().unwrap_or("")),
        crate::symbol_index::SymbolKind::Param => format!("**param** `{}`\n\n{}", resolution.name, resolution.type_info.as_deref().unwrap_or("")),
        crate::symbol_index::SymbolKind::Wire => format!("**wire** `{}`\n\n{}", resolution.name, resolution.type_info.as_deref().unwrap_or("")),
        crate::symbol_index::SymbolKind::Var => format!("**var** `{}`\n\n{}", resolution.name, resolution.type_info.as_deref().unwrap_or("")),
        crate::symbol_index::SymbolKind::Instance => format!("**instance** `{}`\n\n{}", resolution.name, resolution.type_info.as_deref().unwrap_or("")),
        crate::symbol_index::SymbolKind::Behavior => format!("**behavior** `{}`\n\n{}", resolution.name, resolution.type_info.as_deref().unwrap_or("")),
        crate::symbol_index::SymbolKind::Function => format!("**function** `{}`", resolution.name),
        crate::symbol_index::SymbolKind::Bench => format!("**bench** `{}`", resolution.name),
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: info,
        }),
        range: None,
    })
}

/// Kept for tests that still call this function directly
pub fn lookup_hover_info(design: &piperine_lang::Design, word: &str) -> Option<String> {
    // Basic mock lookup for tests since tests don't have source string
    for m in design.modules() {
        if m.name() == word { return Some(format!("**module** `{}`", word)); }
        if m.port(word).is_some() { return Some(format!("**port** `{}`", word)); }
        if m.param(word).is_some() { return Some(format!("**param** `{}`", word)); }
    }
    None
}
