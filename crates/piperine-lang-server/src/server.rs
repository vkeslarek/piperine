//! Server state and capability declaration.

use std::time::{Duration, Instant};
use std::collections::HashMap;

use crossbeam_channel::{select, Receiver, Sender};
use lsp_server::{Connection, Message, Request, Notification};
use lsp_types::{
    CompletionOptions, HoverProviderCapability, OneOf, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, WorkspaceServerCapabilities,
    WorkspaceFoldersServerCapabilities,
};
use lsp_types::notification::Notification as _;

use crate::state::ServerState;

pub enum WorkerMsg {
    Request(Request),
    Notification(Notification),
    UpdateSource { uri: lsp_types::Uri, source: String, version: i32 },
    Elaborate { uri: lsp_types::Uri },
}

pub struct LanguageServer {
    pub connection: Connection,
    pub worker_tx: Sender<WorkerMsg>,
    pub pending_analysis: HashMap<lsp_types::Uri, Instant>,
}

impl LanguageServer {
    pub fn new(connection: Connection) -> Self {
        let (worker_tx, rx) = crossbeam_channel::unbounded::<WorkerMsg>();
        let conn_sender = connection.sender.clone();

        std::thread::spawn(move || {
            let mut state = ServerState::new();
            for msg in rx {
                match msg {
                    WorkerMsg::Request(req) => {
                        crate::dispatch::handle_request(&mut state, req, &conn_sender);
                    }
                    WorkerMsg::Notification(not) => {
                        crate::dispatch::handle_notification(&mut state, not, &conn_sender);
                    }
                    WorkerMsg::UpdateSource { uri, source, version } => {
                        if let Some(doc) = state.documents.get_mut(&uri) {
                            doc.source = source;
                            doc.version = version;
                        } else {
                            state.documents.insert(uri, crate::state::DocumentState {
                                source,
                                version,
                                design: None,
                                errors: Vec::new(),
                            });
                        }
                    }
                    WorkerMsg::Elaborate { uri } => {
                        if let Some(doc) = state.documents.get_mut(&uri) {
                            let (design, errors) = crate::state::parse_and_collect_errors(&doc.source);
                            doc.design = design;
                            doc.errors = errors;
                            let dummy_conn = lsp_server::Connection {
                                sender: conn_sender.clone(),
                                receiver: crossbeam_channel::never(),
                            };
                            crate::handlers::diagnostics::publish_diagnostics(&state, &uri, &dummy_conn);
                        }
                    }
                }
            }
        });

        Self {
            connection,
            worker_tx,
            pending_analysis: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let now = Instant::now();
            let timeout = self.pending_analysis.values()
                .min()
                .map(|deadline| {
                    if *deadline > now {
                        *deadline - now
                    } else {
                        Duration::from_millis(0)
                    }
                });

            let timer_rx = timeout.map(crossbeam_channel::after);
            let default_rx = crossbeam_channel::never();
            let timer = timer_rx.as_ref().unwrap_or(&default_rx);

            select! {
                recv(&self.connection.receiver) -> msg => {
                    match msg {
                        Ok(Message::Request(req)) => {
                            if self.connection.handle_shutdown(&req)? {
                                return Ok(());
                            }
                            let _ = self.worker_tx.send(WorkerMsg::Request(req));
                        }
                        Ok(Message::Notification(not)) => {
                            if not.method == lsp_types::notification::DidChangeTextDocument::METHOD {
                                if let Ok(params) = serde_json::from_value::<lsp_types::DidChangeTextDocumentParams>(not.params.clone()) {
                                    let uri = params.text_document.uri;
                                    let version = params.text_document.version;
                                    if let Some(change) = params.content_changes.into_iter().last() {
                                        let _ = self.worker_tx.send(WorkerMsg::UpdateSource { 
                                            uri: uri.clone(), 
                                            source: change.text, 
                                            version 
                                        });
                                        self.pending_analysis.insert(uri, Instant::now() + Duration::from_millis(250));
                                    }
                                }
                            } else if not.method == lsp_types::notification::DidOpenTextDocument::METHOD {
                                if let Ok(params) = serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(not.params.clone()) {
                                    let uri = params.text_document.uri;
                                    let _ = self.worker_tx.send(WorkerMsg::Notification(not));
                                    self.pending_analysis.insert(uri, Instant::now() + Duration::from_millis(10));
                                }
                            } else if not.method == "workspace/didChangeWatchedFiles" {
                                // Handled in worker thread, but we can also trigger full analysis if needed.
                                let _ = self.worker_tx.send(WorkerMsg::Notification(not));
                            } else {
                                let _ = self.worker_tx.send(WorkerMsg::Notification(not));
                            }
                        }
                        Ok(Message::Response(_)) => {}
                        Err(_) => break, // client disconnected
                    }
                }
                recv(timer) -> _ => {
                    let now = Instant::now();
                    let mut uris_to_dispatch = Vec::new();
                    for (uri, deadline) in &self.pending_analysis {
                        if now >= *deadline {
                            uris_to_dispatch.push(uri.clone());
                        }
                    }
                    for uri in uris_to_dispatch {
                        self.pending_analysis.remove(&uri);
                        let _ = self.worker_tx.send(WorkerMsg::Elaborate { uri });
                    }
                }
            }
        }
        Ok(())
    }
}

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
        workspace: Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
