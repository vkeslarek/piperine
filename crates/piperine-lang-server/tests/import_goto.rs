//! Go-to-definition across imports: on a `use` statement's path (opens
//! the loaded file) and on an imported module's type name (opens its real
//! declaring header, not the current document). Regression guard for the
//! import-goto bugs found testing the VS Code extension.

use piperine_lang_server::state::DocumentState;

fn analyzed(src: &str) -> DocumentState {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    let mut doc = DocumentState::new(src.to_string(), 1);
    doc.analyze(&map);
    assert!(doc.design.is_some(), "elaborates: {:?}", doc.errors);
    doc
}

#[test]
fn goto_on_use_path_and_imported_module() {
    let src = "\
use piperine::disciplines;
use spice::passives;
mod Board() {
    wire a: Electrical; wire gnd: Electrical;
    r1: res(.p = a, .n = gnd) { .r = 1e3 };
}
";
    let doc = analyzed(src);

    // Click on `passives` in the `use spice::passives;` line.
    let use_off = src.find("spice::passives").unwrap() + "spice::".len();
    let r = doc.resolve_at(use_off).expect("use path resolves");
    assert!(r.file.as_ref().unwrap().ends_with("headers/spice/passives.phdl"), "use path opens passives.phdl, got {:?}", r.file);

    // Click on `spice` in the same use line.
    let use_off2 = src.find("spice::passives").unwrap();
    let r2 = doc.resolve_at(use_off2).expect("use path (first seg) resolves");
    assert!(r2.file.as_ref().unwrap().ends_with("headers/spice/passives.phdl"));

    // Imported module type `res`.
    let mod_off = src.find("r1: res").unwrap() + "r1: ".len();
    let r3 = doc.resolve_at(mod_off).expect("imported module resolves");
    assert!(r3.file.as_ref().unwrap().ends_with("headers/spice/passives.phdl"), "res -> passives.phdl, got {:?}", r3.file);
}
