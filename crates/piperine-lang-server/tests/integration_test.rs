use lsp_server::Connection;
use lsp_types::{Position, Uri};

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_server_new_creates_empty_state() {
    let (conn, _io_threads) = Connection::memory();
    let server = piperine_lang_server::server::LanguageServer::new(&conn);
    assert!(server.state.documents.is_empty());
}

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
fn test_document_state_upsert_valid_phdl() {
    use piperine_lang_server::state::ServerState;
    let mut state = ServerState::dummy();
    let uri: Uri = "file:///test.phdl".parse().unwrap();
    let source = "discipline Electrical { potential v: Real; flow i: Real; } \
                  mod R (inout p: Electrical, inout n: Electrical) {}";
    state.upsert_document(uri, source.to_string(), 1);
    assert!(state.documents.len() == 1);
    let doc = state.documents.values().next().unwrap();
    assert!(doc.errors.is_empty());
}

#[test]
fn test_document_state_upsert_invalid_phdl() {
    use piperine_lang_server::state::ServerState;
    let mut state = ServerState::dummy();
    let uri: Uri = "file:///test.phdl".parse().unwrap();
    let source = "mod Bad { this is not valid phdl }";
    state.upsert_document(uri, source.to_string(), 1);
    let doc = state.documents.values().next().unwrap();
    assert!(doc.design.is_none());
    assert!(!doc.errors.is_empty());
}

#[test]
fn test_byte_to_line_col_first_line() {
    let source = "mod foo {\n  wire x: Electrical;\n}";
    let (line, col) = byte_to_line_col(source, 4);
    assert_eq!(line, 0);
    assert_eq!(col, 4);
}

#[test]
fn test_byte_to_line_col_second_line() {
    let source = "mod foo {\n  wire x: Electrical;\n}";
    let (line, _col) = byte_to_line_col(source, 12);
    assert_eq!(line, 1);
}

#[test]
fn test_byte_to_line_col_eof() {
    let source = "mod foo {}\n";
    let len = source.len();
    let (line, col) = byte_to_line_col(source, len);
    assert_eq!(line, 1);
    assert_eq!(col, 0);
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
fn test_word_at_position_simple() {
    let source = "mod resistor {\n  param r: Real = 1e3;\n}";
    let word = piperine_lang_server::handlers::hover::word_at_position(
        source,
        Position {
            line: 0,
            character: 6,
        },
    );
    assert_eq!(word.as_deref(), Some("resistor"));
}

#[test]
fn test_word_at_position_keyword() {
    let source = "mod resistor {\n  param r: Real = 1e3;\n}";
    let word = piperine_lang_server::handlers::hover::word_at_position(
        source,
        Position {
            line: 0,
            character: 0,
        },
    );
    assert_eq!(word.as_deref(), Some("mod"));
}

#[test]
fn test_word_at_position_inside_identifier() {
    let source = "mod resistor {}";
    let word = piperine_lang_server::handlers::hover::word_at_position(
        source,
        Position {
            line: 0,
            character: 4,
        },
    );
    assert_eq!(word.as_deref(), Some("resistor"));
}

#[test]
fn test_find_definition_module() {
    let source = "mod resistor (inout p: Electrical, inout n: Electrical) {}";
    let range =
        piperine_lang_server::handlers::goto_def::find_definition(source, "resistor", None);
    assert!(range.is_some());
    let r = range.unwrap();
    assert_eq!(r.start.line, 0);
    assert_eq!(r.start.character, 4); // "resistor" starts at char 4 after "mod "
}

#[test]
fn test_find_definition_not_found() {
    let source = "mod resistor {}";
    let range =
        piperine_lang_server::handlers::goto_def::find_definition(source, "nonexistent", None);
    assert!(range.is_none());
}



#[test]
fn test_hover_module_info() {
    let design = piperine_lang::parse_and_elaborate(
        "discipline Electrical { potential v: Real; flow i: Real; } \
         mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }",
    )
    .expect("parse failed");
    let info = piperine_lang_server::handlers::hover::lookup_hover_info(&design, "R");
    assert!(info.is_some());
    assert!(info.unwrap().contains("module"));
}

#[test]
fn test_hover_discipline_info() {
    let design =
        piperine_lang::parse_and_elaborate("discipline Electrical { potential v: Real; flow i: Real; }")
            .expect("parse failed");
    let info = piperine_lang_server::handlers::hover::lookup_hover_info(&design, "Electrical");
    assert!(info.is_some());
    assert!(info.unwrap().contains("discipline"));
}

#[test]
fn test_diagnostics_no_error_on_valid_code() {
    use piperine_lang_server::state::ServerState;
    let mut state = ServerState::dummy();
    let uri: Uri = "file:///valid.phdl".parse().unwrap();
    let source = "discipline Electrical { potential v: Real; flow i: Real; }";
    state.upsert_document(uri, source.to_string(), 1);
    let doc = state.documents.values().next().unwrap();
    assert!(doc.design.is_some(), "valid code should parse successfully");
    assert!(doc.errors.is_empty());
}

#[test]
fn test_diagnostics_error_on_bad_syntax() {
    use piperine_lang_server::state::ServerState;
    let mut state = ServerState::dummy();
    let uri: Uri = "file:///bad.phdl".parse().unwrap();
    let source = "mod Bad { @@@ }";
    state.upsert_document(uri, source.to_string(), 1);
    let doc = state.documents.values().next().unwrap();
    assert!(doc.design.is_none());
    assert!(!doc.errors.is_empty());
}



// ── Helpers ─────────────────────────────────────────────────────────────────

fn byte_to_line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let offset = byte_offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() as u32;
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = (offset - last_newline) as u32;
    (line, col)
}
