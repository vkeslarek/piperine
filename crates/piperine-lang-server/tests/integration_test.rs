use lsp_server::{Connection, Message, Request, RequestId, Notification};
use lsp_types::{
    Position, Uri, HoverParams, TextDocumentPositionParams, TextDocumentIdentifier,
    DidOpenTextDocumentParams, TextDocumentItem,
};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use std::time::Duration;
use crossbeam_channel::Receiver;

// ── Tests ───────────────────────────────────────────────────────────────────

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




// ── End-to-end LSP Tests ──────────────────────────────────────────────────────────

fn recv_timeout(rx: &Receiver<Message>, timeout_ms: u64) -> Message {
    rx.recv_timeout(Duration::from_millis(timeout_ms)).expect("did not receive message in time")
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

