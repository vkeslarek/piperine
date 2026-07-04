use lsp_server::{Connection, Request};
use lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use piperine_lang::parse::lexer::{Lexer, Tok};

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<FoldingRangeParams>(connection) else { return };

    let mut result: Vec<FoldingRange> = vec![];

    if let Some(doc) = state.documents.get(&params.text_document.uri) {
        // Brace pairs from the token stream, so braces inside strings and
        // comments never open a fold.
        if let Ok(tokens) = Lexer::new(&doc.source).tokenize_all() {
            let mut stack = vec![];
            for lexed in tokens {
                match lexed.tok {
                    Tok::LBrace => stack.push(lexed.start),
                    Tok::RBrace => {
                        if let Some(start) = stack.pop() {
                            let start_pos = crate::text_pos::byte_to_position(&doc.source, start);
                            let end_pos = crate::text_pos::byte_to_position(&doc.source, lexed.start);
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
                    _ => {}
                }
            }
        }
    }

    connection.respond(id, result);
}
