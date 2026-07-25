use lsp_server::{Connection, Message, Request, RequestId, Notification};
use lsp_types::{
    Position, Uri, HoverParams, TextDocumentPositionParams, TextDocumentIdentifier,
    DidOpenTextDocumentParams, TextDocumentItem, GotoDefinitionParams, GotoDefinitionResponse,
    Location,
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
///
/// Uses the real, globally-declared `sin` (`headers/math.phdl`, T19/
/// DLS-18, auto-loaded into every compilation unit's prelude) rather than
/// a local re-declaration: a local `extern fn sin` would now collide with
/// the real one (two structurally identical candidates — an ambiguous
/// overload, correctly rejected), and any *other* locally-declared name
/// would correctly fail as DLS-05's "extern with no native binding" case,
/// since every `MATH_FNS`-backed name now already has its own extern
/// declaration. This is arguably the more faithful proof of P3's actual
/// acceptance bar ("`sin(x)` in a stdlib header … returns a Location
/// inside the relevant extern declaration") — the `decl_span` now points
/// into the real `headers/math.phdl` text (embedded identically via
/// `include_str!` here and in `piperine-lang`'s own prelude loading, so
/// the expected offset can be computed precisely without reading the file
/// from disk at test time).
#[test]
fn extern_fn_use_site_resolves_to_its_decl_span() {
    let src = "mod Top() {}\ndigital Top { var y: Real = sin(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let call_site = src.rfind("sin(1.0)").expect("call site must be present");
    let resolution = doc.resolve_at(call_site).expect("sin(...) use site must resolve");

    assert_eq!(resolution.kind, SymbolKind::Function);
    let decl_span = resolution.decl_span.expect("extern fn must carry a decl_span");

    let math_header = include_str!("../../piperine-lang/headers/math.phdl");
    let decl_start = math_header.find("extern fn sin(").expect("declaration must be present in headers/math.phdl");
    assert_eq!(
        decl_span.offset(), decl_start,
        "decl_span must point at headers/math.phdl's `extern fn sin` declaration, not the call site"
    );
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
    // A distinct name (not one of `headers/operators.phdl`'s real DLS-20
    // declarations, e.g. `ddt`) — reusing a real name here would register
    // a second, ambiguous overload candidate now that `ddt` has a genuine
    // global declaration (DLS-20/T22), rather than testing this fixture's
    // own local decl_span in isolation.
    let src = "extern operator my_op(x: Real) -> Real;\nmod Top() {}\ndigital Top { var y: Real = my_op(1.0); }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.rfind("my_op(1.0)").expect("call site must be present");
    let resolution = doc.resolve_at(use_site).expect("`my_op` use site must resolve");

    assert_eq!(resolution.kind, SymbolKind::Operator);
    let decl_span = resolution.decl_span.expect("extern operator must carry a decl_span");
    let decl_start = src.find("extern operator my_op").expect("declaration must be present");
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

// ── T4: hover renders `doc` as Markdown (LSP-08/09) ─────────────────────────

/// Drives a real `Connection::memory()` round trip (init-free, matching the
/// existing `test_e2e_lsp_server_memory_connection` pattern): open `source`,
/// wait for the server's post-open diagnostics, request hover at
/// `(line, character)`, and return the response's Markdown contents.
fn lsp_hover_markdown(source: &str, line: u32, character: u32) -> String {
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

/// LSP-08: hovering a `///`-documented declaration prepends the doc text as
/// Markdown above the `**kind** \`name\`` line.
#[test]
fn hover_on_documented_module_renders_doc_as_markdown() {
    let source = "discipline Electrical { potential v: Real; flow i: Real; }\n/// A two-terminal resistor.\nmod R (inout p: Electrical, inout n: Electrical) {}";
    // line 2 is `mod R (...)`; character 4 lands on `R`.
    let contents = lsp_hover_markdown(source, 2, 4);
    assert!(
        contents.contains("A two-terminal resistor."),
        "hover contents must include the doc text, got: {contents}"
    );
    assert!(contents.contains("**module** `R`"), "hover must still render the kind/name line, got: {contents}");
    // The doc text must precede the kind line (LSP-08: "prepend ... above").
    let doc_pos = contents.find("A two-terminal resistor.").unwrap();
    let kind_pos = contents.find("**module** `R`").unwrap();
    assert!(doc_pos < kind_pos, "doc text must be prepended above the kind/type line");
}

/// LSP-09: a declaration with no `///` run renders exactly as before —
/// hover is unchanged, no stray doc section appears.
#[test]
fn hover_on_undocumented_module_is_unchanged() {
    let source = "discipline Electrical { potential v: Real; flow i: Real; }\nmod R (inout p: Electrical, inout n: Electrical) {}";
    let contents = lsp_hover_markdown(source, 1, 4);
    assert!(contents.contains("**module** `R`"), "hover must render the kind/name line, got: {contents}");
    // No regression: the contents are exactly the kind line (no leading doc
    // paragraph / blank-line-separated section before it).
    assert_eq!(contents, "**module** `R`");
}

// ── T6: `resolve_at` cursor-context + shadowing (LSP-01/02) ─────────────────

/// LSP-01/02 independent test: two modules each declare a `param` of the
/// same name (`x`). Cursor context must resolve each module's own `x` to
/// *that* module's decl_span — never the first module in POM iteration
/// order (the bug the old word-based global loop had).
#[test]
fn resolve_at_uses_cursor_context_not_global_first_match() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    param x: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param x: Real = 2.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    // The `x` inside A's `param x: Real = 1.0;`.
    let a_body_start = src.find("mod A").unwrap();
    let a_x_offset = src[a_body_start..].find("param x").unwrap() + a_body_start + "param ".len();
    let a_resolution = doc.resolve_at(a_x_offset).expect("A's x resolves");
    assert_eq!(a_resolution.kind, SymbolKind::Param);
    let a_x_decl = a_resolution.decl_span.expect("A's x has a decl_span");

    // The `x` inside B's `param x: Real = 2.0;`.
    let b_body_start = src.find("mod B").unwrap();
    let b_x_offset = src[b_body_start..].find("param x").unwrap() + b_body_start + "param ".len();
    let b_resolution = doc.resolve_at(b_x_offset).expect("B's x resolves");
    assert_eq!(b_resolution.kind, SymbolKind::Param);
    let b_x_decl = b_resolution.decl_span.expect("B's x has a decl_span");

    assert_ne!(
        a_x_decl.offset(), b_x_decl.offset(),
        "A's x and B's x are distinct declarations — cursor context must not collapse them to the same (first-match) decl_span"
    );
    // Each decl_span must land inside the module it was declared in — not
    // A's decl_span leaking into B's cursor position or vice versa.
    assert!(a_x_decl.offset() < b_body_start, "A's x decl_span must be inside module A");
    assert!(b_x_decl.offset() >= b_body_start, "B's x decl_span must be inside module B, not A's");
}

/// LSP-02: given a declaration (`param`) and an unrelated same-named
/// declaration in *another* module, resolving inside the module that owns
/// the local one must return that module's own binding — the "innermost"
/// half of shadowing (the outer/other module's same-named decl never wins
/// just because it appears earlier in iteration order).
#[test]
fn resolve_at_shadowed_name_resolves_to_innermost_not_first_declared() {
    // `Outer` is declared textually first; `Inner`'s own `gain` must still
    // win when the cursor is inside `Inner`.
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod Outer (inout p: Electrical, inout n: Electrical) {\n\
    param gain: Real = 1.0;\n\
}\n\
mod Inner (inout p: Electrical, inout n: Electrical) {\n\
    param gain: Real = 9.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let inner_start = src.find("mod Inner").unwrap();
    let gain_offset = src[inner_start..].find("param gain").unwrap() + inner_start + "param ".len();
    let resolution = doc.resolve_at(gain_offset).expect("Inner's gain resolves");
    let decl_span = resolution.decl_span.expect("gain has a decl_span");

    assert!(
        decl_span.offset() >= inner_start,
        "cursor inside Inner must resolve to Inner's own `gain`, not Outer's (first-declared)"
    );
}

// ── T7: goto-definition rides the resolved binding (LSP-04) ─────────────────

/// Drives a `Connection::memory()` round trip and returns the goto-definition
/// response for `(line, character)` in `source`.
fn lsp_goto_definition(source: &str, line: u32, character: u32) -> GotoDefinitionResponse {
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

    let msg = recv_timeout(&client_conn.receiver, 1000);
    let response = if let Message::Response(resp) = msg {
        assert_eq!(resp.id, RequestId::from(1));
        let val = resp.result.expect("goto response must have a result");
        serde_json::from_value(val).expect("goto result must deserialize")
    } else {
        panic!("expected a goto-definition response");
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

fn goto_target_offset(resp: &GotoDefinitionResponse, source: &str) -> usize {
    let loc = match resp {
        GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };
    position_to_byte(source, loc.range.start)
}

// Mirrors `piperine_lang_server::text_pos::position_to_byte` for the test's
// own use (that module is crate-private to the server crate).
fn position_to_byte(source: &str, pos: Position) -> usize {
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

/// LSP-04 independent test: two modules each declare a `param` of the same
/// name. goto-definition on the *use* site inside `Inner`'s own declaration
/// (i.e. the cursor position that resolve_at now resolves via cursor
/// context, T6) must land inside `Inner`, never inside the textually-first
/// `Outer` — proving goto rides the resolved binding, not a same-named
/// match anywhere in the file.
#[test]
fn goto_definition_on_shadowed_name_lands_on_the_correct_declaration() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod Outer (inout p: Electrical, inout n: Electrical) {\n\
    param gain: Real = 1.0;\n\
}\n\
mod Inner (inout p: Electrical, inout n: Electrical) {\n\
    param gain: Real = 9.0;\n\
}\n";

    let inner_line = src[..src.find("mod Inner").unwrap()].matches('\n').count() as u32 + 1;
    // `    param gain: Real = 9.0;` — character offset of `gain`.
    let character = "    param ".chars().count() as u32;

    let response = lsp_goto_definition(src, inner_line, character);
    let target_offset = goto_target_offset(&response, src);

    let inner_start = src.find("mod Inner").unwrap();
    assert!(
        target_offset >= inner_start,
        "goto-definition on Inner's own `gain` must land inside Inner (offset {target_offset}), not Outer (which starts before offset {inner_start})"
    );
}

// ── T8: occurrence engine from binding (LSP-10/13 base) ─────────────────────

/// LSP-10/13 base: resolving a declared binding's own position returns
/// exactly the index's recorded uses for that binding — per T5's
/// SPEC_DEVIATION, `ResolutionIndex.use_spans` today holds only the
/// binding's own declaration span (a reflexive use), so this is a
/// one-element list, not an invented richer occurrence set.
#[test]
fn occurrences_at_returns_exactly_the_indexed_uses() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 1.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let power_offset = src.find("param power").unwrap() + "param ".len();
    let occurrences = doc.occurrences_at(power_offset);

    assert_eq!(
        occurrences.len(),
        1,
        "the shipped ResolutionIndex only tracks the reflexive decl-site use; occurrences_at must not invent more, got: {occurrences:?}"
    );
    let (start, end) = occurrences[0];
    assert!(
        power_offset >= start && power_offset < end,
        "the sole occurrence must cover the binding's own declaration site"
    );
}

/// LSP-10/13 base edge case: occurrences must never include a same-spelled
/// binding in another scope, nor a `// name` comment mention — both would
/// be false positives under the old `word_occurrences` text scan.
#[test]
fn occurrences_at_excludes_other_scope_and_comment_matches() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    // power is computed elsewhere\n\
    param power: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 2.0;\n\
}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let a_start = src.find("mod A").unwrap();
    let b_start = src.find("mod B").unwrap();
    let a_power_offset = src[a_start..].find("param power").unwrap() + a_start + "param ".len();
    let occurrences = doc.occurrences_at(a_power_offset);

    for (start, _end) in &occurrences {
        assert!(
            *start < b_start,
            "occurrences of A's power must never include B's declaration (offset {start} >= {b_start})"
        );
    }
    let comment_offset = src.find("power is computed").unwrap();
    for (start, end) in &occurrences {
        assert!(
            !(comment_offset >= *start && comment_offset < *end || *start == comment_offset),
            "occurrences must never point inside the `//` comment"
        );
    }
}

// ── T9: references handler rides binding occurrences (LSP-10) ──────────────

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/references` response for `(line, character)` in `source`.
fn lsp_references(source: &str, line: u32, character: u32) -> Vec<Location> {
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

/// LSP-10: references on a declared binding return only that binding's
/// recorded occurrences — a `// power` comment mention and an unrelated
/// module's own `power` declaration must never appear.
#[test]
fn references_excludes_comment_and_other_scope_matches() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    // power is computed elsewhere\n\
    param power: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 2.0;\n\
}\n";

    let a_line = src[..src.find("param power").unwrap()].matches('\n').count() as u32;
    let character = "    param ".chars().count() as u32;

    let locations = lsp_references(src, a_line, character);

    let comment_line = src[..src.find("power is computed").unwrap()].matches('\n').count() as u32;
    let b_line = src[..src.find("mod B").unwrap()].matches('\n').count() as u32;

    assert!(!locations.is_empty(), "references must return at least the declaration site");
    for loc in &locations {
        assert_ne!(loc.range.start.line, comment_line, "a `// power` comment must never appear in references");
        assert!(loc.range.start.line < b_line, "module B's own `power` must never appear in module A's references");
    }
}

/// LSP-10/13 base edge case: a cursor on a non-symbol (a numeric literal)
/// yields no occurrences at all.
#[test]
fn occurrences_at_on_non_symbol_is_empty() {
    let src = "mod Top() {}\ndigital Top { var y: Real = 1.0; }";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let literal_offset = src.rfind("1.0").unwrap();
    assert!(doc.occurrences_at(literal_offset).is_empty());
}


