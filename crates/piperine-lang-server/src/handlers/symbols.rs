//! Document symbols: returns a hierarchical outline of modules, ports,
//! params, wires, behaviors, and instances.

use lsp_server::{Connection, Request, Response};
use lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, Position, Range, SymbolKind,
};
use serde_json::from_value;

use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let (id, params) = (req.id, req.params);
    let params = match from_value::<lsp_types::DocumentSymbolParams>(params) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("invalid documentSymbol params: {e}");
            return;
        }
    };

    let uri = params.text_document.uri;

    let symbols = state
        .documents
        .get(&uri)
        .and_then(|doc| doc.design.as_ref())
        .map(|design| extract_symbols(design, &doc_text(&uri, state)))
        .unwrap_or_default();

    let result = DocumentSymbolResponse::Nested(symbols);

    let response = Response {
        id,
        result: Some(serde_json::to_value(result).unwrap()),
        error: None,
    };
    connection.sender.send(lsp_server::Message::Response(response)).unwrap();
}

fn doc_text(uri: &lsp_types::Uri, state: &ServerState) -> String {
    state.documents.get(uri).map(|d| d.source.clone()).unwrap_or_default()
}

#[allow(deprecated)]
fn extract_symbols(design: &piperine_lang::Design, source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for module in design.modules() {
        let mut children = Vec::new();

        // Ports
        for port in module.ports() {
            if let Some(range) = find_decl_range(source, &format!("input {}", port.name()))
                .or_else(|| find_decl_range(source, &format!("output {}", port.name())))
                .or_else(|| find_decl_range(source, &format!("inout {}", port.name())))
            {
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
            if let Some(range) = find_decl_range(source, &format!("param {}", param.name())) {
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
            if let Some(range) = find_decl_range(source, &format!("wire {}", wire.name())) {
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
            if let Some(range) = find_decl_range(source, &format!("{} {}", kind_str, behavior.name())) {
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
            let label = inst.label().unwrap_or(inst.module_name());
            if let Some(range) = find_decl_range(source, label) {
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

        if let Some(range) = find_decl_range(source, &format!("mod {}", module.name())) {
            let end_pos = find_end_of_module(source, range.start);
            let mod_range = Range { start: range.start, end: end_pos };
            symbols.push(DocumentSymbol {
                name: module.name().to_string(),
                detail: Some(format!("{} ports, {} params", module.ports().len(), module.params().len())),
                kind: SymbolKind::MODULE,
                range: mod_range,
                selection_range: range,
                children: Some(children),
                tags: None,
                deprecated: None
            });
        }
    }

    symbols
}

/// Find the byte range of a declaration in source by matching the text.
fn find_decl_range(source: &str, pattern: &str) -> Option<Range> {
    let pos = source.find(pattern)?;
    let end = pos + pattern.len();
    let (sl, sc) = byte_to_line_col(source, pos);
    let (el, ec) = byte_to_line_col(source, end);
    Some(Range {
        start: Position { line: sl, character: sc },
        end: Position { line: el, character: ec },
    })
}

/// Find a reasonable end position for a module (closing brace or EOF).
fn find_end_of_module(source: &str, _start: Position) -> Position {
    // Simple heuristic: find the last `}` after the module declaration
    // or fall back to end of file.
    let len = source.len();
    let (line, col) = byte_to_line_col(source, len);
    Position { line, character: col }
}

fn byte_to_line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let offset = byte_offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() as u32;
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = (offset - last_newline) as u32;
    (line, col)
}
