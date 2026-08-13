use lsp_server::{Connection, Request};
use lsp_types::{InlayHint, InlayHintLabel, InlayHintParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use piperine_vams::lexer::{Lexer, NumberLit, Tok};

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<InlayHintParams>(connection) else { return };

    let mut result: Vec<InlayHint> = vec![];

    if let Some(doc) = state.documents.get(&params.text_document.uri) {
        let source = &doc.source;
        let tokens = Lexer::new(source).tokenize().unwrap_or_default();

        // T37 (MD-45) regression, documented: the PHDL-only `var x = …`
        // inferred-type hint is gone — VAMS has no local-variable-with-
        // inferred-type declaration form (module-level `integer`/`real`
        // declarations, LRM Annex A.2.1.3, always carry an explicit
        // type keyword; there is nothing to infer).
        for lexed in &tokens {
            // SI-suffixed literals (`1k`, `2.2u`, `10M`) — the lexer
            // already folded the suffix into the value; show it when the
            // literal text ends with one.
            if let Tok::Number(NumberLit::Real(value)) = &lexed.tok {
                let text = &source[lexed.span.start..lexed.span.end];
                if text.ends_with(['T', 'G', 'M', 'k', 'm', 'u', 'n', 'p', 'f', 'a']) {
                    result.push(InlayHint {
                        position: crate::text_pos::byte_to_position(source, lexed.span.end),
                        label: InlayHintLabel::String(format!(" = {value}")),
                        kind: None,
                        text_edits: None,
                        tooltip: None,
                        padding_left: Some(true),
                        padding_right: None,
                        data: None,
                    });
                }
            }
        }
    }

    connection.respond(id, result);
}
