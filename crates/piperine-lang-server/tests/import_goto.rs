//! Go-to-definition across `` `include``s: on an imported module's type
//! name, clicking resolves to its real declaring header, not the current
//! document. Regression guard for the import-goto bugs found testing the
//! VS Code extension.
//!
//! **lang-server-vams-migration (T6)**: PHDL's `use pkg::mod;` path-segment
//! click scenario (two of this file's three original sub-scenarios: click
//! on a `use` path segment opens the loaded file) has **no VAMS
//! equivalent at all** — VAMS only has `` `include "file.va"`` (a raw
//! textual include, not an addressable, per-segment-clickable module path),
//! and `symbol_index::resolve_at` has no include-directive-click handling.
//! This is a genuinely deleted language mechanism (MD-44/45), not an
//! out-of-scope gap to route around — those two sub-scenarios are dropped
//! rather than forced onto syntax that doesn't exist.
//!
//! The third sub-scenario (an imported module's *type name*, clicked at an
//! instantiation site, resolves to its real declaring file) DOES have a
//! direct VAMS equivalent — `` `include "spice/passives.va"`` + an
//! instance of the stdlib `res` module — and is kept below, migrated.
//!
//! **Fixed**: `goto_def::handle` no longer relies on `Project::item_files`/
//! `use_files` (PHDL's own dead `use`-resolution pass, MD-45/T21 — nothing
//! populates them for VAMS's `` `include`` mechanism, confirmed directly:
//! `design.project().item_file("res")` still returns `None` after a clean
//! `` `include "spice/passives.va"``-based elaboration). Instead,
//! `symbol_index::resolve_decl_location` resolves *any* `decl_span`
//! through `Design::source_provenance` — the same real per-`` `include ``
//! -file identity `preprocessor-span-mapping` already built — so the
//! actual LSP `textDocument/definition` response now jumps to the real
//! declaring file, independent of the still-dead `item_files` map.

mod common;
use common::*;

/// An `` `include``d module's type name resolves to its real declaring
/// file — driven through the real `textDocument/definition` round trip
/// (not `resolve_at` directly, since the fix lives in how `goto_def`
/// turns a `decl_span` into a `Location`, not in `Resolution` itself).
/// Uses a `ScratchProject` (a real on-disk `Piperine.toml` + `src/`) so
/// `` `include`` resolves through the project's own `SourceMap` — the
/// crate's builtin `piperine`/`spice` header namespaces are not reachable
/// from `SourceMap::dummy()`'s relative paths under this crate's own test
/// working directory, so a self-contained two-file project is the correct
/// (and only reliable) way to exercise a real `` `include``.
#[test]
fn goto_on_imported_module_type_resolves_to_its_real_declaring_file() {
    let scratch = ScratchProject::new("import_goto_cross_file");
    let inc_src = "\
module res(p, n);
  inout electrical p, n;
  parameter real r = 1.0;
  analog V(p,n) <+ r * I(p,n);
endmodule
";
    let inc_path = scratch.write_src("inc.vams", inc_src);
    let top_src = "\
`include \"inc.vams\"
module Board();
  electrical a, gnd;
  res r1 (.p(a), .n(gnd));
endmodule
";
    let top_path = scratch.write_src("top.vams", top_src);
    let top_uri: Uri = format!("file://{}", top_path.display()).parse().unwrap();

    // `res r1` — land the cursor on `res`.
    let off = top_src.find("res r1").unwrap();
    let pos = byte_to_position(top_src, off);
    let resp = lsp_goto_definition_at(&top_uri, top_src, pos.line, pos.character);
    let loc = match &resp {
        lsp_types::GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };
    let inc_uri: Uri = format!("file://{}", inc_path.display()).parse().unwrap();
    assert_eq!(loc.uri.as_str(), inc_uri.as_str(), "res -> inc.vams, got {}", loc.uri.as_str());
    // The location must also land on the real declaration line inside
    // inc.vams, not offset (0,0)/garbage — `module res(p, n);` is line 0.
    assert_eq!(loc.range.start.line, 0, "must point at inc.vams's own module declaration line");
}

/// A click on the `` `include "path" `` directive itself opens that file
/// (Ctrl+click on an include line, not on a symbol it declares — a
/// separate bug found testing the VS Code extension).
#[test]
fn click_on_include_directive_opens_the_included_file() {
    let scratch = ScratchProject::new("import_goto_include_click");
    let inc_path = scratch.write_src("inc.vams", "module res(p, n);\nendmodule\n");
    let top_src = "`include \"inc.vams\"\nmodule Board();\nendmodule\n";
    let top_path = scratch.write_src("top.vams", top_src);
    let top_uri: Uri = format!("file://{}", top_path.display()).parse().unwrap();

    let off = top_src.find("inc.vams").unwrap();
    let pos = byte_to_position(top_src, off);
    let resp = lsp_goto_definition_at(&top_uri, top_src, pos.line, pos.character);
    let loc = match &resp {
        lsp_types::GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };
    let inc_uri: Uri = format!("file://{}", inc_path.display()).parse().unwrap();
    assert_eq!(loc.uri.as_str(), inc_uri.as_str(), "clicking the include path must open inc.vams, got {}", loc.uri.as_str());
}

/// A discipline declared inside `` `include``d content (`disciplines.vams`
/// via the crate's own `piperine` header namespace) resolves to that real
/// file, not a wrong location inside the current document — the exact
/// scenario found testing the VS Code extension (Ctrl+click on
/// `electrical` "pointed at the same file even though it's declared in an
/// include").
#[test]
fn goto_on_include_declared_discipline_resolves_to_its_real_file() {
    let scratch = ScratchProject::new("import_goto_discipline");
    let disc_path = scratch.write_src(
        "disc.vams",
        "\
nature Voltage;
  units = \"V\";
  access = V;
  abstol = 1e-6;
endnature
nature Current;
  units = \"A\";
  access = I;
  abstol = 1e-12;
endnature
discipline electrical;
  potential Voltage;
  flow Current;
enddiscipline
",
    );
    let top_src = "\
`include \"disc.vams\"
module Board();
  electrical a, gnd;
endmodule
";
    let top_path = scratch.write_src("top.vams", top_src);
    let top_uri: Uri = format!("file://{}", top_path.display()).parse().unwrap();

    // The `electrical` in `electrical a, gnd;` — not the include line.
    let off = top_src.rfind("electrical").unwrap();
    let pos = byte_to_position(top_src, off);
    let resp = lsp_goto_definition_at(&top_uri, top_src, pos.line, pos.character);
    let loc = match &resp {
        lsp_types::GotoDefinitionResponse::Scalar(loc) => loc,
        other => panic!("expected a scalar goto-definition response, got: {other:?}"),
    };
    let disc_uri: Uri = format!("file://{}", disc_path.display()).parse().unwrap();
    assert_eq!(loc.uri.as_str(), disc_uri.as_str(), "electrical -> disc.vams, got {}", loc.uri.as_str());
    let disc_src = std::fs::read_to_string(&disc_path).unwrap();
    let want_line = disc_src[..disc_src.find("discipline electrical").unwrap()].matches('\n').count() as u32;
    assert_eq!(loc.range.start.line, want_line, "must point at disc.vams's own discipline declaration line");
}
