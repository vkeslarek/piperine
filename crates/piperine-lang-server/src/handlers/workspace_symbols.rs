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
                use piperine_lang::parse::ast::Item;
                let (name, kind, span) = match item {
                    Item::ModuleDeclaration(m) => (&m.name, SymbolKind::MODULE, m.span),
                    Item::BehaviorDecl(b) => (&b.name, SymbolKind::CLASS, b.span),
                    Item::FnDecl(f) => (&f.sig.name, SymbolKind::FUNCTION, f.span),
                    Item::BenchDecl(b) => (&b.name, SymbolKind::METHOD, b.span),
                    Item::DisciplineDecl(d) => (&d.name, SymbolKind::INTERFACE, d.span),
                    Item::BundleDecl(b) => (&b.name, SymbolKind::STRUCT, b.span),
                    Item::EnumDecl(e) => (&e.name, SymbolKind::ENUM, e.span),
                    Item::ConstDecl(c) => (&c.name, SymbolKind::CONSTANT, c.span),
                    _ => continue,
                };

                if (query.is_empty() || name.to_lowercase().contains(&query))
                    && let Some(span) = span {
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
