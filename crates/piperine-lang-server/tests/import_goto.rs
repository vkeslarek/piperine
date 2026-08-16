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
//! **Blocked** (a newly discovered gap, parallel to but distinct from the
//! feature's documented `span: None` gap): `symbol_index::resolve_at`'s
//! cross-file `file` field is populated from `design.project().item_file
//! (name)` (`pom/design.rs`'s `Project::item_files`/`use_files` maps).
//! Those maps are filled in exclusively by `crate::resolve::Resolver::
//! item_files` — PHDL's own `use`-resolution pass, which no longer exists
//! in this crate at all (deleted with the PHDL parser, MD-45/T21; `grep -n
//! "struct Resolver" crates/piperine-lang/src/` finds nothing). Nothing
//! populates `item_files`/`use_files` for VAMS's `` `include`` mechanism —
//! confirmed directly: `design.project().item_file("res")` returns `None`
//! even after a clean `` `include "spice/passives.va"``-based elaboration.
//! So `resolve_at`'s `file` field is always `None` for VAMS content — the
//! module NAME itself still resolves correctly (step 3's global
//! module-name scan doesn't need `item_file` or any span), but "opens its
//! real declaring file" cannot be proven until VAMS gets its own
//! cross-file provenance wiring (an elaborator feature, out of scope for
//! this lang-server-wiring feature).

use piperine_lang_server::state::DocumentState;

fn analyzed(src: &str) -> DocumentState {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    let mut doc = DocumentState::new(src.to_string(), 1);
    doc.analyze(&map);
    assert!(doc.design.is_some(), "elaborates: {:?}", doc.errors);
    doc
}

/// **Blocked**: see the file-level doc — an `` `include``d module's type
/// name resolves correctly by NAME (`resolution.kind == Module`, `name ==
/// "res"`), but `resolution.file` is always `None` for VAMS content
/// because nothing populates `Project::item_files` outside the deleted
/// PHDL `use` resolver.
#[test]
fn goto_on_imported_module_type_resolves_to_its_real_declaring_file() {
    let src = "\
`include \"spice/passives.va\"
module Board();
  electrical a, gnd;
  res r1 (.p(a), .n(gnd));
endmodule
";
    let doc = analyzed(src);

    // Click on `res` at the instantiation site.
    let mod_off = src.find("res r1").unwrap();
    let r = doc.resolve_at(mod_off).expect("imported module resolves by name");
    assert_eq!(r.name, "res");
    assert!(r.file.as_ref().unwrap().ends_with("headers/spice/passives.va"), "res -> passives.va, got {:?}", r.file);
}
