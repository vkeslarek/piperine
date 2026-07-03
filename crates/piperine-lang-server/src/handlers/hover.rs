//! Hover handler: type information on demand.
//!
//! When the user hovers over an identifier, this handler looks up the name
//! in the elaborated `Design` and returns type info for ports, params,
//! wires, modules, and instances.

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
    let word = word_at_position(source, position)?;
    let info = lookup_hover_info(design, &word)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: info,
        }),
        range: None,
    })
}

/// Extract the word (identifier) at the given LSP position.
pub fn word_at_position(source: &str, position: Position) -> Option<String> {
    let offset = position_to_byte(source, position)?;
    let chars: Vec<char> = source.chars().collect();
    if offset >= chars.len() || !chars[offset].is_ascii_alphanumeric() && chars[offset] != '_' {
        // Check if we're just past the end of a word (cursor after last char)
        if offset > 0 && (chars[offset - 1].is_ascii_alphanumeric() || chars[offset - 1] == '_') {
            return extract_word(&chars, offset - 1);
        }
        return None;
    }
    extract_word(&chars, offset)
}

fn extract_word(chars: &[char], pos: usize) -> Option<String> {
    let mut start = pos;
    while start > 0 && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let mut end = pos;
    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    if start < end {
        Some(chars[start..end].iter().collect())
    } else {
        None
    }
}

fn position_to_byte(_source: &str, position: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut byte_offset = 0usize;
    for (i, c) in _source.char_indices() {
        if line == position.line && byte_offset == 0 {
            byte_offset = i;
        }
        if line == position.line {
            let col = _source[byte_offset..i].chars().count() as u32;
            if col >= position.character {
                return Some(i);
            }
        }
        if c == '\n' {
            line += 1;
            if line > position.line {
                byte_offset = i + 1;
                break;
            }
        }
    }
    if byte_offset > 0 || position.line == 0 {
        Some(byte_offset)
    } else {
        None
    }
}

pub fn lookup_hover_info(design: &piperine_lang::Design, word: &str) -> Option<String> {
    // Check if the word is a module name.
    if let Some(m) = design.module(word) {
        return Some(format!(
            "**module** `{}`\n\nPorts: {}\nParams: {}",
            m.name(),
            m.ports().iter().map(|p| format!("`{}: {}`", p.name(), p.net_type().discipline_name())).collect::<Vec<_>>().join(", "),
            m.params().iter().map(|p| format!("`{}`", p.name())).collect::<Vec<_>>().join(", "),
        ));
    }

    // Search all modules for ports, params, wires with this name.
    for m in design.modules() {
        if let Some(port) = m.port(word) {
            return Some(format!(
                "**port** `{}` ({})\n\nDiscipline: `{}`\nModule: `{}`",
                port.name(),
                match port.direction() {
                    piperine_lang::parse::Direction::Input => "input",
                    piperine_lang::parse::Direction::Output => "output",
                    piperine_lang::parse::Direction::Inout => "inout",
                },
                port.net_type().discipline_name(),
                m.name(),
            ));
        }
        if let Some(param) = m.param(word) {
            return Some(format!(
                "**param** `{}`\n\nType: `{:?}`{}\nModule: `{}`",
                param.name(),
                param.value_type(),
                param.default().map(|d| format!("\nDefault: `{:?}`", d)).unwrap_or_default(),
                m.name(),
            ));
        }
        if let Some(wire) = m.wire(word) {
            return Some(format!(
                "**wire** `{}`\n\nDiscipline: `{}`\nModule: `{}`",
                wire.name(),
                wire.net_type().discipline_name(),
                m.name(),
            ));
        }
        if let Some(inst) = m.instance(word) {
            return Some(format!(
                "**instance** `{}` of `{}`\n\nModule: `{}`",
                inst.name(),
                inst.module_name(),
                m.name(),
            ));
        }
    }

    // Check function names.
    if design.function(word).is_some() {
        return Some(format!("**function** `{word}`"));
    }

    // Check discipline names.
    if design.discipline(word).is_some() {
        return Some(format!("**discipline** `{word}`"));
    }

    // Check capability names.
    if design.capability(word).is_some() {
        return Some(format!("**capability** `{word}`"));
    }

    None
}
