use lsp_server::{Connection, Message, Request, RequestId, Notification};
use lsp_types::{
    Position, Uri, HoverParams, TextDocumentPositionParams, TextDocumentIdentifier,
    DidOpenTextDocumentParams, TextDocumentItem, GotoDefinitionParams, GotoDefinitionResponse,
    Location, WorkspaceEdit, PrepareRenameResponse, DocumentChanges, PublishDiagnosticsParams,
};
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use std::time::Duration;
use std::collections::HashMap;
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

/// Wait for the `Message::Response` matching `id`, draining and discarding
/// any `Notification`s received first — T15's per-file diagnostic fan-out
/// (LSP-16) can publish more than one `PublishDiagnostics` notification per
/// analysis (one per project file), so a response is no longer guaranteed
/// to be the very next message after a request.
fn recv_response(rx: &Receiver<Message>, id: RequestId, timeout_ms: u64) -> lsp_server::Response {
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

/// A module that also has a same-named `analog`/`digital` behavior block
/// (e.g. `mod PwmSwitch(...) {}` + `analog PwmSwitch { ... }`) — hover on
/// the `mod` declaration's own name must resolve as the Module (with the
/// module's own doc), not as the Behavior (whose `.name` is always the
/// SAME string as the owning module's name, so a bare name match without
/// position-awareness ambiguously prefers whichever comes first). Mirrors
/// the exact real repro from `examples/10_pwm_dimmer.phdl`.
#[test]
fn hover_on_module_with_same_named_behavior_resolves_the_module_not_the_behavior() {
    let source = "discipline Electrical { potential v: Real; flow i: Real; }\n\
/// The switch.\n\
mod PwmSwitch(inout sw: Electrical, inout gnd: Electrical) { var drive: Real = 0.0; }\n\
analog PwmSwitch { V(sw, gnd) <- drive; }\n";
    // line 2 (0-indexed) is `mod PwmSwitch(...)`; character 4 lands on `PwmSwitch`.
    let contents = lsp_hover_markdown(source, 2, 4);
    assert!(
        contents.contains("The switch."),
        "hover must show the MODULE's doc, not fall through to the same-named behavior (which has none), got: {contents}"
    );
    assert!(contents.contains("**module** `PwmSwitch`"), "hover must resolve as module, got: {contents}");
}

/// `discipline` declarations never got a `doc` field wired at all (a gap
/// distinct from BUG-2's extern-declaration fix) — hover on a documented
/// `discipline` must show its `///` doc, same as `mod`/`param`/etc.
#[test]
fn hover_on_documented_discipline_renders_doc_as_markdown() {
    let source = "/// Electrical signals: voltage and current.\ndiscipline Electrical { potential v: Real; flow i: Real; }\n";
    // line 1 is `discipline Electrical {...}`; character 13 lands on `Electrical`.
    let contents = lsp_hover_markdown(source, 1, 13);
    assert!(
        contents.contains("Electrical signals: voltage and current."),
        "hover must include the discipline's doc text, got: {contents}"
    );
    assert!(contents.contains("**discipline** `Electrical`"), "hover must resolve as discipline, got: {contents}");
}

// ── BUG-2 (LSB-04..06): hover shows `///` docs for extern declarations ──────

/// spec.md's Independent Test for P2 (BUG-2): hover on a `///`-documented
/// `extern` use-site renders the doc as Markdown, same convention as an
/// already-documented `mod`/`param` (mirrors
/// `hover_on_documented_module_renders_doc_as_markdown`).
#[test]
fn hover_on_documented_extern_operator_renders_doc_as_markdown() {
    let source = "/// Time derivative of its argument.\nextern operator my_ddt(x: Real) -> Real;\ndiscipline Electrical { potential v: Real; flow i: Real; }\nmod Cap (inout p: Electrical, inout n: Electrical) {\n    param c: Real = 1.0;\n    analog Behave { I(p, n) <+ c * my_ddt(V(p, n)); }\n}\n";

    let call_offset = source.find("my_ddt(V").unwrap();
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, ch) in source[..call_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    let character = source[last_nl..call_offset].chars().count() as u32;

    let contents = lsp_hover_markdown(source, line, character);
    assert!(
        contents.contains("Time derivative of its argument."),
        "hover contents must include the extern operator's doc text, got: {contents}"
    );
}

/// LSP-09-equivalent no-regression check for BUG-2: an `extern` declaration
/// with no `///` run still hovers without a stray doc paragraph.
#[test]
fn hover_on_undocumented_extern_operator_is_unchanged() {
    let source = "extern operator my_ddt(x: Real) -> Real;\ndiscipline Electrical { potential v: Real; flow i: Real; }\nmod Cap (inout p: Electrical, inout n: Electrical) {\n    param c: Real = 1.0;\n    analog Behave { I(p, n) <+ c * my_ddt(V(p, n)); }\n}\n";

    let call_offset = source.find("my_ddt(V").unwrap();
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, ch) in source[..call_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    let character = source[last_nl..call_offset].chars().count() as u32;

    let contents = lsp_hover_markdown(source, line, character);
    assert!(
        !contents.to_lowercase().contains("derivative"),
        "no doc text should appear when there is no `///` run, got: {contents}"
    );
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

// ── BUG-1 (LSB-01..03): extern goto lands on the real declaring file ────────

/// spec.md's Independent Test for P1 (BUG-1): goto on `ddt` inside an analog
/// body must return a `Location` whose URI is the real declaring file
/// (`headers/operators.phdl`) and whose range covers the real
/// `extern operator ddt` text there — not the current document.
#[test]
fn goto_definition_on_ddt_lands_on_operators_header() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod Cap (inout p: Electrical, inout n: Electrical) {\n\
    param c: Real = 1.0;\n\
    analog Behave { I(p, n) <+ c * ddt(V(p, n)); }\n\
}\n";

    let ddt_call_offset = src.find("ddt(V").unwrap();
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, ch) in src[..ddt_call_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    let character = src[last_nl..ddt_call_offset].chars().count() as u32;

    let response = lsp_goto_definition(src, line, character);
    let loc = match response {
        GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };

    let uri_str = loc.uri.as_str();
    assert!(
        uri_str.ends_with("headers/operators.phdl"),
        "goto on `ddt` must land on headers/operators.phdl, got {uri_str}"
    );
    let path = url::Url::parse(uri_str).unwrap().to_file_path().unwrap();
    assert!(path.is_file(), "{} must exist on disk", path.display());

    let header_text = std::fs::read_to_string(&path).unwrap();
    let expected_offset = header_text.find("extern operator ddt").unwrap();
    let expected_offset_byte = position_to_byte(&header_text, loc.range.start);
    assert_eq!(
        expected_offset_byte, expected_offset,
        "goto range must cover the real `extern operator ddt` declaration text"
    );
}

/// spec.md P1 AC3 (no regression): when the `extern` name is declared in
/// the *current* document (a user's own `extern` stub), goto-definition
/// must still resolve there — the file-based branch must not divert it
/// elsewhere.
#[test]
fn goto_definition_on_same_file_extern_decl_still_works() {
    let src = "extern operator my_op(x: Real) -> Real;\n\
discipline Electrical { potential v: Real; flow i: Real; }\n\
mod Cap (inout p: Electrical, inout n: Electrical) {\n\
    analog Behave { I(p, n) <+ my_op(V(p, n)); }\n\
}\n";

    let call_offset = src.find("my_op(V").unwrap();
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, ch) in src[..call_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    let character = src[last_nl..call_offset].chars().count() as u32;

    let response = lsp_goto_definition(src, line, character);
    let loc = match response {
        GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };

    assert_eq!(
        loc.uri.as_str(),
        "file:///goto_def_test.phdl",
        "goto on a same-file extern decl must resolve within the current document, not another file"
    );
    let target_offset = position_to_byte(src, loc.range.start);
    let decl_offset = src.find("extern operator my_op").unwrap();
    assert_eq!(target_offset, decl_offset, "goto must land on my_op's own declaration");
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

// ── T10: rename handler rides binding occurrences (LSP-11) ─────────────────

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/rename` response for `(line, character)` -> `new_name`.
fn lsp_rename(source: &str, line: u32, character: u32, new_name: &str) -> Option<WorkspaceEdit> {
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
fn lsp_prepare_rename(source: &str, line: u32, character: u32) -> Option<PrepareRenameResponse> {
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

/// LSP-11: renaming a `power` param declared in module A must not edit
/// module B's own unrelated `power` param.
#[test]
fn rename_edits_only_the_binding_uses_other_scope_untouched() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
mod A (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 1.0;\n\
}\n\
mod B (inout p: Electrical, inout n: Electrical) {\n\
    param power: Real = 2.0;\n\
}\n";

    let a_line = src[..src.find("param power").unwrap()].matches('\n').count() as u32;
    let character = "    param ".chars().count() as u32;
    let b_line = src[..src.find("mod B").unwrap()].matches('\n').count() as u32;

    let edit = lsp_rename(src, a_line, character, "gain").expect("rename on A's power must succeed");
    let changes = edit.changes.expect("rename must produce changes");
    let uri: Uri = "file:///rename_test.phdl".parse().unwrap();
    let edits = changes.get(&uri).expect("changes must target the open document");

    assert!(!edits.is_empty(), "at least the declaration site must be edited");
    for e in edits {
        assert_eq!(e.new_text, "gain");
        assert!(e.range.start.line < b_line, "module B's own `power` must never be edited by A's rename");
    }
}

/// LSP-11 edge case: prepare-rename declines (returns `None`) on a
/// non-renameable token — here, a numeric literal.
#[test]
fn prepare_rename_declines_on_literal() {
    let src = "mod Top() {}\ndigital Top { var y: Real = 1.0; }";
    let line = src[..src.rfind("1.0").unwrap()].matches('\n').count() as u32;
    let line_start = src[..src.rfind("1.0").unwrap()].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (src.rfind("1.0").unwrap() - line_start) as u32;

    let response = lsp_prepare_rename(src, line, character);
    assert!(response.is_none(), "prepare-rename must decline on a numeric literal, got: {response:?}");
}

// ── T11: document-highlight rides binding occurrences (LSP-13) ─────────────

/// Drives a `Connection::memory()` round trip and returns the
/// `textDocument/documentHighlight` response for `(line, character)`.
fn lsp_document_highlight(source: &str, line: u32, character: u32) -> Vec<lsp_types::DocumentHighlight> {
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

/// LSP-13: highlighting module A's `power` must never highlight module B's
/// own unrelated `power` declaration or a `// power` comment mention —
/// same binding-identity source as references (T9), not a text scan.
#[test]
fn document_highlight_excludes_other_scope_and_comment_matches() {
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
    let comment_line = src[..src.find("power is computed").unwrap()].matches('\n').count() as u32;
    let b_line = src[..src.find("mod B").unwrap()].matches('\n').count() as u32;

    let highlights = lsp_document_highlight(src, a_line, character);

    assert!(!highlights.is_empty(), "highlight must return at least the declaration site");
    for h in &highlights {
        assert_ne!(h.range.start.line, comment_line, "a `// power` comment must never be highlighted");
        assert!(h.range.start.line < b_line, "module B's own `power` must never be highlighted from A's cursor");
    }
}

// ── T12: `ProjectUnit` — multi-file index (LSP-14) ──────────────────────────

/// A scratch on-disk project (`Piperine.toml` + `src/`), removed on drop —
/// mirrors `piperine-project`'s own `ScratchDir` test helper
/// (`crates/piperine-project/src/source_map.rs`).
struct ScratchProject(std::path::PathBuf);

impl ScratchProject {
    fn new(tag: &str) -> Self {
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

/// LSP-14: a project's `ProjectUnit` covers every `.phdl` file under
/// `src/` — one `Design` per file, one `ResolutionIndex` spanning both,
/// with every binding's `file` stamped to its owning source path.
#[test]
fn project_unit_builds_one_index_spanning_all_project_files() {
    let scratch = ScratchProject::new("multi_file");
    let a_path = scratch.write_src(
        "a.phdl",
        "discipline Electrical { potential v: Real; flow i: Real; }\nmod A (inout p: Electrical, inout n: Electrical) { param gain: Real = 1.0; }\n",
    );
    let b_path = scratch.write_src(
        "b.phdl",
        "discipline Electrical { potential v: Real; flow i: Real; }\nmod B (inout p: Electrical, inout n: Electrical) { param gain: Real = 2.0; }\n",
    );

    let uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();
    let mut state = piperine_lang_server::state::ServerState::dummy();
    state.documents.insert(uri.clone(), DocumentState::new(std::fs::read_to_string(&a_path).unwrap(), 1));
    state.analyze_document(&uri);

    let doc = state.documents.get(&uri).expect("document must still be present");
    let root = doc.project_root.clone().expect("a.phdl must resolve to the scratch project's root");
    assert_eq!(root, scratch.0);

    let unit = state.projects.get(&root).expect("ServerState.projects must hold a unit for the discovered root");
    assert_eq!(unit.designs.len(), 2, "the unit must cover both a.phdl and b.phdl");
    assert!(unit.designs.contains_key(&a_path));
    assert!(unit.designs.contains_key(&b_path));

    let files: std::collections::HashSet<_> =
        unit.index.bindings().filter_map(|(_, info)| info.file.clone()).collect();
    assert!(
        files.contains(&a_path.display().to_string()),
        "bindings from a.phdl must carry a.phdl's path, got file set: {files:?}"
    );
    assert!(
        files.contains(&b_path.display().to_string()),
        "bindings from b.phdl must carry b.phdl's path, got file set: {files:?}"
    );
}

/// LSP-17 (T12's fallback half): a standalone document outside any
/// `Piperine.toml` gets `project_root: None` and no entry in
/// `ServerState.projects` — the existing single-file behavior is
/// unchanged, no regression.
#[test]
fn standalone_document_has_no_project_unit() {
    let uri: Uri = "file:///standalone_lsp_test_file.phdl".parse().unwrap();
    let mut state = piperine_lang_server::state::ServerState::dummy();
    state.documents.insert(uri.clone(), DocumentState::new("mod Top() {}".to_string(), 1));
    state.analyze_document(&uri);

    let doc = state.documents.get(&uri).unwrap();
    assert!(doc.project_root.is_none(), "a standalone file must not resolve to any project root");
    assert!(state.projects.is_empty(), "no ProjectUnit should be built for a standalone document");
    assert!(doc.design.is_some(), "standalone single-file elaboration must still work (no regression)");
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

// ── T13: cross-file goto (LSP-15) ────────────────────────────────────────

/// Drives a `Connection::memory()` round trip against a specific `uri`
/// (unlike `lsp_goto_definition`, which always opens a hardcoded
/// single-file uri) — needed for cross-file scenarios where a second
/// project file must already exist on disk before the request lands.
fn lsp_goto_definition_at(uri: &Uri, source: &str, line: u32, character: u32) -> GotoDefinitionResponse {
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

/// LSP-15 Independent Test: a two-file project where `b.phdl` `use`-imports
/// and instantiates a module declared in `a.phdl`; goto on the instance
/// type opens `a.phdl` at the module's declaration, not a (wrong) span
/// interpreted against `b.phdl`'s own buffer.
#[test]
fn cross_file_goto_opens_the_declaring_file() {
    let scratch = ScratchProject::new("goto_cross_file");
    let a_src = "pub discipline Electrical { potential v: Real; flow i: Real; }\npub mod A (inout p: Electrical, inout n: Electrical) { param gain: Real = 1.0; }\n";
    let a_path = scratch.write_src("a.phdl", a_src);
    let b_src = "use scratch_proj::a;\nmod B (inout p: Electrical, inout n: Electrical) {\n    inst: A(.p = p, .n = n);\n}\n";
    let b_path = scratch.write_src("b.phdl", b_src);

    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();
    // `    inst: A(...)` — cursor on the `A` type name.
    let line = 2u32;
    let character = "    inst: ".chars().count() as u32;

    let response = lsp_goto_definition_at(&b_uri, b_src, line, character);
    let loc = match response {
        GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };

    let a_uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();
    assert_eq!(loc.uri, a_uri, "goto on `A` must open a.phdl, not b.phdl");

    let target_offset = position_to_byte(a_src, loc.range.start);
    let mod_a_start = a_src.find("pub mod A").unwrap();
    assert!(
        target_offset >= mod_a_start,
        "goto target (offset {target_offset}) must land inside `A`'s own declaration (starting at {mod_a_start})"
    );
}

// ── T14: cross-file rename (LSP-12) ──────────────────────────────────────

/// Drives a `Connection::memory()` rename round trip against a specific
/// `uri` (the cross-file counterpart of `lsp_rename`, which always opens a
/// hardcoded single-file uri).
fn lsp_rename_at(uri: &Uri, source: &str, line: u32, character: u32, new_name: &str) -> Option<WorkspaceEdit> {
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

/// LSP-12 Independent Test: renaming a module used across a two-file
/// project (`b.phdl` `use`-imports and instantiates a module declared in
/// `a.phdl`) edits both files via `WorkspaceEdit.document_changes` — the
/// declaration in `a.phdl` and the instantiation's type name in `b.phdl`.
#[test]
fn cross_file_rename_edits_every_referencing_file() {
    let scratch = ScratchProject::new("rename_cross_file");
    let a_src = "pub discipline Electrical { potential v: Real; flow i: Real; }\npub mod A (inout p: Electrical, inout n: Electrical) { param gain: Real = 1.0; }\n";
    let a_path = scratch.write_src("a.phdl", a_src);
    let b_src = "use scratch_proj::a;\nmod B (inout p: Electrical, inout n: Electrical) {\n    inst: A(.p = p, .n = n);\n}\n";
    let b_path = scratch.write_src("b.phdl", b_src);

    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();
    // `    inst: A(...)` — cursor on the `A` type name.
    let line = 2u32;
    let character = "    inst: ".chars().count() as u32;

    let edit = lsp_rename_at(&b_uri, b_src, line, character, "Amp")
        .expect("renaming a cross-file module must produce a WorkspaceEdit");

    let document_changes = match edit.document_changes {
        Some(DocumentChanges::Edits(edits)) => edits,
        other => panic!("expected document_changes edits, got: {other:?}"),
    };
    assert_eq!(
        document_changes.len(), 2,
        "cross-file rename must edit both the declaring file and the referencing file"
    );

    let a_uri: Uri = format!("file://{}", a_path.display()).parse().unwrap();

    let a_edit = document_changes.iter().find(|e| e.text_document.uri == a_uri)
        .expect("a.phdl (the declaring file) must have an edit");
    assert_eq!(a_edit.edits.len(), 1, "a.phdl gets exactly one edit: the module's own name");
    let a_new_text = match &a_edit.edits[0] {
        lsp_types::OneOf::Left(te) => &te.new_text,
        lsp_types::OneOf::Right(ate) => &ate.text_edit.new_text,
    };
    assert_eq!(a_new_text, "Amp");

    let b_edit = document_changes.iter().find(|e| e.text_document.uri == b_uri)
        .expect("b.phdl (the referencing file) must have an edit");
    assert_eq!(b_edit.edits.len(), 1, "b.phdl gets exactly one edit: the instance's type name");
    let b_new_text = match &b_edit.edits[0] {
        lsp_types::OneOf::Left(te) => &te.new_text,
        lsp_types::OneOf::Right(ate) => &ate.text_edit.new_text,
    };
    assert_eq!(b_new_text, "Amp");

    // The edited range in a.phdl must fall on `A`'s own name, not
    // anywhere else in the declaration (e.g. inside the port list).
    let a_edit_start = position_to_byte(a_src, match &a_edit.edits[0] {
        lsp_types::OneOf::Left(te) => te.range.start,
        lsp_types::OneOf::Right(ate) => ate.text_edit.range.start,
    });
    let a_name_offset = a_src.find("pub mod A").unwrap() + "pub mod ".len();
    assert_eq!(a_edit_start, a_name_offset, "a.phdl's edit must target `A`'s own name token");
}

// ── T15: per-file diagnostic fan-out + single-file fallback (LSP-16/17) ──

/// Open `uri` (already written to disk as part of a project) and collect
/// every `PublishDiagnostics` notification received within `timeout_ms`,
/// keyed by the URI they were published against — T15's fan-out publishes
/// one notification per project file, not just the opened document's.
fn lsp_collect_diagnostics(uri: &Uri, source: &str, timeout_ms: u64) -> HashMap<Uri, Vec<lsp_types::Diagnostic>> {
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
                by_uri.insert(params.uri, params.diagnostics);
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

/// LSP-16 Independent Test: an error in `a.phdl` publishes against
/// `a.phdl`'s own URI, not the document that was actually opened
/// (`b.phdl`, which is itself error-free and doesn't even reference `a`).
#[test]
fn cross_file_diagnostics_fan_out_to_the_erroring_file() {
    let scratch = ScratchProject::new("diag_fan_out");
    // References an undeclared discipline — a genuine elaboration error.
    let a_src = "pub mod A (inout p: Nonexistent, inout n: Nonexistent) {}\n";
    scratch.write_src("a.phdl", a_src);
    let b_src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod B (inout p: Electrical, inout n: Electrical) {}\n";
    let b_path = scratch.write_src("b.phdl", b_src);

    let a_uri: Uri = format!("file://{}", scratch.0.join("src").join("a.phdl").display()).parse().unwrap();
    let b_uri: Uri = format!("file://{}", b_path.display()).parse().unwrap();

    let by_uri = lsp_collect_diagnostics(&b_uri, b_src, 800);

    let a_diags = by_uri.get(&a_uri).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for a.phdl, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!a_diags.is_empty(), "a.phdl's own undeclared-discipline error must be published against a.phdl");

    let b_diags = by_uri.get(&b_uri).expect("b.phdl must also get a (empty) diagnostics publish");
    assert!(b_diags.is_empty(), "b.phdl elaborates cleanly and must not inherit a.phdl's error");
}

/// LSP-17 Independent Test (fallback half): a standalone document outside
/// any `Piperine.toml` still gets its own diagnostics published — the
/// single-file path is unaffected by T15's project fan-out.
#[test]
fn standalone_document_diagnostics_still_publish() {
    let uri: Uri = "file:///standalone_diag_test.phdl".parse().unwrap();
    // Undeclared discipline — a genuine elaboration error, no project.
    let src = "mod Top (inout p: Nonexistent) {}\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);

    let diags = by_uri.get(&uri).unwrap_or_else(|| {
        panic!("expected a PublishDiagnostics notification for the standalone file, got URIs: {:?}", by_uri.keys().collect::<Vec<_>>())
    });
    assert!(!diags.is_empty(), "the standalone document's own error must still be published (LSP-17 fallback)");
}

// ── T17: diagnostic severity + structured codes (LSP-19) ──────────────────

/// An undeclared-discipline error (`ElabErrorKind::UndefinedType`, coded
/// `E2002` in `piperine_lang::pom::error`) must publish with that exact
/// code and `ERROR` severity — not the old blanket `"parse-error"` string.
#[test]
fn diagnostic_carries_the_structured_elab_error_code() {
    let uri: Uri = "file:///t17_elab_code.phdl".parse().unwrap();
    let src = "mod Top (inout p: Nonexistent) {}\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);
    let diags = by_uri.get(&uri).expect("expected a PublishDiagnostics notification");
    assert!(!diags.is_empty(), "the undefined-type error must be published");

    let d = &diags[0];
    assert_eq!(
        d.code,
        Some(lsp_types::NumberOrString::String("E2002".into())),
        "expected the ElabErrorKind::UndefinedType code E2002, got: {:?}",
        d.code
    );
    assert_eq!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
}

/// A parser-level syntax error carries its own distinct code family
/// (`E1xxx`, `piperine_lang::parse::error::ParseError`) — proving the
/// lexer/parser path and the elaboration path each surface their real
/// code, not a single shared placeholder.
#[test]
fn diagnostic_carries_the_structured_parse_error_code() {
    let uri: Uri = "file:///t17_parse_code.phdl".parse().unwrap();
    // Malformed module header — a parser-level syntax error, not elaboration.
    let src = "mod Top ( this is not valid phdl\n";

    let by_uri = lsp_collect_diagnostics(&uri, src, 800);
    let diags = by_uri.get(&uri).expect("expected a PublishDiagnostics notification");
    assert!(!diags.is_empty(), "the syntax error must be published");

    let d = &diags[0];
    let code = match &d.code {
        Some(lsp_types::NumberOrString::String(s)) => s.clone(),
        other => panic!("expected a string code, got: {other:?}"),
    };
    assert!(code.starts_with("E1"), "expected an E1xxx parser code, got: {code}");
    assert_ne!(code, "parse-error", "the blanket placeholder must be gone");
    assert_eq!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
}

// ── T18: `@schema` completion (LSP-20) ──────────────────────────────────────

use piperine_lang_server::handlers::completion::completions_at;

/// `@rf|` completes to `rfport` — the built-in `rfport` schema is always
/// in-scope (registered unconditionally in `ElabContext::new()`).
///
/// Types `@rf` after a valid document, simulating the moment completion is
/// triggered mid-edit: `doc.source` is updated but not re-analyzed, so
/// `doc.ctx` still holds the last successful elaboration's registries — the
/// pre-existing stale-but-valid resilience (`state.rs`) completion rides.
#[test]
fn schema_completion_after_at_sign_offers_in_scope_schema_names() {
    let mut doc = DocumentState::new("mod Top() {}\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some(), "must elaborate cleanly: {:?}", doc.errors);

    doc.source = "mod Top() {}\n@rf".to_string();
    let offset = doc.source.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"rfport"), "expected `rfport` in {labels:?}");
}

/// Only schema names matching the typed prefix are offered — `@zz` must not
/// include `rfport`.
#[test]
fn schema_completion_filters_by_typed_prefix() {
    let mut doc = DocumentState::new("mod Top() {}\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some());

    doc.source = "mod Top() {}\n@zz".to_string();
    let offset = doc.source.len();

    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"rfport"), "`@zz` must not offer `rfport`: {labels:?}");
}

/// Off the `@` position entirely, completion falls back to the ordinary
/// predictive-parser completions — schema names are not injected everywhere.
#[test]
fn completion_off_attr_position_does_not_offer_schema_names() {
    let mut doc = DocumentState::new("mod Top() {}\n".to_string(), 1);
    doc.analyze(&piperine_lang::SourceMap::dummy());
    assert!(doc.ctx.is_some());

    let offset = doc.source.len();
    let items = completions_at(&doc, offset);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"rfport"), "unrelated position must not offer schema names: {labels:?}");
}

// ── T19: attribute-argument validation (LSP-21) ─────────────────────────────

/// `@rfport(num = "x", ...)` — a bad-type argument value — must produce a
/// diagnostic (structured `E2023` code, `AttrSchemaField`) whose span covers
/// the specific offending argument (`num = "x"`), not the whole attribute
/// or a `0:0` fallback.
#[test]
fn attr_arg_bad_type_diagnostic_points_at_the_specific_argument() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = \"x\", z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a bad-type attribute argument must fail elaboration");
    assert!(!doc.errors.is_empty(), "expected at least one elaboration error");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let arg_start = src.find("num = \"x\"").expect("argument text must be present");
    assert_eq!(span.offset(), arg_start, "span must point at `num = \"x\"`, not the whole attribute or 0:0");
}

/// An unknown field (`bogus`, not part of the `rfport` schema) must produce
/// a diagnostic whose span covers that specific argument.
#[test]
fn attr_arg_unknown_field_diagnostic_points_at_the_specific_argument() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = 1, bogus = 2) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "an unknown attribute field must fail elaboration");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let arg_start = src.find("bogus = 2").expect("argument text must be present");
    assert_eq!(span.offset(), arg_start, "span must point at `bogus = 2`");
}

/// A missing required field (`num`, `rfport`'s only required field) — with
/// no single argument to blame — must still produce a diagnostic, with its
/// span falling back to the whole `@rfport(...)` attribute rather than
/// `0:0`.
#[test]
fn attr_missing_required_field_diagnostic_points_at_the_attribute() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_none(), "a missing required attribute field must fail elaboration");

    let err = doc.errors.iter().find(|e| e.code.as_deref() == Some("E2023"))
        .expect("expected the AttrSchemaField (E2023) diagnostic");
    let span = err.span.expect("diagnostic must carry a span");

    let attr_start = src.find("@rfport(z0 = 50)").expect("attribute text must be present");
    assert_eq!(span.offset(), attr_start, "span must fall back to the whole attribute, not 0:0");
}

// ── T20: hover -> schema fields, goto -> `@attribute` decl (LSP-22/23) ──────

use piperine_lang_server::text_pos::byte_to_position;

/// Hovering `@rfport` (a use site) lists its fields — `num` and `z0`
/// (spec.md's own P3 independent test) — as Markdown.
#[test]
fn hover_on_attr_schema_use_lists_its_fields() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = 1, z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.find("rfport(num").expect("use site must be present");
    let resolution = doc.resolve_at(use_site).expect("`@rfport` use site must resolve");
    assert_eq!(resolution.kind, SymbolKind::AttrSchema);

    let pos = byte_to_position(src, use_site);
    let contents = lsp_hover_markdown(src, pos.line, pos.character);
    assert!(contents.contains("num"), "expected `num` field in hover: {contents}");
    assert!(contents.contains("z0"), "expected `z0` field in hover: {contents}");
}

/// Goto-definition on an `@name(...)` use site opens the schema's own
/// `extern attribute` declaration. Uses a locally-declared schema
/// (`widget_meta`) rather than the prelude-embedded `model`/`rfport` so the
/// expected decl offset is computable against this document's own text.
#[test]
fn goto_definition_on_attr_schema_use_opens_its_extern_attribute_decl() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nextern attribute widget_meta { rating: Real }\nmod Top ( inout p : Electrical ) { @widget_meta(rating = 4.5) wire w : Electrical; }";
    let use_site = src.rfind("widget_meta(rating").expect("use site must be present");
    let pos = byte_to_position(src, use_site);

    let response = lsp_goto_definition(src, pos.line, pos.character);
    let target = goto_target_offset(&response, src);
    let decl_start = src.find("extern attribute widget_meta").expect("declaration must be present");
    assert_eq!(target, decl_start, "goto must open the `extern attribute widget_meta` declaration");
}

// ── T21: attribute outline entries (LSP-24) ─────────────────────────────────

use piperine_lang_server::handlers::symbols::extract_symbols;

/// A `@rfport`-annotated wire shows the attribute as a nested outline entry
/// on the wire's own declaration.
#[test]
fn attribute_instance_appears_as_outline_entry_on_its_declaration() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = 1, z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    let design = doc.design.as_ref().expect("source must elaborate cleanly");

    let symbols = extract_symbols(design, src);
    let module_sym = symbols.iter().find(|s| s.name == "M").expect("module `M` must be in the outline");
    let wire_children = module_sym.children.as_ref().expect("module must have children");
    let wire_sym = wire_children.iter().find(|s| s.name.starts_with("rf_in")).expect("wire `rf_in` must be in the outline");

    let attr_children = wire_sym.children.as_ref().expect("wire with an attribute must have outline children");
    assert!(
        attr_children.iter().any(|c| c.name == "@rfport"),
        "expected an `@rfport` outline entry, got: {:?}",
        attr_children.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}

/// A wire with no attribute has no attribute children in the outline (no
/// regression / no spurious entries).
#[test]
fn outline_entry_without_attribute_has_no_attribute_children() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\nwire plain : Electrical;\n}\n";
    let doc = analyzed(src);
    let design = doc.design.as_ref().expect("source must elaborate cleanly");

    let symbols = extract_symbols(design, src);
    let module_sym = symbols.iter().find(|s| s.name == "M").expect("module `M` must be in the outline");
    let wire_children = module_sym.children.as_ref().expect("module must have children");
    let wire_sym = wire_children.iter().find(|s| s.name.starts_with("plain")).expect("wire `plain` must be in the outline");

    assert!(wire_sym.children.is_none(), "an un-attributed wire must not carry outline children");
}
