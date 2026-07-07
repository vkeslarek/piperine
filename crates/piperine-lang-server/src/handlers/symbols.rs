//! Document symbols: returns a hierarchical outline of modules, ports,
//! params, wires, behaviors, and instances.

use lsp_server::{Connection, Request};
use lsp_types::{DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Range, SymbolKind};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<DocumentSymbolParams>(connection) else { return };

    let symbols = state
        .documents
        .get(&params.text_document.uri)
        .and_then(|doc| Some(extract_symbols(doc.design.as_ref()?, &doc.source)))
        .unwrap_or_default();

    connection.respond(id, DocumentSymbolResponse::Nested(symbols));
}

#[allow(deprecated)]
fn extract_symbols(design: &piperine_lang::Design, source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for module in design.modules() {
        let mut children = Vec::new();

        // Ports
        for port in module.ports() {
            if let Some(range) = span_to_range(source, port.span) {
                children.push(DocumentSymbol {
                    name: format!("{} ({})", port.name(), port.net_type().discipline_name()),
                    detail: Some(format!("{:?}", port.direction())),
                    kind: SymbolKind::PROPERTY,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None
                });
            }
        }

        // Params
        for param in module.params() {
            if let Some(range) = span_to_range(source, param.span) {
                children.push(DocumentSymbol {
                    name: param.name().to_string(),
                    detail: Some(format!("{:?}", param.value_type())),
                    kind: SymbolKind::VARIABLE,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None
                });
            }
        }

        // Wires
        for wire in module.wires() {
            if let Some(range) = span_to_range(source, wire.span) {
                children.push(DocumentSymbol {
                    name: wire.name().to_string(),
                    detail: Some(wire.net_type().discipline_name().to_string()),
                    kind: SymbolKind::FIELD,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None
                });
            }
        }

        // Behaviors
        for behavior in module.behaviors() {
            let kind_str = if behavior.is_analog() { "analog" } else { "digital" };
            if let Some(range) = span_to_range(source, behavior.span) {
                children.push(DocumentSymbol {
                    name: behavior.name().to_string(),
                    detail: Some(kind_str.to_string()),
                    kind: SymbolKind::METHOD,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None
                });
            }
        }

        // Instances
        for inst in module.instances() {
            if let Some(range) = span_to_range(source, inst.span) {
                children.push(DocumentSymbol {
                    name: inst.name().to_string(),
                    detail: Some(format!("instance of {}", inst.module_name())),
                    kind: SymbolKind::OBJECT,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None
                });
            }
        }

        if let Some(range) = span_to_range(source, module.span) {
            symbols.push(DocumentSymbol {
                name: module.name().to_string(),
                detail: Some(format!("{} ports, {} params", module.ports().len(), module.params().len())),
                kind: SymbolKind::MODULE,
                range,
                selection_range: range,
                children: Some(children),
                tags: None,
                deprecated: None
            });
        }
    }

    for (name, e) in design.enums() {
        if let Some(range) = span_to_range(source, e.span) {
            symbols.push(DocumentSymbol {
                name: name.clone(),
                detail: Some("enum".into()),
                kind: SymbolKind::ENUM,
                range,
                selection_range: range,
                children: None,
                tags: None,
                #[allow(deprecated)]
                deprecated: None
            });
        }
    }

    for (name, b) in design.bundles() {
        if let Some(range) = span_to_range(source, b.span) {
            symbols.push(DocumentSymbol {
                name: name.clone(),
                detail: Some("bundle".into()),
                kind: SymbolKind::STRUCT,
                range,
                selection_range: range,
                children: None,
                tags: None,
                #[allow(deprecated)]
                deprecated: None
            });
        }
    }

    for (name, d) in design.disciplines() {
        if let Some(range) = span_to_range(source, d.span) {
            symbols.push(DocumentSymbol {
                name: name.clone(),
                detail: Some("discipline".into()),
                kind: SymbolKind::INTERFACE,
                range,
                selection_range: range,
                children: None,
                tags: None,
                #[allow(deprecated)]
                deprecated: None
            });
        }
    }

    for (name, c) in design.capabilities() {
        if let Some(range) = span_to_range(source, c.span) {
            symbols.push(DocumentSymbol {
                name: name.clone(),
                detail: Some("capability".into()),
                kind: SymbolKind::INTERFACE,
                range,
                selection_range: range,
                children: None,
                tags: None,
                #[allow(deprecated)]
                deprecated: None
            });
        }
    }

    for i in design.impls() {
        let mut children = Vec::new();
        for m in &i.methods {
            if let Some(range) = span_to_range(source, m.span) {
                children.push(DocumentSymbol {
                    name: m.name.clone(),
                    detail: Some("method".into()),
                    kind: SymbolKind::METHOD,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None
                });
            }
        }
        if let Some(range) = span_to_range(source, i.span) {
            symbols.push(DocumentSymbol {
                name: format!("impl {} for {}", i.capability.as_deref().unwrap_or(""), i.ty),
                detail: Some("impl".into()),
                kind: SymbolKind::CLASS,
                range,
                selection_range: range,
                children: Some(children),
                tags: None,
                #[allow(deprecated)]
                deprecated: None
            });
        }
    }

    symbols
}

fn span_to_range(source: &str, span: Option<miette::SourceSpan>) -> Option<Range> {
    let span = span?;
    let start = span.offset();
    let end = start + span.len();
    Some(Range {
        start: crate::text_pos::byte_to_position(source, start),
        end: crate::text_pos::byte_to_position(source, end),
    })
}
