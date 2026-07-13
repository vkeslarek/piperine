use lsp_server::{Connection, Request};
use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use piperine_lang::parse::lexer::{Lexer, Tok};

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<InlayHintParams>(connection) else { return };

    let mut result: Vec<InlayHint> = vec![];

    if let Some(doc) = state.documents.get(&params.text_document.uri) {
        let source = &doc.source;
        let tokens = Lexer::new(source).tokenize_all().unwrap_or_default();

        for (i, lexed) in tokens.iter().enumerate() {
            match &lexed.tok {
                // `var x = …` — inferred type after the variable name.
                Tok::Ident(kw) if kw == "var" => {
                    if let Some(name_tok) = tokens.get(i + 1)
                        && matches!(&name_tok.tok, Tok::Ident(_))
                            && let Some(ty) = doc
                                .resolve_at(name_tok.start)
                                .and_then(|res| res.type_info)
                            {
                                result.push(InlayHint {
                                    position: crate::text_pos::byte_to_position(source, name_tok.end),
                                    label: InlayHintLabel::String(format!(": {ty}")),
                                    kind: Some(InlayHintKind::TYPE),
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: Some(true),
                                    data: None,
                                });
                            }
                }
                // SI-suffixed literals (`1k`, `2.2u`, `10M`) — the lexer
                // already folded the suffix into the value; show it when the
                // literal text ends with one.
                Tok::Real(value) => {
                    let text = &source[lexed.start..lexed.end];
                    if text.ends_with(['T', 'G', 'M', 'k', 'm', 'u', 'n', 'p', 'f', 'a']) {
                        result.push(InlayHint {
                            position: crate::text_pos::byte_to_position(source, lexed.end),
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
                _ => {}
            }
        }
    }

    connection.respond(id, result);
}
