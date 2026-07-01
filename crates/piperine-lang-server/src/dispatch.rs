//! Message dispatch: routes incoming LSP requests and notifications
//! to the appropriate handler functions.

use lsp_server::{Connection, Request};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification as _,
};
use lsp_types::request::{
    Completion, DocumentSymbolRequest, GotoDefinition, HoverRequest, Request as _,
};

use crate::handlers;
use crate::server::LanguageServer;

impl LanguageServer {
    /// Dispatch an incoming request to the appropriate handler.
    pub fn handle_request(&mut self, req: Request, connection: &Connection) {
        match req.method.as_str() {
            HoverRequest::METHOD => {
                handlers::hover::handle(&mut self.state, req, connection);
            }
            Completion::METHOD => {
                handlers::completion::handle(&mut self.state, req, connection);
            }
            GotoDefinition::METHOD => {
                handlers::goto_def::handle(&mut self.state, req, connection);
            }
            DocumentSymbolRequest::METHOD => {
                handlers::symbols::handle(&mut self.state, req, connection);
            }
            _ => {
                eprintln!("unhandled request: {}", req.method);
            }
        }
    }

    /// Dispatch an incoming notification to the appropriate handler.
    pub fn handle_notification(&mut self, not: lsp_server::Notification, connection: &Connection) {
        match not.method.as_str() {
            DidOpenTextDocument::METHOD => {
                handlers::diagnostics::handle_open(&mut self.state, not, connection);
            }
            DidChangeTextDocument::METHOD => {
                handlers::diagnostics::handle_change(&mut self.state, not, connection);
            }
            DidSaveTextDocument::METHOD => {
                handlers::diagnostics::handle_save(&mut self.state, not, connection);
            }
            DidCloseTextDocument::METHOD => {
                handlers::diagnostics::handle_close(&mut self.state, not);
            }
            // Initialized notification and trace — no action needed.
            "initialized" | "$/setTrace" => {}
            _ => {
                eprintln!("unhandled notification: {}", not.method);
            }
        }
    }
}
