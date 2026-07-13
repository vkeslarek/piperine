//! Message dispatch: routes incoming LSP requests and notifications
//! to the appropriate handler functions.

use lsp_server::{Request, Notification};
use lsp_types::notification::{
    DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification as _,
};
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, Completion, DocumentHighlightRequest, DocumentSymbolRequest,
    FoldingRangeRequest, GotoDefinition, HoverRequest, InlayHintRequest, PrepareRenameRequest,
    References, Rename, Request as _, SelectionRangeRequest, SemanticTokensFullRequest,
    SignatureHelpRequest, WorkspaceSymbolRequest,
};

use crate::handlers;
use crate::state::{ServerState, DocumentState};
use crossbeam_channel::Sender;

/// Dispatch an incoming request to the appropriate handler.
pub fn handle_request(state: &mut ServerState, req: Request, conn_sender: &Sender<lsp_server::Message>) {
    // create a dummy connection so we don't have to change all handler signatures
    let connection = lsp_server::Connection {
        sender: conn_sender.clone(),
        receiver: crossbeam_channel::never(),
    };

    match req.method.as_str() {
        HoverRequest::METHOD => {
            handlers::hover::handle(state, req, &connection);
        }
        Completion::METHOD => {
            handlers::completion::handle(state, req, &connection);
        }
        GotoDefinition::METHOD => {
            handlers::goto_def::handle(state, req, &connection);
        }
        DocumentSymbolRequest::METHOD => {
            handlers::symbols::handle(state, req, &connection);
        }
        lsp_types::request::Formatting::METHOD => {
            handlers::formatting::handle(state, req, &connection);
        }
        CodeLensRequest::METHOD => {
            handlers::code_lens::handle(state, req, &connection);
        }
        SemanticTokensFullRequest::METHOD => {
            handlers::semantic_tokens::handle(state, req, &connection);
        }
        References::METHOD => {
            handlers::references::handle(state, req, &connection);
        }
        Rename::METHOD => {
            handlers::rename::handle_rename(state, req, &connection);
        }
        PrepareRenameRequest::METHOD => {
            handlers::rename::handle_prepare_rename(state, req, &connection);
        }
        SignatureHelpRequest::METHOD => {
            handlers::signature_help::handle(state, req, &connection);
        }
        CodeActionRequest::METHOD => {
            handlers::code_actions::handle(state, req, &connection);
        }
        InlayHintRequest::METHOD => {
            handlers::inlay_hints::handle(state, req, &connection);
        }
        FoldingRangeRequest::METHOD => {
            handlers::folding_range::handle(state, req, &connection);
        }
        SelectionRangeRequest::METHOD => {
            handlers::selection_range::handle(state, req, &connection);
        }
        WorkspaceSymbolRequest::METHOD => {
            handlers::workspace_symbols::handle(state, req, &connection);
        }
        DocumentHighlightRequest::METHOD => {
            handlers::document_highlight::handle(state, req, &connection);
        }
        "$/cancelRequest" => {
            // Ignore cancel requests for now
        }
        _ => {
            handlers::diagnostics::log_message(
                &connection, 
                lsp_types::MessageType::WARNING, 
                format!("unhandled request: {}", req.method)
            );
        }
    }
}

/// Dispatch an incoming notification to the appropriate handler.
pub fn handle_notification(state: &mut ServerState, not: Notification, conn_sender: &Sender<lsp_server::Message>) {
    let connection = lsp_server::Connection {
        sender: conn_sender.clone(),
        receiver: crossbeam_channel::never(),
    };

    match not.method.as_str() {
        DidOpenTextDocument::METHOD => {
            if let Ok(params) = serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(not.params) {
                let uri = params.text_document.uri;
                let source = params.text_document.text;
                let version = params.text_document.version;

                state.documents.insert(uri.clone(), DocumentState::new(source, version));
                // Analysis is triggered via Elaborate message
            }
        }
        DidSaveTextDocument::METHOD => {
            if let Ok(params) = serde_json::from_value::<lsp_types::DidSaveTextDocumentParams>(not.params) {
                let uri = params.text_document.uri;
                if let Some(text) = params.text
                    && let Some(doc) = state.documents.get_mut(&uri) {
                        doc.source = text;
                    }
            }
        }
        DidCloseTextDocument::METHOD => {
            if let Ok(params) = serde_json::from_value::<lsp_types::DidCloseTextDocumentParams>(not.params) {
                state.documents.remove(&params.text_document.uri);
            }
        }
        "workspace/didChangeWatchedFiles" => {
            // Re-elaborate all documents when a watched file changes
            let uris: Vec<_> = state.documents.keys().cloned().collect();
            for uri in uris {
                if let Some(doc) = state.documents.get_mut(&uri) {
                    let source_map = crate::project::ProjectContext::discover(&uri).source_map();
                    doc.analyze(&source_map);
                    crate::handlers::diagnostics::publish_diagnostics(state, &uri, &connection);
                }
            }
        }
        "workspace/didChangeConfiguration" => {
            // No-op
        }
        // Initialized notification and trace — no action needed.
        "initialized" | "$/setTrace" | "$/cancelRequest" => {}
        _ => {
            handlers::diagnostics::log_message(
                &connection, 
                lsp_types::MessageType::WARNING, 
                format!("unhandled notification: {}", not.method)
            );
        }
    }
}
