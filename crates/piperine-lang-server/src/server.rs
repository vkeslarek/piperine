//! Server state and capability declaration.

use lsp_server::Connection;
use lsp_types::{
    CompletionOptions, HoverProviderCapability, OneOf, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};

use crate::state::ServerState;

pub struct LanguageServer {
    pub state: ServerState,
}

impl LanguageServer {
    pub fn new(connection: &Connection) -> Self {
        Self { state: ServerState::new(connection) }
    }
}

/// Server capabilities advertised to the client during initialization.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".into(), ":".into(), " ".into()]),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}
