//! Fixtures and helpers shared by the LSP suites: the in-memory server
//! connection helpers, `DocumentState` analysis, position arithmetic, and the
//! scratch-project builder.
//!
//! Not a test target — a `tests/common/` module the suites include.

#![allow(dead_code, unused_imports)]

pub use lsp_server::{Connection, Message, Request, RequestId, Notification};
pub use lsp_types::{
    Position, Uri, HoverParams, TextDocumentPositionParams, TextDocumentIdentifier,
    DidOpenTextDocumentParams, TextDocumentItem, GotoDefinitionParams, GotoDefinitionResponse,
    Location, WorkspaceEdit, PrepareRenameResponse, DocumentChanges, PublishDiagnosticsParams,
};
pub use lsp_types::notification::Notification as _;
pub use lsp_types::request::Request as _;
pub use std::time::Duration;
pub use std::collections::HashMap;
pub use crossbeam_channel::Receiver;










// ── End-to-end LSP Tests ──────────────────────────────────────────────────────────

pub fn recv_timeout(rx: &Receiver<Message>, timeout_ms: u64) -> Message {
    rx.recv_timeout(Duration::from_millis(timeout_ms)).expect("did not receive message in time")
}

/// Wait for the `Message::Response` matching `id`, draining and discarding
/// any `Notification`s received first — T15's per-file diagnostic fan-out
/// (LSP-16) can publish more than one `PublishDiagnostics` notification per
/// analysis (one per project file), so a response is no longer guaranteed
/// to be the very next message after a request.
pub fn recv_response(rx: &Receiver<Message>, id: RequestId, timeout_ms: u64) -> lsp_server::Response {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let msg = rx.recv_timeout(remaining).expect("did not receive the expected response in time");
        match msg {
            Message::Response(resp) if resp.id == id => return resp,
            Message::Response(other) => panic!("expected response id {id:?}, got {:?}", other.id),
            Message::Notification(_) => continue,
            Message::Request(req) => panic!("unexpected request from server: {req:?}"),
        }
    }
}


// ── declared-language-surface T14/T15: symbol_index resolves extern decls ──

pub use piperine_lang_server::state::DocumentState;
pub use piperine_lang_server::symbol_index::SymbolKind;

pub fn analyzed(source: &str) -> DocumentState {
    let mut doc = DocumentState::new(source.to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    doc
}








// ── T4: hover renders `doc` as Markdown (LSP-08/09) ─────────────────────────

/// Drives a real `Connection::memory()` round trip (init-free, matching the
/// existing `test_e2e_lsp_server_memory_connection` pattern): open `source`,
/// wait for the server's post-open diagnostics, request hover at
/// `(line, character)`, and return the response's Markdown contents.
pub fn lsp_hover_markdown(source: &str, line: u32, character: u32) -> String {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let uri: Uri = "file:///hover_doc_test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::HoverRequest::METHOD.to_string(),
        params: serde_json::to_value(hover_params).unwrap(),
    })).unwrap();

    let msg = recv_timeout(&client_conn.receiver, 1000);
    let contents = if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(1));
        let val = resp.result.expect("hover response must have a result");
        let hover: lsp_types::Hover = serde_json::from_value(val).expect("hover result must deserialize");
        match hover.contents {
            lsp_types::HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup contents"),
        }
    } else {
        panic!("expected a hover response");
    };

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    contents
}





// ── BUG-2 (LSB-04..06): hover shows `///` docs for extern declarations ──────



// ── T6: `resolve_at` cursor-context + shadowing (LSP-01/02) ─────────────────



// ── T7: goto-definition rides the resolved binding (LSP-04) ─────────────────

/// Drives a `Connection::memory()` round trip and returns the goto-definition
/// response for `(line, character)` in `source`.
pub fn lsp_goto_definition(source: &str, line: u32, character: u32) -> GotoDefinitionResponse {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let uri: Uri = "file:///goto_def_test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let goto_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::GotoDefinition::METHOD.to_string(),
        params: serde_json::to_value(goto_params).unwrap(),
    })).unwrap();

    let resp = recv_response(&client_conn.receiver, RequestId::from(1), 1000);
    let val = resp.result.expect("goto response must have a result");
    let response = serde_json::from_value(val).expect("goto result must deserialize");

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}

pub fn goto_target_offset(resp: &GotoDefinitionResponse, source: &str) -> usize {
    let loc = match resp {
        GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };
    position_to_byte(source, loc.range.start)
}

// Mirrors `piperine_lang_server::text_pos::position_to_byte` for the test's
// own use (that module is crate-private to the server crate).
pub fn position_to_byte(source: &str, pos: Position) -> usize {
    let mut byte = 0usize;
    for (i, line) in source.split('\n').enumerate() {
        if i as u32 == pos.line {
            let col_bytes: usize = line.chars().take(pos.character as usize).map(|c| c.len_utf8()).sum();
            return byte + col_bytes;
        }
        byte += line.len() + 1;
    }
    byte
}


// ── BUG-1 (LSB-01..03): extern goto lands on the real declaring file ────────



// ── T8: occurrence engine from binding (LSP-10/13 base) ─────────────────────



// ── T9: references handler rides binding occurrences (LSP-10) ──────────────

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/references` response for `(line, character)` in `source`.
pub fn lsp_references(source: &str, line: u32, character: u32) -> Vec<Location> {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let uri: Uri = "file:///references_test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let references_params = lsp_types::ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: lsp_types::ReferenceContext { include_declaration: true },
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::References::METHOD.to_string(),
        params: serde_json::to_value(references_params).unwrap(),
    })).unwrap();

    let msg = recv_timeout(&client_conn.receiver, 1000);
    let response = if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(1));
        let val = resp.result.expect("references response must have a result");
        serde_json::from_value(val).expect("references result must deserialize")
    } else {
        panic!("expected a references response");
    };

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}


// ── T10: rename handler rides binding occurrences (LSP-11) ─────────────────

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/rename` response for `(line, character)` -> `new_name`.
pub fn lsp_rename(source: &str, line: u32, character: u32, new_name: &str) -> Option<WorkspaceEdit> {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let uri: Uri = "file:///rename_test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let rename_params = lsp_types::RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        new_name: new_name.to_string(),
        work_done_progress_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::Rename::METHOD.to_string(),
        params: serde_json::to_value(rename_params).unwrap(),
    })).unwrap();

    let resp = recv_response(&client_conn.receiver, RequestId::from(1), 1000);
    let response = resp.result.and_then(|val| serde_json::from_value(val).ok());

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/prepareRename` response for `(line, character)`.
pub fn lsp_prepare_rename(source: &str, line: u32, character: u32) -> Option<PrepareRenameResponse> {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let uri: Uri = "file:///prepare_rename_test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        position: Position { line, character },
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::PrepareRenameRequest::METHOD.to_string(),
        params: serde_json::to_value(params).unwrap(),
    })).unwrap();

    let msg = recv_timeout(&client_conn.receiver, 1000);
    let response = if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(1));
        resp.result.and_then(|val| serde_json::from_value(val).ok())
    } else {
        panic!("expected a prepareRename response");
    };

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}



// ── T11: document-highlight rides binding occurrences (LSP-13) ─────────────

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/documentHighlight` response for `(line, character)`.
pub fn lsp_document_highlight(source: &str, line: u32, character: u32) -> Vec<lsp_types::DocumentHighlight> {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let uri: Uri = "file:///highlight_test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let params = lsp_types::DocumentHighlightParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::DocumentHighlightRequest::METHOD.to_string(),
        params: serde_json::to_value(params).unwrap(),
    })).unwrap();

    let msg = recv_timeout(&client_conn.receiver, 1000);
    let response = if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(1));
        let val = resp.result.expect("documentHighlight response must have a result");
        serde_json::from_value(val).unwrap_or_default()
    } else {
        panic!("expected a documentHighlight response");
    };

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}


// ── T12: `ProjectUnit` — multi-file index (LSP-14) ──────────────────────────

/// A scratch on-disk project (`Piperine.toml` + `src/`), removed on drop —
/// mirrors `piperine-project`'s own `ScratchDir` test helper
/// (`crates/piperine-project/src/source_map.rs`).
pub struct ScratchProject(pub std::path::PathBuf);

impl ScratchProject {
    pub fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir()
            .join(format!("piperine-lsp-project-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Piperine.toml"),
            // `authors`/`edition` are required (no `#[serde(default)]`,
            // `piperine-project::PiperineToml`) — without them
            // `PiperineToml::load` fails silently and `project_source_map`
            // never registers the project's own namespace, breaking any
            // same-project `use scratch_proj::…;` (T13/LSP-15).
            "[project]\nname = \"scratch_proj\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
        )
        .unwrap();
        Self(dir)
    }

    pub fn write_src(&self, name: &str, content: &str) -> std::path::PathBuf {
        let path = self.0.join("src").join(name);
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}




// ── T13: cross-file goto (LSP-15) ────────────────────────────────────────

/// Drives a `Connection::memory()` round trip against a specific `uri`
/// (unlike `lsp_goto_definition`, which always opens a hardcoded
/// single-file uri) — needed for cross-file scenarios where a second
/// project file must already exist on disk before the request lands.
pub fn lsp_goto_definition_at(uri: &Uri, source: &str, line: u32, character: u32) -> GotoDefinitionResponse {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let goto_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::GotoDefinition::METHOD.to_string(),
        params: serde_json::to_value(goto_params).unwrap(),
    })).unwrap();

    let resp = recv_response(&client_conn.receiver, RequestId::from(1), 1000);
    let val = resp.result.expect("goto response must have a result");
    let response = serde_json::from_value(val).expect("goto result must deserialize");

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}


// ── T14: cross-file rename (LSP-12) ──────────────────────────────────────

/// Drives a `Connection::memory()` rename round trip against a specific
/// `uri` (the cross-file counterpart of `lsp_rename`, which always opens a
/// hardcoded single-file uri).
pub fn lsp_rename_at(uri: &Uri, source: &str, line: u32, character: u32, new_name: &str) -> Option<WorkspaceEdit> {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    for _ in 0..5 {
        if let Ok(Message::Notification(not)) = client_conn.receiver.recv_timeout(Duration::from_millis(500))
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                break;
            }
    }

    let rename_params = lsp_types::RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        new_name: new_name.to_string(),
        work_done_progress_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::Rename::METHOD.to_string(),
        params: serde_json::to_value(rename_params).unwrap(),
    })).unwrap();

    let resp = recv_response(&client_conn.receiver, RequestId::from(1), 1000);
    let response = resp.result.and_then(|val| serde_json::from_value(val).ok());

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    recv_timeout(&client_conn.receiver, 500);
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    response
}


// ── T15: per-file diagnostic fan-out + single-file fallback (LSP-16/17) ──

/// Open `uri` (already written to disk as part of a project) and collect
/// every `PublishDiagnostics` notification received within `timeout_ms`,
/// keyed by the URI they were published against — T15's fan-out publishes
/// one notification per project file, not just the opened document's.
pub fn lsp_collect_diagnostics(uri: &Uri, source: &str, timeout_ms: u64) -> HashMap<String, Vec<lsp_types::Diagnostic>> {
    let (client_conn, server_conn) = Connection::memory();

    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });

    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: source.to_string(),
        },
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    let mut by_uri = HashMap::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    while let Ok(msg) = client_conn.receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
        if let Message::Notification(not) = msg
            && not.method == lsp_types::notification::PublishDiagnostics::METHOD
            && let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(not.params) {
                by_uri.insert(params.uri.to_string(), params.diagnostics);
            }
        if std::time::Instant::now() >= deadline {
            break;
        }
    }

    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    let _ = client_conn.receiver.recv_timeout(Duration::from_millis(500));
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();

    by_uri
}



// ── T17: diagnostic severity + structured codes (LSP-19) ──────────────────



// ── T18: `@schema` completion (LSP-20) ──────────────────────────────────────

pub use piperine_lang_server::handlers::completion::completions_at;




// ── T19: attribute-argument validation (LSP-21) ─────────────────────────────




// ── T20: hover -> schema fields, goto -> `@attribute` decl (LSP-22/23) ──────

pub use piperine_lang_server::text_pos::byte_to_position;



// ── T21: attribute outline entries (LSP-24) ─────────────────────────────────

pub use piperine_lang_server::handlers::symbols::extract_symbols;
