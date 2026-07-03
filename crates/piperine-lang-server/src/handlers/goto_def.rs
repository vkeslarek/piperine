//! Go-to-definition: navigates from an identifier to its declaration.
//!
//! Uses the elaborated Design to resolve module names, function names,
//! discipline names, etc. Falls back to string matching for unresolved
//! identifiers.

use lsp_server::{Connection, Request, Response};
use lsp_types::{GotoDefinitionResponse, Location, Position, Range};
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
            let word = word_at_position(&doc.source, pos)?;
            find_definition(&doc.source, &word, doc.design.as_ref())
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

/// Extract the word at a given LSP position.
fn word_at_position(source: &str, position: Position) -> Option<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, &c) in chars.iter().enumerate() {
        if line == position.line && col == position.character {
            if c.is_ascii_alphanumeric() || c == '_' {
                // Cursor on a word character — extract the full word.
                let mut start = i;
                while start > 0
                    && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_')
                {
                    start -= 1;
                }
                let mut end = i;
                while end < chars.len()
                    && (chars[end].is_ascii_alphanumeric() || chars[end] == '_')
                {
                    end += 1;
                }
                return Some(chars[start..end].iter().collect());
            }
            // Cursor on whitespace just after a word — grab the previous word.
            if i > 0 && (chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_') {
                let mut start = i - 1;
                while start > 0
                    && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_')
                {
                    start -= 1;
                }
                return Some(chars[start..i].iter().collect());
            }
            return None;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    None
}

/// Search the source for the declaration of `word` and return its range.
///
/// When a Design is available, knows which kind of thing `word` is
/// (module, discipline, function, capability) and looks for the
/// appropriate declaration keyword.
pub fn find_definition(
    source: &str,
    word: &str,
    design: Option<&piperine_lang::Design>,
) -> Option<Range> {
    if let Some(design) = design {
        // Check what kind of entity this word is.
        if design.module(word).is_some() {
            return find_decl(source, "mod ", word)
                .or_else(|| find_decl(source, "mod ", word));
        }
        if design.discipline(word).is_some() {
            return find_decl(source, "discipline ", word);
        }
        if design.capability(word).is_some() {
            return find_decl(source, "capability ", word);
        }
        if design.function(word).is_some() {
            return find_decl(source, "fn ", word);
        }
        // It might be a port/param/wire inside a module.
        for m in design.modules() {
            if m.port(word).is_some()
                && let Some(r) = find_decl(source, "input ", word)
                    .or_else(|| find_decl(source, "output ", word))
                    .or_else(|| find_decl(source, "inout ", word))
                {
                    return Some(r);
                }
            if m.param(word).is_some()
                && let Some(r) = find_decl(source, "param ", word) {
                    return Some(r);
                }
            if m.wire(word).is_some()
                && let Some(r) = find_decl(source, "wire ", word) {
                    return Some(r);
                }
        }
    }

    // Fallback: try common declaration patterns.
    let patterns = [
        "mod ",
        "fn ",
        "analog ",
        "digital ",
        "discipline ",
        "bundle ",
        "enum ",
        "capability ",
        "param ",
        "wire ",
        "input ",
        "output ",
        "inout ",
    ];
    for pattern in &patterns {
        if let Some(r) = find_decl(source, pattern, word) {
            return Some(r);
        }
    }
    None
}

fn find_decl(source: &str, prefix: &str, name: &str) -> Option<Range> {
    let search = format!("{prefix}{name}");
    let pos = source.find(&search)?;
    let name_start = pos + prefix.len();
    let name_end = name_start + name.len();
    let (sl, sc) = byte_to_line_col(source, name_start);
    let (el, ec) = byte_to_line_col(source, name_end);
    Some(Range {
        start: Position {
            line: sl,
            character: sc,
        },
        end: Position {
            line: el,
            character: ec,
        },
    })
}

fn byte_to_line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let offset = byte_offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() as u32;
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = (offset - last_newline) as u32;
    (line, col)
}
