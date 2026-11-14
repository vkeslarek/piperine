//! Scope: crates in the POM (feature `msl-packages`): the crate graph as plain data (PKG-03),
//! and, as later tasks land, the embedded `std` and its prelude (PKG-06), crates in the design
//! (PKG-04), name resolution across crates (PKG-05) and reflection over crates (PKG-07).

use piperine_lang::pom::{CrateGraph, CrateId, Design, DeclId, GraphError, PathId, Reflect, Resolution, is_crate_name};

fn three() -> (CrateGraph, CrateId, CrateId, CrateId) {
    let mut g = CrateGraph::new();
    let a = g.add_crate("a", "a/lib.msl").unwrap();
    let b = g.add_crate("b", "b/lib.msl").unwrap();
    let c = g.add_crate("c", "c/lib.msl").unwrap();
    (g, a, b, c)
}

#[test]
fn graph_keeps_crates_and_edges_as_given() {
    let (mut g, a, b, c) = three();
    assert_eq!((a, b, c), (CrateId(0), CrateId(1), CrateId(2)));
    g.add_dependency(a, "util", b).unwrap();
    g.add_dependency(a, "cells", c).unwrap();
    g.add_dependency(b, "c", c).unwrap();
    let nodes = g.crates();
    assert_eq!(nodes.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(nodes[1].root, std::path::PathBuf::from("b/lib.msl"));
    let edges = |i: usize| nodes[i].dependencies.iter().map(|d| (d.name.as_str(), d.krate)).collect::<Vec<_>>();
    assert_eq!(edges(0), [("util", b), ("cells", c)]);
    assert_eq!(edges(1), [("c", c)]);
    assert!(edges(2).is_empty());
}

#[test]
fn graph_rejects_invalid_and_reserved_names() {
    let (mut g, a, b, _) = three();
    let before = g.clone();
    for name in ["9a", "a-b", "fn", "module", "std", ""] {
        assert!(!is_crate_name(name), "{name:?}");
        assert_eq!(g.add_crate(name, "x.msl"), Err(GraphError::InvalidName { name: name.to_string() }));
        assert_eq!(g.add_dependency(a, name, b), Err(GraphError::InvalidName { name: name.to_string() }));
        assert_eq!(g, before, "{name:?} changed the graph");
    }
    assert!(is_crate_name("util"));
    assert!(is_crate_name("_x9"));
}

#[test]
fn graph_rejects_a_cycle_and_names_it() {
    let (mut g, a, b, c) = three();
    g.add_dependency(a, "b", b).unwrap();
    g.add_dependency(b, "c", c).unwrap();
    let before = g.clone();
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(g.add_dependency(c, "a", a), Err(GraphError::Cycle { names: names(&["a", "b", "c", "a"]) }));
    assert_eq!(g, before);
    assert_eq!(g.add_dependency(a, "me", a), Err(GraphError::Cycle { names: names(&["a", "a"]) }));
    assert_eq!(g, before);
}

#[test]
fn graph_rejects_a_repeated_dependency_name() {
    let (mut g, a, b, c) = three();
    g.add_dependency(a, "util", b).unwrap();
    let before = g.clone();
    assert_eq!(g.add_dependency(a, "util", c), Err(GraphError::DuplicateDependency { krate: "a".into(), name: "util".into() }));
    assert_eq!(g, before);
    // the same name on another crate is fine
    g.add_dependency(b, "util", c).unwrap();
}

#[test]
fn graph_rejects_a_foreign_id() {
    let (mut g, a, _, _) = three();
    let before = g.clone();
    let past = CrateId(3);
    assert_eq!(g.add_dependency(past, "x", a), Err(GraphError::UnknownCrate { id: past }));
    assert_eq!(g.add_dependency(a, "x", past), Err(GraphError::UnknownCrate { id: past }));
    assert_eq!(g.add_dependency(CrateId(u32::MAX), "x", CrateId(u32::MAX)), Err(GraphError::UnknownCrate { id: CrateId(u32::MAX) }));
    assert_eq!(g, before);
}

#[test]
fn piperine_lang_takes_the_graph_as_data() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    for name in ["toml", "git2", "piperine-project"] {
        assert!(!manifest.contains(name), "Cargo.toml names `{name}`");
    }
}

// ---- PKG-06: `std` is embedded and its prelude is implicit (T7) ----

/// Every resolution of the first segment of the single-segment paths named `name`.
fn resolutions(design: &Design, name: &str) -> Vec<Resolution> {
    (0..design.path_count())
        .map(|i| design.path_node(PathId(i as u32)))
        .filter(|p| p.segments.len() == 1 && p.segments[0].name.text == name)
        .map(|p| p.segments[0].res)
        .collect()
}

fn codes(design: &Design) -> Vec<String> {
    design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
}

/// The file name of the declaration a resolution names.
fn declared_in(design: &Design, res: Resolution) -> String {
    let Resolution::Decl(d) = res else { panic!("{res:?} is not a declaration") };
    design.decl(d).file().name().to_string()
}

const STD: &str = "pub mod prelude { pub const P: u32 = 1; const HIDDEN: u32 = 2; }\npub const TOP: u32 = 3;\npub mod other { pub const D: u32 = 4; }\n";

fn with_std(project: &str) -> Design {
    Design::builder().std_source("lib.msl", STD).source("top.msl", project).build()
}

#[test]
fn embedded_std_builds_clean_and_has_a_prelude() {
    let empty = Design::builder().build();
    assert_eq!(codes(&empty), Vec::<String>::new());
    let design = Design::builder().source("top.msl", "").build();
    assert_eq!(codes(&design), Vec::<String>::new());
    let files = design.files();
    assert!(files.last().unwrap().is_prelude(), "the embedded library is the last file");
    let prelude = (0..design.decl_count()).map(|i| DeclId(i as u32)).find(|d| {
        let data = design.decl_data(*d);
        data.name.as_ref().is_some_and(|n| n.text == "prelude") && data.kind.label() == "Namespace" && data.public && design.decl(*d).file().is_prelude()
    });
    assert!(prelude.is_some(), "std declares a pub namespace `prelude`");
}

#[test]
fn prelude_items_resolve_without_an_import() {
    let design = with_std("const X: u32 = P;\nmod m { const Y: u32 = P; }\n");
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = resolutions(&design, "P");
    assert_eq!(found.len(), 2);
    for res in found {
        assert_eq!(declared_in(&design, res), "lib.msl");
    }
}

#[test]
fn only_pub_items_of_std_prelude_are_implicit() {
    for name in ["HIDDEN", "TOP", "D"] {
        let design = with_std(&format!("const X: u32 = {name};\n"));
        assert_eq!(codes(&design), ["E0161"], "{name}");
    }
    assert_eq!(codes(&with_std("const X: u32 = P;\n")), Vec::<String>::new());
}

#[test]
fn std_source_replaces_the_embedded_library() {
    let design = Design::builder()
        .std_source("root.msl", "pub mod prelude;\n")
        .std_source("prelude.msl", "pub const MINE: u32 = 1;\n")
        .source("top.msl", "const X: u32 = MINE;\n")
        .build();
    assert_eq!(codes(&design), Vec::<String>::new());
    let std_files: Vec<String> = design.files().iter().filter(|f| f.is_prelude()).map(|f| f.name().to_string()).collect();
    assert_eq!(std_files, ["root.msl", "prelude.msl"], "the first given file is the root; nothing embedded remains");
    assert_eq!(declared_in(&design, resolutions(&design, "MINE")[0]), "prelude.msl");
}

#[test]
fn a_namespace_item_hides_a_prelude_item() {
    let design = with_std("const P: u32 = 9;\nmod m { const P: u32 = 8; const Z: u32 = P; }\nconst W: u32 = P;\n");
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = resolutions(&design, "P");
    assert_eq!(found.len(), 2);
    for res in found {
        assert_eq!(declared_in(&design, res), "top.msl");
    }
}

#[test]
fn std_files_are_numbered_last() {
    let design = Design::builder().source("top.msl", "mod b;\n").source("b.msl", "").build();
    let flags: Vec<(String, bool)> = design.files().iter().map(|f| (f.name().to_string(), f.is_prelude())).collect();
    assert_eq!(flags[0], ("top.msl".to_string(), false));
    assert_eq!(flags[1], ("b.msl".to_string(), false));
    assert!(flags[2..].iter().all(|(_, std)| *std) && flags.len() > 2, "{flags:?}");
}

#[test]
fn std_files_see_the_prelude() {
    let std = "pub mod prelude { pub const P: u32 = 1; }\npub mod other { pub const Z: u32 = P; }\n";
    let design = Design::builder().std_source("lib.msl", std).source("top.msl", "").build();
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = resolutions(&design, "P");
    assert_eq!(found.len(), 1);
    assert_eq!(declared_in(&design, found[0]), "lib.msl");
}
