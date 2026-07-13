use lsp_server::{Connection, Request};
use lsp_types::{SemanticToken, SemanticTokens, SemanticTokensParams};

use super::{ConnectionExt, RequestExt};

use crate::state::ServerState;
use crate::text_pos::byte_to_position;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<SemanticTokensParams>(connection) else { return };

    let uri = params.text_document.uri;
    let mut tokens = Vec::new();

    if let Some(doc) = state.documents.get(&uri) {
        let mut lexer = piperine_lang::parse::lexer::Lexer::new(&doc.source);
        if let Ok(lexed_tokens) = lexer.tokenize_all() {
            let mut prev_line = 0;
            let mut prev_start = 0;

            for lexed in lexed_tokens {
                let token_type_and_mod = match &lexed.tok {
                    piperine_lang::parse::lexer::Tok::SysCall(_) => Some((5, 0)), // MACRO
                    piperine_lang::parse::lexer::Tok::Ident(word) => {
                        let mut kind = None;
                        if let Some(design) = &doc.design {
                            if design.module(word).is_some() {
                                kind = Some((0, 0)); // NAMESPACE
                            } else if design.discipline(word).is_some() || design.bundle(word).is_some() || design.enum_(word).is_some() {
                                kind = Some((7, 0)); // TYPE
                            } else if design.function(word).is_some() {
                                kind = Some((4, 0)); // FUNCTION
                            } else if design.enum_value_map().contains_key(word) {
                                kind = Some((6, 0)); // ENUM_MEMBER
                            } else {
                                // Global search
                                'search: for module in design.modules() {
                                    if module.ports.iter().any(|p| &p.name == word) {
                                        kind = Some((2, 1)); // VARIABLE + READONLY
                                        break 'search;
                                    }
                                    if module.params.iter().any(|p| &p.name == word) {
                                        kind = Some((1, 0)); // PARAMETER
                                        break 'search;
                                    }
                                    if module.wires.iter().any(|w| &w.name == word) {
                                        kind = Some((2, 0)); // VARIABLE
                                        break 'search;
                                    }
                                    if module.instances.iter().any(|i| i.name() == word) {
                                        kind = Some((2, 0)); // VARIABLE (instance)
                                        break 'search;
                                    }
                                }
                                if kind.is_none() {
                                    for (_, bundle) in design.bundles() {
                                        if bundle.fields.iter().any(|f| &f.name == word) {
                                            kind = Some((3, 0)); // PROPERTY
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        kind
                    }
                    _ => None,
                };

                if let Some((token_type, token_modifiers)) = token_type_and_mod {
                    let pos = byte_to_position(&doc.source, lexed.start);
                    let end_pos = byte_to_position(&doc.source, lexed.end);
                    let line = pos.line;
                    let start = pos.character;
                    let length = end_pos.character - pos.character;

                    let delta_line = line - prev_line;
                    let delta_start = if delta_line == 0 {
                        start - prev_start
                    } else {
                        start
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length,
                        token_type,
                        token_modifiers_bitset: token_modifiers,
                    });

                    prev_line = line;
                    prev_start = start;
                }
            }
        }
    }

    connection.respond(id, SemanticTokens { result_id: None, data: tokens });
}
