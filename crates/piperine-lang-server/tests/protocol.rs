//! T22/LSP-25: a reusable protocol-level test harness driving a real
//! `Connection::memory()` server through init -> didOpen -> request/response
//! for hover, completion, goto, references, and rename.
//!
//! `integration_test.rs` already proved the pattern works (one bespoke
//! `lsp_*` helper function per feature, each opening its own memory
//! connection). This harness generalizes that into a single reusable
//! `Harness` so a protocol-round-trip test is a few lines instead of a new
//! 60-line helper function per feature (T23 builds directly on it).
//!
//! "init": the server (`server.rs`) does not gate any request on an
//! `initialize` handshake — `didOpen` is handled unconditionally the moment
//! the connection exists (matching `integration_test.rs`'s own "init-free"
//! `test_e2e_lsp_server_memory_connection`). The harness's `start()` is the
//! literal init step: spawning the server thread and wiring the memory
//! connection is everything "initialize" would otherwise need to unblock.

use crossbeam_channel::Receiver;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use lsp_types::{
    DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    Location, Position, ReferenceContext, ReferenceParams, RenameParams, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    WorkspaceEdit,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

/// A running `piperine-lang-server`, reachable over an in-memory LSP
/// connection, plus the auto-incrementing request-id counter every `request`
/// call consumes.
pub struct Harness {
    client: Connection,
    next_id: i32,
}

impl Harness {
    /// Spins up a fresh server over a memory connection ("init") and opens
    /// `source` at `uri` ("didOpen"), draining the post-open diagnostics
    /// notification(s) before returning — every subsequent `request` call
    /// sees a fully analyzed document.
    pub fn start(uri: &Uri, source: &str) -> Self {
        let (client, server) = Connection::memory();
        std::thread::spawn(move || {
            let mut server = piperine_lang_server::server::LanguageServer::new(server);
            server.run().unwrap();
        });
        let mut harness = Self { client, next_id: 1 };
        harness.open(uri, source);
        harness
    }

    /// `didOpen` a document and wait for its diagnostics notification
    /// (T15's per-file fan-out can publish more than one notification per
    /// analysis; draining until the *first* one is observed is sufficient
    /// to know the document was analyzed).
    pub fn open(&mut self, uri: &Uri, source: &str) {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "phdl".to_string(),
                version: 1,
                text: source.to_string(),
            },
        };
        self.notify(lsp_types::notification::DidOpenTextDocument::METHOD, params);
        self.wait_for_notification(lsp_types::notification::PublishDiagnostics::METHOD, 800);
    }

    /// Send a notification (no response expected).
    pub fn notify<P: Serialize>(&self, method: &str, params: P) {
        self.client
            .sender
            .send(Message::Notification(Notification {
                method: method.to_string(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
    }

    /// Send a request and deserialize its response result — the core
    /// request/response round trip every typed helper below rides.
    /// `None` when the server answered with a `null`/absent result (e.g.
    /// hover/goto on a position with nothing to show).
    pub fn request<P: Serialize, R: DeserializeOwned>(&mut self, method: &str, params: P) -> Option<R> {
        let id = RequestId::from(self.next_id);
        self.next_id += 1;
        self.client
            .sender
            .send(Message::Request(Request {
                id: id.clone(),
                method: method.to_string(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
        let resp = self.recv_response(id, 1500);
        resp.result.and_then(|v| serde_json::from_value(v).ok())
    }

    fn recv_response(&self, id: RequestId, timeout_ms: u64) -> lsp_server::Response {
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let msg = self
                .client
                .receiver
                .recv_timeout(remaining)
                .expect("did not receive the expected response in time");
            match msg {
                Message::Response(resp) if resp.id == id => return resp,
                Message::Response(other) => panic!("expected response id {id:?}, got {:?}", other.id),
                Message::Notification(_) => continue,
                Message::Request(req) => panic!("unexpected request from server: {req:?}"),
            }
        }
    }

    /// Drain notifications until one matching `method` is observed, or the
    /// timeout elapses (diagnostics may already have arrived before this is
    /// called, so a short-circuit miss is not itself a failure — callers
    /// that need the diagnostics payload should use `receiver()` directly).
    fn wait_for_notification(&self, method: &str, timeout_ms: u64) {
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match self.client.receiver.recv_timeout(remaining) {
                Ok(Message::Notification(not)) if not.method == method => return,
                Ok(_) => continue,
                Err(_) => return,
            }
        }
    }

    /// The raw receiver, for tests that need to inspect notifications
    /// directly (e.g. collecting per-file diagnostics across a project).
    pub fn receiver(&self) -> &Receiver<Message> {
        &self.client.receiver
    }

    // ── Typed convenience wrappers ───────────────────────────────────────

    pub fn hover(&mut self, uri: &Uri, line: u32, character: u32) -> Option<Hover> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.request(lsp_types::request::HoverRequest::METHOD, params)
    }

    pub fn completion(&mut self, uri: &Uri, line: u32, character: u32) -> Vec<lsp_types::CompletionItem> {
        let params = lsp_types::CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
            context: None,
        };
        let resp: Option<lsp_types::CompletionResponse> =
            self.request(lsp_types::request::Completion::METHOD, params);
        match resp {
            Some(lsp_types::CompletionResponse::Array(items)) => items,
            Some(lsp_types::CompletionResponse::List(list)) => list.items,
            None => Vec::new(),
        }
    }

    pub fn goto_definition(&mut self, uri: &Uri, line: u32, character: u32) -> Option<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        };
        self.request(lsp_types::request::GotoDefinition::METHOD, params)
    }

    pub fn references(&mut self, uri: &Uri, line: u32, character: u32) -> Vec<Location> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext { include_declaration: true },
        };
        self.request(lsp_types::request::References::METHOD, params).unwrap_or_default()
    }

    pub fn rename(&mut self, uri: &Uri, line: u32, character: u32, new_name: &str) -> Option<WorkspaceEdit> {
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            new_name: new_name.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.request(lsp_types::request::Rename::METHOD, params)
    }

    /// Graceful shutdown/exit — mirrors `integration_test.rs`'s existing
    /// pattern; not calling it is harmless (the server thread simply exits
    /// when the connection drops), but this keeps the sequence explicit.
    pub fn shutdown(self) {
        let id = RequestId::from(self.next_id);
        let _ = self.client.sender.send(Message::Request(Request {
            id,
            method: "shutdown".to_string(),
            params: serde_json::Value::Null,
        }));
        let _ = self.client.receiver.recv_timeout(Duration::from_millis(500));
        let _ = self.client.sender.send(Message::Notification(Notification {
            method: "exit".to_string(),
            params: serde_json::Value::Null,
        }));
    }
}

fn uri(s: &str) -> Uri {
    s.parse().unwrap()
}

// ── T22: harness self-tests — one round trip per feature ───────────────────

#[test]
fn harness_drives_hover_round_trip() {
    let uri = uri("file:///protocol_hover.phdl");
    let mut h = Harness::start(&uri, "mod R (inout p: Electrical, inout n: Electrical) {}\ndiscipline Electrical { potential v: Real; flow i: Real; }\n");
    let hover = h.hover(&uri, 0, 4).expect("expected a hover result on `R`");
    let text = match hover.contents {
        lsp_types::HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup contents"),
    };
    assert!(text.contains("module"), "expected module hover, got: {text}");
    h.shutdown();
}

#[test]
fn harness_drives_completion_round_trip() {
    let uri = uri("file:///protocol_completion.phdl");
    let mut h = Harness::start(&uri, "mod Top() {}\n");
    // Simulate a mid-edit `@rf` completion trigger without re-triggering
    // analysis (matches `integration_test.rs`'s own stale-but-valid pattern
    // for schema completion) by editing the buffer directly via didChange.
    let change = lsp_types::DidChangeTextDocumentParams {
        text_document: lsp_types::VersionedTextDocumentIdentifier { uri: uri.clone(), version: 2 },
        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "mod Top() {}\n@rf".to_string(),
        }],
    };
    h.notify(lsp_types::notification::DidChangeTextDocument::METHOD, change);
    let items = h.completion(&uri, 1, 3);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"rfport"), "expected `rfport` completion, got: {labels:?}");
    h.shutdown();
}

#[test]
fn harness_drives_goto_definition_round_trip() {
    let uri = uri("file:///protocol_goto.phdl");
    let src = "mod R (inout p: Electrical, inout n: Electrical) {}\ndiscipline Electrical { potential v: Real; flow i: Real; }\nmod Top() { r1: R(); }\n";
    let mut h = Harness::start(&uri, src);
    // "R" inside `r1: R();` on line 2.
    let line_start = src.lines().nth(2).unwrap();
    let col = line_start.find("R()").unwrap() as u32;
    let resp = h.goto_definition(&uri, 2, col).expect("expected a goto-definition result");
    let location = match resp {
        GotoDefinitionResponse::Scalar(loc) => loc,
        GotoDefinitionResponse::Array(mut locs) => locs.remove(0),
        GotoDefinitionResponse::Link(_) => panic!("expected Scalar/Array, got Link"),
    };
    assert_eq!(location.uri, uri);
    h.shutdown();
}

#[test]
fn harness_drives_references_round_trip() {
    let uri = uri("file:///protocol_references.phdl");
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod Top (inout p: Electrical, inout n: Electrical) { param power: Real = 1.0; }\n";
    let mut h = Harness::start(&uri, src);
    let line = src[..src.find("param power").unwrap()].matches('\n').count() as u32;
    let col = "mod Top (inout p: Electrical, inout n: Electrical) { param ".chars().count() as u32;
    let refs = h.references(&uri, line, col);
    assert!(!refs.is_empty(), "expected at least the declaration site as a reference to `power`");
    h.shutdown();
}

#[test]
fn harness_drives_rename_round_trip() {
    let uri = uri("file:///protocol_rename.phdl");
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod Top (inout p: Electrical, inout n: Electrical) { param power: Real = 1.0; }\n";
    let mut h = Harness::start(&uri, src);
    let line = src[..src.find("param power").unwrap()].matches('\n').count() as u32;
    let col = "mod Top (inout p: Electrical, inout n: Electrical) { param ".chars().count() as u32;
    let edit = h.rename(&uri, line, col, "pwr").expect("expected a rename WorkspaceEdit");
    assert!(edit.changes.is_some() || edit.document_changes.is_some(), "rename must produce an edit");
    h.shutdown();
}
