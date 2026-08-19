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
        connection.respond_invalid(id, format!("`{new_name}` is not a valid Verilog-AMS identifier"));
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
        if let Some(edit) = cross_file_rename_edit(doc, &uri, &resolution, decl_span, &new_name) {
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
    // Cross-file is only for a module declared somewhere other than the
    // current document's top level (an `` `include ``d file) — a local
    // declaration takes the single-file path.
    let target = design.module(&target_name)?;
    let module_span = target.span?;
    let ctx = crate::project::ProjectContext::discover(current_uri);
    let source_map = ctx.source_map();

    // One edit site = a flat-buffer span (module header or instance
    // statement), resolved through `source_provenance` to its real file
    // and local range: source 0 is the current document, anything else
    // is an `` `include ``d file whose captured text the provenance
    // already holds.
    fn push_site(
        design: &piperine_lang::Design,
        source_map: &piperine_lang::SourceMap,
        current_uri: &lsp_types::Uri,
        span: miette::SourceSpan,
        token: &dyn Fn(&str, miette::SourceSpan) -> Option<(usize, usize)>,
        out: &mut Vec<(lsp_types::Uri, bool, TextEdit)>,
    ) -> Option<()> {
        let flat =
            piperine_lang::error::PreprocessedSpan::new(span.offset(), span.offset() + span.len());
        let resolved = design.source_provenance.resolve(flat);
        // The resolved local range is authoritative for BOTH files — the
        // flat buffer's offsets are never the document's own when an
        // `` `include `` expanded ahead of the site.
        let local = miette::SourceSpan::new(resolved.start.into(), resolved.end - resolved.start);
        let text = design.source_provenance.text(resolved.source_id);
        let (start, end) = token(text, local)?;
        let uri = if resolved.source_id == piperine_lang::source_provenance::SourceId(0) {
            current_uri.clone()
        } else {
            let raw = design.source_provenance.name(resolved.source_id);
            let path = crate::symbol_index::resolve_include_source_path(source_map, raw)?;
            format!("file://{}", path.display()).parse().ok()?
        };
        out.push((
            uri,
            false,
            TextEdit {
                range: crate::text_pos::byte_range(text, start, end),
                new_text: String::new(),
            },
        ));
        Some(())
    }

    let mut sites: Vec<(lsp_types::Uri, bool, TextEdit)> = Vec::new();
    let name = target_name.clone();
    let header = move |text: &str, span: miette::SourceSpan| header_token_range(text, span, &name);
    push_site(design, &source_map, current_uri, module_span, &header, &mut sites);

    for m in design.modules() {
        for inst in &m.instances {
            if inst.module != target_name {
                continue;
            }
            // The instance TYPE token — VAMS spells it before the label
            // (`A inst(…)`), so the instance's own `type_span` (flat
            // buffer) names it directly; a missing type_span means a
            // pre-retype design — skip rather than guessing.
            let Some(span) = inst.type_span else { continue };
            let (start, end) = (span.offset(), span.offset() + span.len());
            let flat = piperine_lang::error::PreprocessedSpan::new(start, end);
            let resolved = design.source_provenance.resolve(flat);
            let text = design.source_provenance.text(resolved.source_id);
            let uri = if resolved.source_id == piperine_lang::source_provenance::SourceId(0) {
                current_uri.clone()
            } else {
                let raw = design.source_provenance.name(resolved.source_id);
                let path = crate::symbol_index::resolve_include_source_path(&source_map, raw)?;
                format!("file://{}", path.display()).parse().ok()?
            };
            sites.push((
                uri,
                false,
                TextEdit {
                    range: crate::text_pos::byte_range(text, resolved.start, resolved.end),
                    new_text: String::new(),
                },
            ));
        }
    }

    if sites.is_empty() {
        return None;
    }
    // Fill the new_text now that the closures are done.
    for (_, _, edit) in sites.iter_mut() {
        edit.new_text = new_name.to_string();
    }

    // Group by URI into TextDocumentEdits.
    let mut by_uri: std::collections::HashMap<lsp_types::Uri, Vec<TextEdit>> =
        std::collections::HashMap::new();
    for (uri, _, edit) in sites {
        by_uri.entry(uri).or_default().push(edit);
    }
    let document_edits: Vec<TextDocumentEdit> = by_uri
        .into_iter()
        .map(|(uri, edits)| {
            let version = if uri == *current_uri { Some(doc.version) } else { None };
            TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri,
                version,
            },
            edits: edits.into_iter().map(OneOf::Left).collect(),
        }
        })
        .collect();

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

/// Verilog-AMS identifier shape: ASCII letter or `_` first, ASCII alphanumerics
/// and `_` after. Mirrors the lexer's ident rule.
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
