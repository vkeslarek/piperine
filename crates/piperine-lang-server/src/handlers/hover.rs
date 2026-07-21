use lsp_server::{Connection, Request};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use super::{ConnectionExt, RequestExt};
use crate::state::{DocumentState, ServerState};
use crate::symbol_index::SymbolKind;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<HoverParams>(connection) else { return };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state.documents.get(&uri).and_then(|doc| resolve_hover(doc, pos));
    connection.respond(id, result);
}

fn resolve_hover(doc: &DocumentState, position: lsp_types::Position) -> Option<Hover> {
    let offset = crate::text_pos::position_to_byte(&doc.source, position);
    let resolution = doc.resolve_at(offset)?;

    let kind = match resolution.kind {
        SymbolKind::Module => "module",
        SymbolKind::Port => "port",
        SymbolKind::Param => "param",
        SymbolKind::Wire => "wire",
        SymbolKind::Var => "var",
        SymbolKind::Instance => "instance",
        SymbolKind::Behavior => "behavior",
        SymbolKind::Function => "function",
        SymbolKind::Enum => "enum",
        SymbolKind::Bundle => "bundle",
        SymbolKind::Discipline => "discipline",
        SymbolKind::Capability => "capability",
        SymbolKind::Type => "type",
        SymbolKind::Operator => "operator",
        SymbolKind::AttrSchema => "attribute schema",
    };
    let mut info = format!("**{kind}** `{}`", resolution.name);
    if let Some(ty) = &resolution.type_info {
        info.push_str("\n\n");
        info.push_str(ty);
    }

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
