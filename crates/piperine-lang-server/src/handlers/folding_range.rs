use lsp_server::{Connection, Request};
use lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<FoldingRangeParams>(connection) else { return };

    let mut result: Vec<FoldingRange> = vec![];

    // T37 (MD-45) regression, documented: VAMS has no `{`/`}` block
    // delimiters at all (LRM Annex A pairs keywords instead —
    // `module`/`endmodule`, `discipline`/`enddiscipline`, `analog
    // begin`/`end`, …) — the old brace-token-stack approach has no VAMS
    // lexical anchor to match on. Folding now uses the parsed AST's own
    // top-level item spans directly (module/discipline/nature) instead
    // of a token-stream brace stack; nested folds (a module's own
    // behaviors/ports/etc.) are not covered — future work, same scope
    // limit `selection_range.rs`'s T37 retarget already documents.
    if let Some(doc) = state.documents.get(&params.text_document.uri)
        && let Some(ast) = &doc.ast
    {
        for item in &ast.items {
            use piperine_vams::ast::Item;
            let span = match item {
                Item::Module(m) => m.span,
                Item::Discipline(d) => d.span,
                Item::Nature(n) => n.span,
                _ => continue,
            };
            let s = crate::symbol_index::vams_span_to_miette(span);
            let start_pos = crate::text_pos::byte_to_position(&doc.source, s.offset());
            let end_pos = crate::text_pos::byte_to_position(&doc.source, s.offset() + s.len());
            if start_pos.line < end_pos.line {
                result.push(FoldingRange {
                    start_line: start_pos.line,
                    start_character: Some(start_pos.character),
                    end_line: end_pos.line,
                    end_character: Some(end_pos.character),
                    kind: Some(FoldingRangeKind::Region),
                    collapsed_text: None,
                });
            }
        }
    }

    connection.respond(id, result);
}
