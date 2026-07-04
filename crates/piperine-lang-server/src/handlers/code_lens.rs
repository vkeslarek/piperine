use lsp_server::{Connection, Request};
use lsp_types::{CodeLens, CodeLensParams, Command, Range};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use crate::text_pos::byte_to_position;
use piperine_lang::parse::ast::Item;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<CodeLensParams>(connection) else { return };

    let uri = params.text_document.uri;
    let mut lenses = Vec::new();

    if let Some(doc) = state.documents.get(&uri) {
        let uri_str = uri.as_str().to_string();

        if let Some(ast) = &doc.ast {
            for item in &ast.items {
                if let Item::BenchDecl(bench) = item {
                    if let Some(span) = bench.span {
                        let range = Range {
                            start: byte_to_position(&doc.source, span.offset()),
                            end: byte_to_position(&doc.source, span.offset() + span.len()),
                        };
                        // Pass the file URI so the extension runs `piperine test <file>`
                        lenses.push(CodeLens {
                            range,
                            command: Some(Command {
                                title: "▶ Run bench".to_string(),
                                command: "piperine.test".to_string(),
                                arguments: Some(vec![serde_json::Value::String(
                                    uri_str.clone(),
                                )]),
                            }),
                            data: None,
                        });
                    }

                    for f in &bench.fns {
                        if f.sig.params.is_empty() {
                            if let Some(span) = f.span {
                                let range = Range {
                                    start: byte_to_position(&doc.source, span.offset()),
                                    end: byte_to_position(
                                        &doc.source,
                                        span.offset() + span.len(),
                                    ),
                                };
                                lenses.push(CodeLens {
                                    range,
                                    command: Some(Command {
                                        title: format!("▶ Run {}", f.sig.name),
                                        command: "piperine.test".to_string(),
                                        arguments: Some(vec![serde_json::Value::String(
                                            uri_str.clone(),
                                        )]),
                                    }),
                                    data: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    connection.respond(id, lenses);
}
