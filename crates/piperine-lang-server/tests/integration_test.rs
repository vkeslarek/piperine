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

// ── declared-language-surface T14/T15: symbol_index resolves extern decls ──

use piperine_lang_server::state::DocumentState;
use piperine_lang_server::symbol_index::SymbolKind;

fn analyzed(source: &str) -> DocumentState {
    let mut doc = DocumentState::new(source.to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    doc
}

/// DLS-15: an `extern fn` use site resolves to a `Resolution` pointing at
/// the `extern fn` declaration's own `decl_span` — the same
/// `Resolution.decl_span` shape `goto_def.rs` already forwards for every
/// ordinary declaration (module/param/wire/…), no special-casing needed.
#[test]
fn extern_fn_use_site_resolves_to_its_decl_span() {
    let src = "extern fn sin(x: Real) -> Real;\nmod Top() {}\ndigital Top { var y: Real = sin(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let call_site = src.rfind("sin(1.0)").expect("call site must be present");
    let resolution = doc.resolve_at(call_site).expect("sin(...) use site must resolve");

    assert_eq!(resolution.kind, SymbolKind::Function);
    let decl_span = resolution.decl_span.expect("extern fn must carry a decl_span");
    let decl_start = src.find("extern fn sin").expect("declaration must be present");
    assert_eq!(decl_span.offset(), decl_start, "decl_span must point at the extern fn declaration, not the call site");
}

/// DLS-15: an `extern type` use site (the type name itself) resolves to the
/// `extern type` declaration's `decl_span`.
#[test]
fn extern_type_use_site_resolves_to_its_decl_span() {
    let src = "extern type Widget;\nextern impl Widget { fn make(x: Real) -> Widget; }\nmod Top() {}\ndigital Top { Widget::make(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.rfind("Widget::make").expect("call site must be present");
    let resolution = doc.resolve_at(use_site).expect("`Widget` in `Widget::make(...)` must resolve");

    assert_eq!(resolution.kind, SymbolKind::Type);
    let decl_span = resolution.decl_span.expect("extern type must carry a decl_span");
    let decl_start = src.find("extern type Widget").expect("declaration must be present");
    assert_eq!(decl_span.offset(), decl_start);
}

/// DLS-15: a `Type::method(...)` use site (the method name) resolves to
/// the `extern impl` method's own `decl_span`, distinct from the block's
/// own span.
#[test]
fn extern_impl_method_use_site_resolves_to_its_own_decl_span() {
    let src = "extern type Widget;\nextern impl Widget { fn make(x: Real) -> Widget; }\nmod Top() {}\ndigital Top { Widget::make(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.rfind("make(1.0)").expect("call site must be present");
    let resolution = doc.resolve_at(use_site).expect("`make` in `Widget::make(...)` must resolve");

    assert_eq!(resolution.kind, SymbolKind::Function);
    let decl_span = resolution.decl_span.expect("extern impl method must carry a decl_span");
    let method_decl_start = src.find("fn make").expect("method declaration must be present");
    assert_eq!(decl_span.offset(), method_decl_start);
}

/// DLS-15: an `extern operator` use site resolves to its own `decl_span`.
#[test]
fn extern_operator_use_site_resolves_to_its_decl_span() {
    let src = "extern operator ddt(x: Real) -> Real;\nmod Top() {}\ndigital Top { var y: Real = ddt(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.rfind("ddt(1.0)").expect("call site must be present");
    let resolution = doc.resolve_at(use_site).expect("`ddt` use site must resolve");

    assert_eq!(resolution.kind, SymbolKind::Operator);
    let decl_span = resolution.decl_span.expect("extern operator must carry a decl_span");
    let decl_start = src.find("extern operator ddt").expect("declaration must be present");
    assert_eq!(decl_span.offset(), decl_start);
}

/// DLS-15: an `extern attribute` schema name's use site (`@name(...)`)
/// resolves to the schema declaration's own `decl_span`.
#[test]
fn extern_attribute_use_site_resolves_to_its_decl_span() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nextern attribute widget_meta { rating: Real }\nmod Top ( inout p : Electrical ) { @widget_meta(rating = 4.5) wire w : Electrical; }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.rfind("widget_meta(rating").expect("use site must be present");
    let resolution = doc.resolve_at(use_site).expect("`@widget_meta` use site must resolve");

    assert_eq!(resolution.kind, SymbolKind::AttrSchema);
    let decl_span = resolution.decl_span.expect("extern attribute must carry a decl_span");
    let decl_start = src.find("extern attribute widget_meta").expect("declaration must be present");
    assert_eq!(decl_span.offset(), decl_start);
}

/// DLS-16: a name with no declaration anywhere (the P1-AC4 error case)
/// still returns `None` — today's behavior for undeclared names is
/// unaffected by T14 wiring the registries into `resolve_at`. Note the
/// source itself fails to elaborate (an undeclared call is a hard
/// elaboration error per T11), so `design`/`ctx` on the `DocumentState`
/// are `None` too — `resolve_at` correctly returns `None` rather than
/// panicking or fabricating a location.
#[test]
fn undeclared_name_use_site_still_returns_no_location() {
    let src = "mod Top() {}\ndigital Top { NoSuchType::no_such_method(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a source with an undeclared call must fail to elaborate");

    let use_site = src.rfind("no_such_method").expect("call site must be present");
    let resolution = doc.resolve_at(use_site);
    assert!(resolution.is_none(), "an undeclared name must not resolve to any location");
}

/// DLS-16 (companion case): even when the *rest* of the document elaborates
/// fine, a position that isn't an identifier at all resolves to nothing —
/// T14's new registry arms don't turn every byte offset into a resolvable
/// symbol, only real identifier use sites.
#[test]
fn non_identifier_position_returns_no_location() {
    let src = "mod Top() {}\ndigital Top { var y: Real = 1.0; }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let bogus_offset = src.rfind("1.0").expect("literal must be present");
    let resolution = doc.resolve_at(bogus_offset);
    assert!(resolution.is_none(), "a numeric literal is not a resolvable symbol");
}


