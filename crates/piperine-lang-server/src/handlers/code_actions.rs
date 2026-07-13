use lsp_server::{Connection, Request};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionParams, CodeActionResponse, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, TextDocumentEdit, TextEdit, WorkspaceEdit,
};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<CodeActionParams>(connection) else { return };

    let mut result: CodeActionResponse = vec![];

    if state.documents.contains_key(&params.text_document.uri) {
        for diag in &params.context.diagnostics {
            if (diag.message.contains("unresolved") || diag.message.contains("not found"))
                && let Some(name) = quoted_name(&diag.message) {
                    let edit = TextEdit {
                        range: lsp_types::Range::default(),
                        new_text: format!("\nmod {name} {{\n}}\n"),
                    };
                    let action = CodeAction {
                        title: format!("Create module `{name}`"),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: None,
                            document_changes: Some(DocumentChanges::Edits(vec![
                                TextDocumentEdit {
                                    text_document: OptionalVersionedTextDocumentIdentifier {
                                        uri: params.text_document.uri.clone(),
                                        version: None,
                                    },
                                    edits: vec![OneOf::Left(edit)],
                                },
                            ])),
                            change_annotations: None,
                        }),
                        ..Default::default()
                    };
                    result.push(lsp_types::CodeActionOrCommand::CodeAction(action));
                }
        }
    }

    connection.respond(id, result);
}

/// The identifier a diagnostic message names, taken from its
/// backtick/quote-delimited fragment (`unresolved name \`Foo\``).
fn quoted_name(message: &str) -> Option<&str> {
    let quote = |c: char| c == '`' || c == '\'' || c == '"';
    let start = message.find(quote)? + 1;
    let len = message[start..].find(quote)?;
    let name = &message[start..start + len];
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then_some(name)
}
