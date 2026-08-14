use lsp_server::{Connection, Request};
use lsp_types::{Location, SymbolInformation, SymbolKind, WorkspaceSymbolParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use crate::text_pos::byte_range;

#[allow(deprecated)] // SymbolInformation::deprecated is required by the struct literal
pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<WorkspaceSymbolParams>(connection) else { return };

    let mut result: Vec<SymbolInformation> = vec![];
    let query = params.query.to_lowercase();

    for (uri, doc) in &state.documents {
        if let Some(ast) = &doc.ast {
            for item in &ast.items {
                use piperine_lang::ast::Item;
                // MD-45: VAMS's own `Item` (Annex A) has no `BehaviorDecl`/
                // `FnDecl`/`BundleDecl`/`EnumDecl`/`ConstDecl` counterpart —
                // those were PHDL-only value-layer declarations, gone at
                // T21. `Paramset`/`Connectrules`/`Config`/`Primitive` are
                // real LRM top-level items with no outline entry authored
                // yet (future work, not this task's scope); only
                // `Module`/`Discipline` are wired here.
                let (name, kind, span) = match item {
                    Item::Module(m) => (&m.name, SymbolKind::MODULE, m.span),
                    Item::Discipline(d) => (&d.name, SymbolKind::INTERFACE, d.span),
                    _ => continue,
                };
                let span = crate::symbol_index::vams_span_to_miette(span);

                if query.is_empty() || name.to_lowercase().contains(&query) {
                    result.push(SymbolInformation {
                        name: name.clone(),
                        kind,
                        tags: None,
                        deprecated: None,
                        location: Location::new(
                            uri.clone(),
                            byte_range(&doc.source, span.offset(), span.offset() + span.len()),
                        ),
                        container_name: None,
                    });
                }
            }
        }
    }

    connection.respond(id, result);
}
