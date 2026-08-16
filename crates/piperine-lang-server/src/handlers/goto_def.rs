use lsp_server::{Connection, Request};
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use crate::symbol_index::{include_directive_path_at, resolve_decl_location, resolve_include_source_path, Resolution};

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<GotoDefinitionParams>(connection) else { return };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);

        // A click on the `` `include "path" `` directive itself opens
        // that file — checked before symbol resolution, since the
        // directive's quoted path is preprocessor syntax, not a POM
        // symbol `resolve_at` has any binding for.
        if let Some(raw_path) = include_directive_path_at(&doc.source, offset) {
            let ctx = crate::project::ProjectContext::discover(&uri);
            let source_map = ctx.source_map();
            let path = resolve_include_source_path(&source_map, &raw_path)?;
            let target_uri: lsp_types::Uri = format!("file://{}", path.display()).parse().ok()?;
            return Some(GotoDefinitionResponse::Scalar(lsp_types::Location {
                uri: target_uri,
                range: lsp_types::Range {
                    start: lsp_types::Position { line: 0, character: 0 },
                    end: lsp_types::Position { line: 0, character: 0 },
                },
            }));
        }

        let resolution = doc.resolve_at(offset)?;
        let decl_span = resolution.decl_span?;
        let design = doc.design.as_ref()?;

        // BUG-1 (LSB-01..03): extern-registry resolutions (Type/Operator/
        // Function/AttrSchema) carry their own real on-disk declaring file
        // directly on `Resolution` (a stdlib header today), already a real
        // path — not a `` `include `` argument string needing re-resolution,
        // so this stays its own branch ahead of the general
        // `resolve_decl_location` path below.
        if let Some(location) = extern_registry_location(&uri, &resolution, decl_span) {
            return Some(GotoDefinitionResponse::Scalar(location));
        }

        // Every other decl_span — module/param/port/wire/var/instance/
        // discipline/nature/behavior — is a flat-preprocessed-buffer
        // offset (T3's rename never changed what the numbers mean).
        // `resolve_decl_location` is what actually fixes Ctrl+click: it
        // resolves through `design.source_provenance` to the real
        // originating file (same document, or a genuine `` `include ``d
        // one) instead of naively applying the raw offset to this
        // document's own text (the root cause behind three of four
        // VS Code Ctrl+click bugs found testing this feature).
        let ctx = crate::project::ProjectContext::discover(&uri);
        let source_map = ctx.source_map();
        let location = resolve_decl_location(design, &uri, Some(&source_map), decl_span)?;
        Some(GotoDefinitionResponse::Scalar(location))
    });

    connection.respond(id, result);
}

fn extern_registry_location(
    current_uri: &lsp_types::Uri,
    resolution: &Resolution,
    decl_span: miette::SourceSpan,
) -> Option<lsp_types::Location> {
    let file = resolution.file.as_ref()?;
    let current_path = url::Url::parse(current_uri.as_str()).ok()?.to_file_path().ok()?;
    let same_file = std::fs::canonicalize(file)
        .ok()
        .zip(std::fs::canonicalize(&current_path).ok())
        .is_some_and(|(a, b)| a == b);
    if same_file {
        return None;
    }
    let content = std::fs::read_to_string(file).ok()?;
    let range =
        crate::text_pos::byte_range(&content, decl_span.offset(), decl_span.offset() + decl_span.len());
    let uri: lsp_types::Uri = format!("file://{}", file.display()).parse().ok()?;
    Some(lsp_types::Location { uri, range })
}

/// Kept for tests that still call this function directly
pub fn find_definition(
    source: &str,
    word: &str,
    design: Option<&piperine_lang::Design>,
) -> Option<lsp_types::Range> {
    let design = design?;
    // Fallback for tests: find the word using basic string search since we don't have byte offset
    let pos = source.find(word)?;
    let resolution = crate::symbol_index::resolve_at(design, source, pos, None)?;
    let decl_span = resolution.decl_span?;
    Some(crate::text_pos::byte_range(source, decl_span.offset(), decl_span.offset() + decl_span.len()))
}
