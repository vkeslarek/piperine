use lsp_server::{Connection, Request};
use lsp_types::{SelectionRange, SelectionRangeParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<SelectionRangeParams>(connection) else { return };

    let mut result: Vec<SelectionRange> = vec![];

    if let Some(doc) = state.documents.get(&params.text_document.uri) {
        for pos in params.positions {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            let mut parent: Option<Box<SelectionRange>> = None;
            
            // Outermost: whole file
            parent = Some(Box::new(SelectionRange {
                range: crate::text_pos::byte_range(&doc.source, 0, doc.source.len()),
                parent,
            }));
            
            if let Some(ast) = &doc.ast {
                for item in &ast.items {
                    use piperine_lang::parse::ast::Item;
                    let span = match item {
                        Item::ModuleDeclaration(m) => m.span,
                        Item::BehaviorDecl(b) => b.span,
                        Item::FnDecl(f) => f.span,
                        Item::DisciplineDecl(d) => d.span,
                        Item::BundleDecl(b) => b.span,
                        Item::EnumDecl(e) => e.span,
                        Item::ConstDecl(c) => c.span,
                        Item::CapabilityDecl(c) => c.span,
                        Item::ImplDecl(i) => i.span,
                        _ => None,
                    };
                    
                    if let Some(s) = span
                        && offset >= s.offset() && offset <= s.offset() + s.len() {
                            parent = Some(Box::new(SelectionRange {
                                range: crate::text_pos::byte_range(&doc.source, s.offset(), s.offset() + s.len()),
                                parent,
                            }));
                            
                            if let Item::ModuleDeclaration(m) = item {
                                for stmt in &m.body {
                                    if let Some(ss) = stmt.span()
                                        && offset >= ss.offset() && offset <= ss.offset() + ss.len() {
                                            parent = Some(Box::new(SelectionRange {
                                                range: crate::text_pos::byte_range(&doc.source, ss.offset(), ss.offset() + ss.len()),
                                                parent,
                                            }));
                                        }
                                }
                            }
                        }
                }
            }
            
            let mut start = offset;
            while start > 0 && (doc.source.as_bytes()[start - 1].is_ascii_alphanumeric() || doc.source.as_bytes()[start - 1] == b'_') {
                start -= 1;
            }
            let mut end = offset;
            while end < doc.source.len() && (doc.source.as_bytes()[end].is_ascii_alphanumeric() || doc.source.as_bytes()[end] == b'_') {
                end += 1;
            }
            if start < end {
                parent = Some(Box::new(SelectionRange {
                    range: crate::text_pos::byte_range(&doc.source, start, end),
                    parent,
                }));
            }
            
            if let Some(p) = parent {
                result.push(*p);
            } else {
                result.push(SelectionRange { range: lsp_types::Range::default(), parent: None });
            }
        }
    }

    connection.respond(id, result);
}
