use lsp_server::{Connection, Request};
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Range};

use super::{ConnectionExt, RequestExt};
use crate::state::{DocumentState, ServerState};
use crate::symbol_index::{Resolution, SymbolKind};

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<GotoDefinitionParams>(connection) else { return };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);
        let resolution = doc.resolve_at(offset)?;
        let decl_span = resolution.decl_span?;

        // Cross-file goto (T13/LSP-15): a `use`-imported module's decl_span
        // holds byte offsets copied through unchanged from its *own* file's
        // parse (`Resolver::expand` inlines the AST node as-is) — applying
        // them to this document's buffer would land on the wrong text.
        // When the resolved binding was actually declared in another
        // project file, jump there instead.
        if let Some(location) = cross_file_location(state, doc, &uri, &resolution, decl_span) {
            return Some(GotoDefinitionResponse::Scalar(location));
        }

        Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: crate::text_pos::byte_range(
                &doc.source,
                decl_span.offset(),
                decl_span.offset() + decl_span.len(),
            ),
        }))
    });

    connection.respond(id, result);
}

/// When `resolution` names (or, for an instantiation, instantiates) a
/// module that was `use`-imported into the current document (i.e. it
/// carries a provenance entry in `Design::project().origins`), find the
/// project file whose *own* elaboration declares that module directly (no
/// origin entry — the `use`-importer's copy is attributed to the wrong
/// file by `ProjectUnit::build`'s per-file `set_file` stamp) and return a
/// `Location` computed against that file's on-disk text.
fn cross_file_location(
    state: &ServerState,
    doc: &DocumentState,
    current_uri: &lsp_types::Uri,
    resolution: &Resolution,
    decl_span: miette::SourceSpan,
) -> Option<Location> {
    let design = doc.design.as_ref()?;

    // BUG-1 (LSB-01..03): extern-registry resolutions (Type/Operator/
    // Function/AttrSchema) carry their own real on-disk declaring file
    // directly on `Resolution` (a stdlib header today) — checked before
    // the Module/Instance-only logic below, since extern items don't need
    // cross-design lookup, just the file + the already-resolved decl_span.
    if let Some(file) = &resolution.file {
        let current_path = url::Url::parse(current_uri.as_str()).ok()?.to_file_path().ok()?;
        let same_file = std::fs::canonicalize(file)
            .ok()
            .zip(std::fs::canonicalize(&current_path).ok())
            .is_some_and(|(a, b)| a == b);
        if same_file {
            // Declared in the current document itself — fall through to
            // the caller's same-file fallback (no regression: LSB-03).
            return None;
        }
        let content = std::fs::read_to_string(file).ok()?;
        let range = crate::text_pos::byte_range(
            &content,
            decl_span.offset(),
            decl_span.offset() + decl_span.len(),
        );
        let uri: lsp_types::Uri = format!("file://{}", file.display()).parse().ok()?;
        return Some(Location { uri, range });
    }

    // `resolve_in_module`'s instance branch matches on either the
    // instance's label *or* its module type (`i.module == word`), so a
    // click on the type name inside `label: Type(...)` also comes back as
    // `SymbolKind::Instance` with `decl_span` set to the whole instance
    // statement's span, not the module's. Recover the instantiated
    // module's name from the POM instance at that exact span so the
    // cross-file check below runs against the *type*, not the label.
    let target_name = match resolution.kind {
        SymbolKind::Module => resolution.name.clone(),
        SymbolKind::Instance => crate::symbol_index::instance_module_type_at(design, decl_span)?,
        _ => return None,
    };

    // A project-local declaration has no origin entry — nothing to jump
    // across files for (LSP-17: no regression for the single-file case).
    design.project().origin_of(&target_name)?;

    let root = doc.project_root.as_ref()?;
    let unit = state.projects.get(root)?;
    let current_path = url::Url::parse(current_uri.as_str()).ok()?.to_file_path().ok()?;

    unit.designs.iter().find_map(|(path, other_design)| {
        if path == &current_path {
            return None;
        }
        // Skip files that only *re-import* this name too — only the
        // file that declares it with no provenance entry is the real
        // owner.
        if other_design.project().origin_of(&target_name).is_some() {
            return None;
        }
        let m = other_design.module(&target_name)?;
        let span = m.span?;
        let content = std::fs::read_to_string(path).ok()?;
        let range =
            crate::text_pos::byte_range(&content, span.offset(), span.offset() + span.len());
        let uri: lsp_types::Uri = format!("file://{}", path.display()).parse().ok()?;
        Some(Location { uri, range })
    })
}

/// Kept for tests that still call this function directly
pub fn find_definition(
    source: &str,
    word: &str,
    design: Option<&piperine_lang::Design>,
) -> Option<Range> {
    let design = design?;
    // Fallback for tests: find the word using basic string search since we don't have byte offset
    let pos = source.find(word)?;
    let resolution = crate::symbol_index::resolve_at(design, source, pos, None)?;
    let decl_span = resolution.decl_span?;
    Some(crate::text_pos::byte_range(source, decl_span.offset(), decl_span.offset() + decl_span.len()))
}
