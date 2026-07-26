//! T22/LSP-25: a reusable protocol-level test harness driving a real
//! `Connection::memory()` server through init -> didOpen -> request/response
//! for hover, completion, goto, references, and rename.
//!
//! The per-feature `lsp_*` helpers this generalizes now live in
//! `tests/common/mod.rs`, shared by the feature suites (P6 T14).
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

// ── T23: shadowing + doc-comment + cross-file protocol tests (LSP-26) ──────
//
// The last task of the whole language-server feature: fixtures asserting
// (1) innermost-binding resolution under shadowing, (2) doc-comment text on
// hover, and (3) cross-file goto + rename — all driven over the T22
// `Harness`, not by calling `DocumentState`/`resolve_at` directly (those
// are already covered unit-style in `integration_test.rs`; this file's job
// is pinning the *protocol* round trip).

/// A minimal on-disk `Piperine.toml` + `src/` scratch project — duplicated
/// from `integration_test.rs`'s own `ScratchProject` (each `tests/*.rs`
/// file compiles as an independent test binary, so there is no shared
/// module to import it from without a larger harness-file reorganization
/// out of this task's scope).
struct ScratchProject(std::path::PathBuf);

impl ScratchProject {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("piperine-lsp-protocol-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Piperine.toml"),
            "[project]\nname = \"scratch_proj\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
        )
        .unwrap();
        Self(dir)
    }

    fn write_src(&self, name: &str, content: &str) -> std::path::PathBuf {
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

fn position_to_byte(source: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut byte = 0usize;
    for l in source.split_inclusive('\n') {
        if line == pos.line {
            return byte + l.chars().take(pos.character as usize).map(|c| c.len_utf8()).sum::<usize>();
        }
        line += 1;
        byte += l.len();
    }
    byte
}

/// LSP-02/LSP-26: a local var shadows an outer param of the same name
/// (`x`) inside one module; hover on the inner (shadowed) `x` must resolve
/// to the local var, not the outer param — the innermost binding in scope.
#[test]
fn protocol_shadowing_fixture_resolves_to_the_innermost_binding() {
    let uri = uri("file:///protocol_shadowing.phdl");
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod Outer (inout p: Electrical, inout n: Electrical) {\n    param x: Real = 1.0;\n    wire x: Electrical;\n}\n";
    let mut h = Harness::start(&uri, src);

    // The `wire x` declaration shadows `param x` within the same module —
    // hover on the `wire x` declaration site must report kind "wire", not
    // "param" (the innermost/most-recently-declared binding for the name
    // `x` in this module's own scope).
    let wire_line = src[..src.find("wire x").unwrap()].matches('\n').count() as u32;
    let wire_col = "    wire ".chars().count() as u32;

    let hover = h.hover(&uri, wire_line, wire_col).expect("expected a hover result on `x`");
    let text = match hover.contents {
        lsp_types::HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup contents"),
    };
    assert!(text.contains("wire"), "expected the innermost (wire) binding, got: {text}");
    assert!(!text.contains("**param**"), "must not resolve to the shadowed param binding: {text}");
    h.shutdown();
}

/// LSP-08/LSP-26: a `///` doc comment above a module declaration renders as
/// Markdown on hover.
#[test]
fn protocol_doc_comment_fixture_renders_on_hover() {
    let uri = uri("file:///protocol_doc.phdl");
    let src = "/// A two-terminal resistor.\nmod Res (inout p: Electrical, inout n: Electrical) {}\ndiscipline Electrical { potential v: Real; flow i: Real; }\n";
    let mut h = Harness::start(&uri, src);

    let line = 1u32;
    let col = "mod ".chars().count() as u32;
    let hover = h.hover(&uri, line, col).expect("expected a hover result on `Res`");
    let text = match hover.contents {
        lsp_types::HoverContents::Markup(m) => m.value,
        _ => panic!("expected markup contents"),
    };
    assert!(text.contains("A two-terminal resistor."), "expected the doc comment text on hover, got: {text}");
    h.shutdown();
}

/// LSP-15/LSP-12/LSP-26: a two-file project fixture — cross-file goto opens
/// the declaring file, and cross-file rename edits every referencing file.
#[test]
fn protocol_cross_file_fixture_goto_and_rename_both_work() {
    let scratch = ScratchProject::new("t23_cross_file");
    let a_src = "pub discipline Electrical { potential v: Real; flow i: Real; }\npub mod A (inout p: Electrical, inout n: Electrical) { param gain: Real = 1.0; }\n";
    let a_path = scratch.write_src("a.phdl", a_src);
    let b_src = "use scratch_proj::a;\nmod B (inout p: Electrical, inout n: Electrical) {\n    inst: A(.p = p, .n = n);\n}\n";
    let b_path = scratch.write_src("b.phdl", b_src);

    let a_uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();
    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();

    let mut h = Harness::start(&b_uri, b_src);

    // Cross-file goto: cursor on `A` inside `inst: A(...)`.
    let goto_line = 2u32;
    let goto_col = "    inst: ".chars().count() as u32;
    let goto_resp = h.goto_definition(&b_uri, goto_line, goto_col).expect("expected a goto-definition result");
    let loc = match goto_resp {
        GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };
    assert_eq!(loc.uri, a_uri, "goto on `A` must open a.phdl, not b.phdl");
    let target_offset = position_to_byte(a_src, loc.range.start);
    let mod_a_start = a_src.find("pub mod A").unwrap();
    assert!(target_offset >= mod_a_start, "goto target must land inside `A`'s own declaration");

    // Cross-file rename: renaming `A` from its use site in b.phdl (the
    // instance's type name) must produce a `WorkspaceEdit` covering both
    // the declaring file (a.phdl) and the referencing file (b.phdl).
    let edit = h.rename(&b_uri, goto_line, goto_col, "AResistor").expect("expected a rename WorkspaceEdit");
    let paths: Vec<Uri> = match (&edit.changes, &edit.document_changes) {
        (Some(changes), _) => changes.keys().cloned().collect(),
        (None, Some(lsp_types::DocumentChanges::Edits(edits))) => {
            edits.iter().map(|e| e.text_document.uri.clone()).collect()
        }
        _ => panic!("expected either `changes` or `document_changes` on the rename edit"),
    };
    assert!(paths.contains(&a_uri), "rename must edit a.phdl (the declaration): {paths:?}");
    assert!(paths.contains(&b_uri), "rename must edit b.phdl (a use site) too: {paths:?}");

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

// ─── Server surface (P6 T14: merged from the former integration_test.rs) ──────
//
// Declared capabilities, the diagnostic error-range helper, and one bespoke
// end-to-end connection — the cases that are about the server itself rather
// than about a language feature.

mod common;
use common::recv_timeout;

#[test]
fn test_server_capabilities_declared() {
    let caps = piperine_lang_server::server::server_capabilities();
    assert!(caps.text_document_sync.is_some());
    assert!(caps.completion_provider.is_some());
    assert!(caps.hover_provider.is_some());
    assert!(caps.definition_provider.is_some());
    assert!(caps.document_symbol_provider.is_some());
}

#[test]
fn test_extract_error_range_lexer_error() {
    let source = "mod foo { wire x: @Electrical; }";
    let error = "Unexpected character '@' at byte 17";
    let range = piperine_lang_server::handlers::diagnostics::extract_error_range(source, error);
    assert!(range.start.line <= 1);
}

#[test]
fn test_extract_error_range_unknown_position() {
    let source = "mod foo;";
    let error = "some random error without position";
    let range = piperine_lang_server::handlers::diagnostics::extract_error_range(source, error);
    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 0);
}

#[test]
fn test_e2e_lsp_server_memory_connection() {
    let (client_conn, server_conn) = Connection::memory();
    
    // Spawn server in a background thread
    std::thread::spawn(move || {
        let mut server = piperine_lang_server::server::LanguageServer::new(server_conn);
        server.run().unwrap();
    });
    
    // Send didOpen notification
    let uri: Uri = "file:///test.phdl".parse().unwrap();
    let did_open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "phdl".to_string(),
            version: 1,
            text: "discipline Electrical { potential v: Real; flow i: Real; }\nmod R (inout p: Electrical, inout n: Electrical) {}".to_string(),
        }
    };
    client_conn.sender.send(Message::Notification(Notification {
        method: lsp_types::notification::DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(did_open_params).unwrap(),
    })).unwrap();

    // Wait for the diagnostics notification, the server elaborates immediately after open.
    let mut received_diagnostics = false;
    for _ in 0..5 {
        if let Ok(msg) = client_conn.receiver.recv_timeout(Duration::from_millis(500)) {
            if let Message::Notification(not) = msg {
                if not.method == lsp_types::notification::PublishDiagnostics::METHOD {
                    received_diagnostics = true;
                    break;
                }
            }
        }
    }
    assert!(received_diagnostics, "Expected PublishDiagnostics notification");
    
    // Test Hover Request
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line: 1, character: 4 }, // "R" in "mod R"
        },
        work_done_progress_params: Default::default(),
    };
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(1),
        method: lsp_types::request::HoverRequest::METHOD.to_string(),
        params: serde_json::to_value(hover_params).unwrap(),
    })).unwrap();
    
    // Wait for hover response
    let msg = recv_timeout(&client_conn.receiver, 1000);
    if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(1));
        assert!(resp.result.is_some());
        let val = resp.result.unwrap();
        let hover: lsp_types::Hover = serde_json::from_value(val).unwrap();
        
        let contents = match hover.contents {
            lsp_types::HoverContents::Markup(m) => m.value,
            _ => panic!("Expected markup"),
        };
        assert!(contents.contains("module"));
    } else {
        panic!("Expected response");
    }

    // Shut down server
    client_conn.sender.send(Message::Request(Request {
        id: RequestId::from(99),
        method: "shutdown".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
    let msg = recv_timeout(&client_conn.receiver, 500);
    if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(99));
    }
    client_conn.sender.send(Message::Notification(Notification {
        method: "exit".to_string(),
        params: serde_json::Value::Null,
    })).unwrap();
}
