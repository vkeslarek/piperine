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
    let mut info = String::new();
    if let Some(rdoc) = &resolution.doc {
        info.push_str(rdoc);
        info.push_str("\n\n");
    }
    info.push_str(&format!("**{kind}** `{}`", resolution.name));
    if let Some(ty) = &resolution.type_info {
        info.push_str("\n\n");
        info.push_str(ty);
    }
    // T20/LSP-22: hovering a schema name shows its fields (name/type/
    // required), the same info a bundle-backed or `extern attribute`
    // declaration validates `@name(...)` use sites against.
    if resolution.kind == SymbolKind::AttrSchema
        && let Some(fields) = schema_field_summary(doc, &resolution.name) {
        info.push_str("\n\n");
        info.push_str(&fields);
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: info,
        }),
        range: None,
    })
}

/// The Markdown-formatted field list (`name: Type` / `name?: Type`) for a
/// registered schema, reading `ctx.schemas`' shape (T20/LSP-22): bundle-
/// backed schemas read their fields off the named bundle's declaration
/// (`design.bundles()`); declared schemas (`extern attribute`, host/plugin-
/// registered) carry their fields directly.
fn schema_field_summary(doc: &DocumentState, schema_name: &str) -> Option<String> {
    use piperine_lang::elab::registry::SchemaShape;

    let ctx = doc.ctx.as_ref()?;
    let shape = ctx.schemas.shape(schema_name)?;
    let mut lines = vec!["**Fields**:".to_string()];
    match shape {
        SchemaShape::Bundle(bundle_name) => {
            let design = doc.design.as_ref()?;
            let (_, bundle) = design.bundles().find(|(name, _)| name.as_str() == bundle_name)?;
            for field in &bundle.fields {
                let opt = if field.default.is_some() { "?" } else { "" };
                lines.push(format!("- `{}{opt}: {}`", field.name, field.ty.name));
            }
        }
        SchemaShape::Declared(fields) => {
            for field in fields {
                let opt = if field.required { "" } else { "?" };
                lines.push(format!("- `{}{opt}: {}`", field.name, field.ty));
            }
        }
    }
    Some(lines.join("\n"))
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
