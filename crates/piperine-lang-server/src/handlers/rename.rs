use lsp_server::{Connection, Request};
use lsp_types::{
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, PrepareRenameResponse,
    RenameParams, TextDocumentEdit, TextDocumentPositionParams, TextEdit, WorkspaceEdit,
};
use std::collections::HashMap;

use super::{ConnectionExt, RequestExt};
use crate::state::{DocumentState, ServerState};
use crate::symbol_index::{Resolution, SymbolKind};

#[allow(clippy::mutable_key_type)]
pub fn handle_rename(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<RenameParams>(connection) else { return };

    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;

    if !is_valid_identifier(&new_name) {
        connection.respond_invalid(id, format!("`{new_name}` is not a valid PHDL identifier"));
        return;
    }

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);
        let resolution = doc.resolve_at(offset)?;
        let decl_span = resolution.decl_span?;

        // Cross-file rename (T14/LSP-12): when the renamed symbol was
        // `use`-imported from another project file, edit every file that
        // declares or instantiates it, not just the current buffer — a
        // `decl_span`/`occurrences_at` result for such a symbol carries
        // byte offsets from a *foreign* file's coordinate space (see
        // T13's goto fix), so blindly emitting a `TextEdit` against the
        // current document would corrupt it.
        if let Some(edit) = cross_file_rename_edit(state, doc, &uri, &resolution, decl_span, &new_name) {
            return Some(edit);
        }

        // occurrences_at (T8) already gates on symbol resolution and
        // returns only the binding's own recorded uses — a same-named
        // identifier in an unrelated scope is never in this set (LSP-11).
        let occurrences = doc.occurrences_at(offset);
        if occurrences.is_empty() {
            return None;
        }

        let edits = occurrences
            .into_iter()
            .map(|(start, end)| TextEdit {
                range: crate::text_pos::byte_range(&doc.source, start, end),
                new_text: new_name.clone(),
            })
            .collect();

        let mut changes = HashMap::new();
        changes.insert(uri.clone(), edits);

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    });

    connection.respond(id, result);
}

/// Build a multi-file `WorkspaceEdit.document_changes` (T14/LSP-12) for a
/// `use`-imported module renamed either at its own declaration or via an
/// instantiation's type name. Returns `None` when the resolved symbol
/// isn't a cross-file module (falls through to the existing single-file
/// path) or when no project unit is available (LSP-17: standalone
/// documents are unaffected).
#[allow(clippy::mutable_key_type)]
fn cross_file_rename_edit(
    state: &ServerState,
    doc: &DocumentState,
    current_uri: &lsp_types::Uri,
    resolution: &Resolution,
    decl_span: miette::SourceSpan,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let design = doc.design.as_ref()?;

    let target_name = match resolution.kind {
        SymbolKind::Module => resolution.name.clone(),
        SymbolKind::Instance => crate::symbol_index::instance_module_type_at(design, decl_span)?,
        _ => return None,
    };
    // A project-local declaration has no origin entry — the existing
    // single-file rename path already handles it correctly.
    design.project().origin_of(&target_name)?;

    let root = doc.project_root.as_ref()?;
    let unit = state.projects.get(root)?;
    let current_path = url::Url::parse(current_uri.as_str()).ok()?.to_file_path().ok()?;

    let mut document_edits = Vec::new();
    for (path, other_design) in &unit.designs {
        let text = if path == &current_path {
            doc.source.clone()
        } else {
            match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(_) => continue,
            }
        };

        let mut edits = Vec::new();

        // This file's own declaration of the target module (skipped for
        // files that only re-import it — same origin-absence check as
        // cross-file goto, T13).
        if other_design.project().origin_of(&target_name).is_none()
            && let Some(m) = other_design.module(&target_name)
            && let Some(span) = m.span
            && let Some((start, end)) = header_token_range(&text, span, &target_name)
        {
            edits.push(TextEdit {
                range: crate::text_pos::byte_range(&text, start, end),
                new_text: new_name.to_string(),
            });
        }

        // Every instance of the target module declared directly in this
        // file (only modules this file itself owns — an inlined `use`
        // copy's instance spans belong to a foreign coordinate space too).
        for m in other_design.modules() {
            if other_design.project().origin_of(&m.name).is_some() {
                continue;
            }
            for inst in &m.instances {
                if inst.module != target_name {
                    continue;
                }
                let Some(span) = inst.span else { continue };
                let Some((start, end)) = instance_type_token_range(&text, span, inst.label.as_deref(), &target_name)
                else {
                    continue;
                };
                edits.push(TextEdit {
                    range: crate::text_pos::byte_range(&text, start, end),
                    new_text: new_name.to_string(),
                });
            }
        }

        if edits.is_empty() {
            continue;
        }
        let uri: lsp_types::Uri = format!("file://{}", path.display()).parse().ok()?;
        document_edits.push(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri,
                version: if path == &current_path { Some(doc.version) } else { None },
            },
            edits: edits.into_iter().map(OneOf::Left).collect(),
        });
    }

    if document_edits.is_empty() {
        return None;
    }

    Some(WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Edits(document_edits)),
        change_annotations: None,
    })
}

/// Find `needle`'s first whole-word byte range within `haystack`.
fn find_whole_word(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let bytes = haystack.as_bytes();
    let is_word_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = 0;
    while let Some(idx) = haystack[start..].find(needle) {
        let begin = start + idx;
        let end = begin + needle.len();
        let bounded_left = begin == 0 || !is_word_byte(bytes[begin - 1]);
        let bounded_right = end == bytes.len() || !is_word_byte(bytes[end]);
        if bounded_left && bounded_right {
            return Some((begin, end));
        }
        start = end;
    }
    None
}

/// The module name token's absolute byte range within a module
/// declaration's own `span` — searched only up to the port list's opening
/// `(` (`mod Name (...)  { ... }`) so the module's own name can't be
/// confused with a same-named reference inside its body.
fn header_token_range(text: &str, span: miette::SourceSpan, target_name: &str) -> Option<(usize, usize)> {
    let decl_text = text.get(span.offset()..span.offset() + span.len())?;
    let header_end = decl_text.find('(').unwrap_or(decl_text.len());
    let (rel_start, rel_end) = find_whole_word(&decl_text[..header_end], target_name)?;
    Some((span.offset() + rel_start, span.offset() + rel_end))
}

/// The instantiated module type's absolute byte range within an instance
/// statement's own `span`. When the instance has a label
/// (`label: Type(...)`), the search starts after the `:` so a label that
/// happens to equal the module's name is never mistaken for the type.
fn instance_type_token_range(
    text: &str,
    span: miette::SourceSpan,
    label: Option<&str>,
    target_name: &str,
) -> Option<(usize, usize)> {
    let inst_text = text.get(span.offset()..span.offset() + span.len())?;
    let search_from = if label.is_some() { inst_text.find(':').map(|p| p + 1).unwrap_or(0) } else { 0 };
    let (rel_start, rel_end) = find_whole_word(&inst_text[search_from..], target_name)?;
    Some((span.offset() + search_from + rel_start, span.offset() + search_from + rel_end))
}

pub fn handle_prepare_rename(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<TextDocumentPositionParams>(connection) else { return };

    let uri = params.text_document.uri;
    let pos = params.position;

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);
        // A keyword/literal/comment never resolves to a symbol, so
        // occurrences_at is empty and prepare-rename correctly declines
        // (LSP-11 edge case: decline on keyword/literal).
        doc.occurrences_at(offset)
            .into_iter()
            .find(|&(start, end)| offset >= start && offset <= end)
            .map(|(start, end)| {
                PrepareRenameResponse::Range(crate::text_pos::byte_range(&doc.source, start, end))
            })
    });

    connection.respond(id, result);
}

/// PHDL identifier shape: ASCII letter or `_` first, ASCII alphanumerics
/// and `_` after. Mirrors the lexer's ident rule.
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
