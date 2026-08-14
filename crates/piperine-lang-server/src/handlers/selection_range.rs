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
            
            // T37 (MD-45) regression, documented: VAMS's `Item` enum
            // (Annex A) only has `Module`/`Discipline`/`Nature`/
            // `Paramset`/`Connectrules`/`Config`/`Primitive` — the PHDL-
            // only `BehaviorDecl`/`FnDecl`/`BundleDecl`/`EnumDecl`/
            // `ConstDecl`/`CapabilityDecl`/`ImplDecl` variants are gone
            // (T21). The deeper "which statement inside the module body
            // am I in" expansion is also gone: PHDL's uniform `Module::
            // body: Vec<Stmt>` (with a `Stmt::span()` method) has no VAMS
            // counterpart — a module's `items: Vec<ModuleItem>` has no
            // single per-item span accessor to walk the same way. Only
            // the item-level (module/discipline) span selection survives.
            if let Some(ast) = &doc.ast {
                for item in &ast.items {
                    use piperine_lang::ast::Item;
                    let span = match item {
                        Item::Module(m) => Some(m.span),
                        Item::Discipline(d) => Some(d.span),
                        _ => None,
                    };

                    if let Some(s) = span {
                        let s = crate::symbol_index::vams_span_to_miette(s);
                        if offset >= s.offset() && offset <= s.offset() + s.len() {
                            parent = Some(Box::new(SelectionRange {
                                range: crate::text_pos::byte_range(&doc.source, s.offset(), s.offset() + s.len()),
                                parent,
                            }));
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
