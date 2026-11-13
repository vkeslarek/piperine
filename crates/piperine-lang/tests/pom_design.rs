//! Scope: building a `Design` from sources (requirement POM-01): the retained
//! name, text and syntax tree, parse diagnostics passed through with their
//! file, determinism, and thread safety.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::msl::parser::parse;
use piperine_lang::pom::{Design, FileId, Reflect};
use support::{build, codes};

#[test]
fn build_retains_name_text_and_ast() {
    let text = "module R(p, n) {}\nconst N: u32 = 8;\n";
    let design = build(text);
    let files = design.files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name(), "top.msl");
    assert_eq!(files[0].text(), text);
    assert_eq!(files[0].ast(), &parse(text).file);
}

#[test]
fn parse_diagnostics_pass_through_with_file() {
    let text = "module M(\nconst N: u32 = ;\n";
    let expected = parse(text).diagnostics;
    assert!(!expected.is_empty(), "the fixture must have a syntax error");
    let design = build(text);
    let got = design.diagnostics();
    assert_eq!(got.len(), expected.len());
    for (g, e) in got.iter().zip(&expected) {
        assert_eq!(g.file, FileId(0));
        assert_eq!(g.diagnostic.code, e.code);
        assert_eq!(g.diagnostic.span(), e.span());
    }
    assert_eq!(codes(&design), expected.iter().map(|d| d.code.to_string()).collect::<Vec<_>>());
}

#[test]
fn design_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Design>();
}

#[test]
fn two_builds_dump_identical() {
    let text = "/// A resistor.\nmodule R(p: Electrical, n: Electrical) {}\nmod a { const N: u32 = 8; }\nnature V { potential: Voltage { access: v } }\n";
    assert!(parse(text).diagnostics.is_empty(), "fixture must parse cleanly");
    let first = build(text).dump();
    let second = build(text).dump();
    assert!(!first.is_empty());
    assert_eq!(first, second);
    // Ids are assigned in source order, so the order of lines is the order of
    // the source, and the same call twice cannot disagree.
    let lines: Vec<_> = first.lines().collect();
    assert!(lines[0].starts_with("Namespace "));
    assert!(lines[1].trim_start().starts_with("Module R "));
    assert!(lines[2].trim_start().starts_with("Port p "));
    assert!(lines[3].trim_start().starts_with("Port n "));
    assert!(lines[4].trim_start().starts_with("Namespace a "));
    assert!(lines[5].trim_start().starts_with("Const N "));
    assert!(lines[6].trim_start().starts_with("Nature V "));
    assert!(lines[7].trim_start().starts_with("Quantity Voltage "));
}

#[test]
fn empty_source_has_empty_root() {
    let design = build("");
    assert!(design.root().children().is_empty());
    assert_eq!(design.files().len(), 1);
    assert!(design.diagnostics().is_empty());
    assert!(design.check().is_empty());
}

fn files_design(files: &[(&str, &str)]) -> Design {
    let mut builder = Design::builder();
    for (path, text) in files {
        builder = builder.source(*path, *text);
    }
    builder.build()
}

fn module_path(design: &Design, path: &str) -> std::path::PathBuf {
    let m = design.find(path).unwrap_or_else(|| panic!("{path} not found"));
    assert_eq!(m.kind().label(), "Module", "{path}");
    m.file().path().expect("a path").to_path_buf()
}

#[test]
fn mod_loads_sibling_file_or_mod_file() {
    use piperine_lang::pom::NamespaceOrigin;
    let sibling = files_design(&[("top.msl", "mod b;\n"), ("b.msl", "module M() {}\n")]);
    assert!(codes(&sibling).is_empty(), "{:?}", sibling.diagnostics());
    assert_eq!(module_path(&sibling, "b::M"), std::path::Path::new("b.msl"));
    let b = sibling.find("b").and_then(|d| d.as_namespace()).expect("namespace b");
    assert_eq!(b.origin(), &NamespaceOrigin::External { file: Some(piperine_lang::pom::FileId(1)) });
    assert_eq!(b.source_file().map(|f| f.name()), Some("b.msl"));
    assert_eq!(b.items().len(), 1);
    assert_eq!(sibling.find("b::M").unwrap().declared_in(), Some(b.decl()));

    let dir = files_design(&[("top.msl", "mod b;\n"), ("b/mod.msl", "module M() {}\n")]);
    assert!(codes(&dir).is_empty(), "{:?}", dir.diagnostics());
    assert_eq!(module_path(&dir, "b::M"), std::path::Path::new("b/mod.msl"));
    assert_eq!(dir.files().iter().map(|f| f.name()).collect::<Vec<_>>(), ["top.msl", "mod.msl"]);
}

#[test]
fn mod_without_file_is_empty_namespace_e0161() {
    use piperine_lang::pom::NamespaceOrigin;
    let text = "mod b;\nmodule R() {}\n";
    let design = files_design(&[("top.msl", text)]);
    let diags = design.diagnostics();
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].diagnostic.code, "E0161");
    assert_eq!(diags[0].file, FileId(0));
    assert_eq!(diags[0].diagnostic.span(), design.find("b").unwrap().span());
    assert!(diags[0].diagnostic.message.contains("b.msl"), "{}", diags[0].diagnostic.message);
    assert!(diags[0].diagnostic.message.contains("b/mod.msl"), "{}", diags[0].diagnostic.message);
    let b = design.find("b").and_then(|d| d.as_namespace()).expect("an empty namespace b stays in the tree");
    assert!(b.items().is_empty());
    assert_eq!(b.origin(), &NamespaceOrigin::External { file: None });
    assert!(b.source_file().is_none());
    assert_eq!(design.files().len(), 1);
    assert!(design.find("R").is_some());
}

#[test]
fn mod_with_two_candidates_is_e0161() {
    let design = files_design(&[
        ("top.msl", "mod b;\n"),
        ("b.msl", "module A() {}\n"),
        ("b/mod.msl", "module B() {}\n"),
    ]);
    let diags = design.diagnostics();
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].diagnostic.code, "E0161");
    assert_eq!(diags[0].diagnostic.span(), design.find("b").unwrap().span());
    assert!(diags[0].diagnostic.message.contains("b.msl") && diags[0].diagnostic.message.contains("b/mod.msl"));
    assert!(design.find("b").unwrap().children().is_empty(), "neither file is loaded");
    assert!(design.find("b::A").is_none() && design.find("b::B").is_none());
    assert_eq!(design.files().len(), 1);
}

#[test]
fn nested_mod_files_follow_rust_layout() {
    use std::path::Path;
    // A plain file `b.msl` keeps its children under `b/`.
    let plain = files_design(&[
        ("top.msl", "mod b;\nmod x;\n"),
        ("b.msl", "mod c;\n"),
        ("b/c.msl", "module C() {}\n"),
        ("x.msl", "module X() {}\n"),
    ]);
    assert!(codes(&plain).is_empty(), "{:?}", plain.diagnostics());
    assert_eq!(module_path(&plain, "b::c::C"), Path::new("b/c.msl"));
    // Files: root, then each loaded file in the order its `mod` is met.
    assert_eq!(plain.files().iter().map(|f| f.path().unwrap().to_path_buf()).collect::<Vec<_>>(),
        [Path::new("top.msl"), Path::new("b.msl"), Path::new("b/c.msl"), Path::new("x.msl")]);

    // `b/mod.msl` and `b/c/mod.msl`.
    let modfile = files_design(&[
        ("top.msl", "mod b;\n"),
        ("b/mod.msl", "mod c;\n"),
        ("b/c/mod.msl", "module C() {}\n"),
    ]);
    assert!(codes(&modfile).is_empty(), "{:?}", modfile.diagnostics());
    assert_eq!(module_path(&modfile, "b::c::C"), Path::new("b/c/mod.msl"));

    // `b/mod.msl` loads `b/c.msl`.
    let beside = files_design(&[("top.msl", "mod b;\n"), ("b/mod.msl", "mod c;\n"), ("b/c.msl", "module C() {}\n")]);
    assert!(codes(&beside).is_empty(), "{:?}", beside.diagnostics());
    assert_eq!(module_path(&beside, "b::c::C"), Path::new("b/c.msl"));

    // Inside an inline `mod a { }` of the root: `a/c.msl` or `a/c/mod.msl`.
    let inline = files_design(&[("top.msl", "mod a { mod c; mod d; }\n"), ("a/c.msl", "module C() {}\n"), ("a/d/mod.msl", "module D() {}\n")]);
    assert!(codes(&inline).is_empty(), "{:?}", inline.diagnostics());
    assert_eq!(module_path(&inline, "a::c::C"), Path::new("a/c.msl"));
    assert_eq!(module_path(&inline, "a::d::D"), Path::new("a/d/mod.msl"));

    // Inline inside an external file: `b/i/d.msl`.
    let deep = files_design(&[("top.msl", "mod b;\n"), ("b.msl", "mod i { mod d; }\n"), ("b/i/d.msl", "module D() {}\n")]);
    assert!(codes(&deep).is_empty(), "{:?}", deep.diagnostics());
    assert_eq!(module_path(&deep, "b::i::d::D").as_path(), Path::new("b/i/d.msl"));
}

#[test]
fn prelude_files_are_numbered_first() {
    let design = Design::builder().prelude("std.msl", "pub const ONE: u32 = 1;").source("top.msl", "mod b;").source("b.msl", "").build();
    let files = design.files();
    assert_eq!(files.iter().map(|f| (f.name(), f.is_prelude())).collect::<Vec<_>>(), [("std.msl", true), ("top.msl", false), ("b.msl", false)]);
}

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[cfg(unix)]
#[test]
fn mod_cycle_is_e0161_and_not_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "mod.msl", "mod a;\nmodule Top() {}\n");
    write(dir.path(), "a/mod.msl", "mod back;\n");
    // `a/back` is the root directory again: `a/back/mod.msl` is the root file.
    std::os::unix::fs::symlink(dir.path(), dir.path().join("a/back")).unwrap();

    let design = Design::load(dir.path().join("mod.msl"));
    let cycles: Vec<_> = design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0161").collect();
    assert_eq!(cycles.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics().len(), 1);
    assert!(cycles[0].diagnostic.message.contains("cycle"), "{}", cycles[0].diagnostic.message);
    assert_eq!(cycles[0].file, FileId(1), "reported in the file that holds `mod back;`");
    assert_eq!(cycles[0].diagnostic.span(), design.find("a::back").unwrap().span());
    // The root and `a/mod.msl` are loaded once; `back` stays empty.
    assert_eq!(design.files().len(), 2);
    assert!(design.find("a::back").unwrap().children().is_empty());
    assert!(design.find("Top").is_some());
}

#[test]
fn load_from_disk_equals_in_memory_build() {
    let dir = tempfile::tempdir().unwrap();
    let top = "mod b;\nmod x;\nmodule Top() {}\n";
    let b = "mod c;\nmodule B() {}\n";
    let c = "//! c docs\nmodule C() {}\n";
    let x = "const N: u32 = 8;\n";
    write(dir.path(), "top.msl", top);
    write(dir.path(), "b.msl", b);
    write(dir.path(), "b/c.msl", c);
    write(dir.path(), "x/mod.msl", x);

    let disk = Design::load(dir.path().join("top.msl"));
    let memory = files_design(&[("top.msl", top), ("b.msl", b), ("b/c.msl", c), ("x/mod.msl", x)]);
    assert!(disk.diagnostics().is_empty(), "{:?}", disk.diagnostics());
    assert_eq!(disk.dump(), memory.dump());
    assert!(disk.dump().lines().count() > 6);
    assert_eq!(disk.diagnostics(), memory.diagnostics());
    assert_eq!(disk.files().iter().map(|f| f.name()).collect::<Vec<_>>(), memory.files().iter().map(|f| f.name()).collect::<Vec<_>>());
    assert_eq!(disk.files()[0].path(), Some(dir.path().join("top.msl").as_path()));
    assert!(disk.check().is_empty());

    // The builder form reads the same tree.
    let built = Design::builder().root_file(dir.path().join("top.msl")).build();
    assert_eq!(built.dump(), disk.dump());
}

#[test]
fn unreadable_root_is_an_empty_root_with_e0161() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.msl");
    let design = Design::load(&missing);
    assert!(design.root().children().is_empty());
    assert_eq!(design.files().len(), 1);
    let diags = design.diagnostics();
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].diagnostic.code, "E0161");
    assert_eq!(diags[0].file, FileId(0));
    assert!(diags[0].diagnostic.message.contains(&missing.display().to_string()), "{}", diags[0].diagnostic.message);
    assert!(design.check().is_empty());
}

#[test]
fn non_utf8_file_is_a_parse_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "top.msl", "mod b;\nmodule Top() {}\n");
    std::fs::write(dir.path().join("b.msl"), [0x6d, 0xff, 0xfe, 0x20]).unwrap();
    let design = Design::load(dir.path().join("top.msl"));
    let diags = design.diagnostics();
    assert!(!diags.is_empty(), "invalid UTF-8 must be reported");
    assert!(diags.iter().all(|d| d.file == FileId(1) && d.diagnostic.code != "E0161"), "{diags:?}");
    assert!(design.find("b").unwrap().children().is_empty());
    assert!(design.find("Top").is_some());
    assert_eq!(design.files().len(), 2);
    assert!(design.check().is_empty());
}

/// Every file of a multi-file design rebuilds to exactly the tree its text
/// parses to, also when the declaring file's `///` and the loaded file's
/// `//!` say the same words (POM-01 AC 5, the lossless claim per file).
#[test]
fn docs_keep_kind_and_file_across_mod_files() {
    let pairs: &[(&str, &str)] = &[
        ("/// x\nmod b;\n", "//! x\n"),
        ("/// y\nmod b;\n", "//! x\n//! y\n"),
        ("/** x */\nmod b;\n", "/*! x */\n"),
        ("/// x\n/// x\nmod b;\n", "//! x\n/// x\nfn f() {}\n"),
        ("//! x\n/// x\nmod b;\n", "//! x\n//! x\n"),
        ("/// a\nmod b;\n", "//! a\n/// a\nmodule M() {}\n"),
    ];
    for (top, b) in pairs {
        let design = files_design(&[("top.msl", top), ("b.msl", b)]);
        assert!(codes(&design).is_empty(), "{top:?} + {b:?}: {:?}", design.diagnostics());
        for (file, text) in design.files().iter().zip([top, b]) {
            assert_eq!(file.to_ast(), parse(text).file, "{top:?} + {b:?}: {}", file.name());
        }
    }
}
