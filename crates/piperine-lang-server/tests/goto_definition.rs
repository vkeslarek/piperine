//! Go-to-definition: `extern` use sites resolving to their declaration spans,
//! shadowed names landing on the innermost binding, stdlib header targets, and
//! the cross-file case.

mod common;
use common::*;

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
