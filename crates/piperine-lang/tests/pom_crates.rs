//! Scope: crates in the POM (feature `msl-packages`): the crate graph as plain data (PKG-03),
//! and, as later tasks land, the embedded `std` and its prelude (PKG-06), crates in the design
//! (PKG-04), name resolution across crates (PKG-05) and reflection over crates (PKG-07).

use piperine_lang::pom::{CrateGraph, CrateId, Design, DeclId, FileId, GraphError, PathId, Reflect, Resolution, is_crate_name};

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
    assert!(files.last().unwrap().krate().is_std(), "the embedded library is the last file");
    let prelude = (0..design.decl_count()).map(|i| DeclId(i as u32)).find(|d| {
        let data = design.decl_data(*d);
        data.name.as_ref().is_some_and(|n| n.text == "prelude") && data.kind.label() == "Namespace" && data.public && design.decl(*d).file().krate().is_std()
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
    let std_files: Vec<String> = design.files().iter().filter(|f| f.krate().is_std()).map(|f| f.name().to_string()).collect();
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
    let flags: Vec<(String, bool)> = design.files().iter().map(|f| (f.name().to_string(), f.krate().is_std())).collect();
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

// ---- PKG-04 AC 1 and 2, PKG-07 AC 1 and 2: the design holds its crates (T8) ----

#[test]
fn single_source_design_has_a_root_crate_named_by_its_file() {
    let design = Design::builder().source("top.msl", "module M() {}\n").build();
    let root = design.root_crate();
    assert_eq!(root.name(), "top");
    assert_eq!(root.root().file().id(), FileId(0));
    assert_eq!(design.root(), root.root().decl());
    // A stem that is no identifier: every other character becomes `_`, a digit gets a leading `_`.
    for (file, name) in [("9.msl", "_9"), ("a-b.msl", "a_b"), ("x y.msl", "x_y"), ("9a-b.msl", "_9a_b")] {
        let design = Design::builder().source(file, "").build();
        assert_eq!(design.root_crate().name(), name, "{file}");
    }
    // From disk, the same rule over the file name.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("front-end.msl"), "").unwrap();
    assert_eq!(Design::load(dir.path().join("front-end.msl")).root_crate().name(), "front_end");
}

#[test]
fn every_design_holds_std_last() {
    for design in [
        Design::builder().build(),
        Design::builder().source("top.msl", "").build(),
        Design::builder().std_source("root.msl", "pub mod prelude {}\n").source("top.msl", "").build(),
    ] {
        let crates = design.crates();
        let last = crates.last().unwrap();
        assert_eq!(last.name(), "std");
        assert!(last.is_std() && !last.is_root());
        assert_eq!(*last, design.std_crate());
        assert_eq!(crates.iter().filter(|c| c.is_std()).count(), 1);
        assert_eq!(crates.first().unwrap(), &design.root_crate());
    }
    let design = Design::builder().std_source("root.msl", "pub mod prelude {}\n").source("top.msl", "").build();
    assert_eq!(design.std_crate().files().iter().map(|f| f.name().to_string()).collect::<Vec<_>>(), ["root.msl"]);
    assert_eq!(design.std_crate().root().file().name(), "root.msl");
}

#[test]
fn a_crate_reports_its_name_root_and_files() {
    let design = Design::builder().source("top.msl", "mod b;\n").source("b.msl", "pub module Inner() {}\n").build();
    let root = design.root_crate();
    assert_eq!((root.name(), root.is_root(), root.is_std()), ("top", true, false));
    assert_eq!(root.id(), CrateId(0));
    assert_eq!(root.files().iter().map(|f| f.name().to_string()).collect::<Vec<_>>(), ["top.msl", "b.msl"]);
    assert_eq!(root.root().file().name(), "top.msl");
    let std = design.std_crate();
    assert_eq!((std.name(), std.is_root(), std.is_std(), std.id()), ("std", false, true, CrateId(1)));
    assert_eq!(std.files().iter().map(|f| f.name().to_string()).collect::<Vec<_>>(), ["std/lib.msl", "prelude.msl"]);
    // Every file belongs to exactly one crate.
    assert_eq!(root.files().len() + std.files().len(), design.files().len());
}

#[test]
fn declarations_and_files_know_their_crate() {
    let design = Design::builder().source("top.msl", "mod b;\n").source("b.msl", "pub module Inner() {}\n").build();
    let inner = design.module("Inner").unwrap();
    assert_eq!(inner.krate(), design.root_crate());
    assert_eq!(inner.file().krate(), design.root_crate());
    for file in design.files() {
        assert_eq!(file.krate().is_std(), file.name() == "std/lib.msl" || file.name() == "prelude.msl", "{}", file.name());
    }
    let design = Design::builder().std_source("lib.msl", "pub mod prelude { pub const P: u32 = 1; }\n").source("top.msl", "").build();
    let std_decl = design.decls().into_iter().find(|d| d.name() == Some("P")).unwrap();
    assert_eq!(std_decl.krate(), design.std_crate());
}

// ---- PKG-04 AC 3 to 7: a design over a crate graph (T9) ----

/// `app -> util` with the roots `app/lib.msl` and `util/lib.msl`, under `base`.
fn app_util(base: &std::path::Path) -> CrateGraph {
    let mut g = CrateGraph::new();
    let app = g.add_crate("app", base.join("app/lib.msl")).unwrap();
    let util = g.add_crate("util", base.join("util/lib.msl")).unwrap();
    g.add_dependency(app, "util", util).unwrap();
    g
}

fn util_sources(builder: piperine_lang::pom::DesignBuilder) -> piperine_lang::pom::DesignBuilder {
    builder
        .source("app/lib.msl", "mod local;\n")
        .source("app/local.msl", "pub module Local() {}\n")
        .source("util/lib.msl", "pub mod cells;\n")
        .source("util/cells.msl", "pub module Cell() {}\nmod deep;\n")
        .source("util/cells/deep.msl", "pub module Deep() {}\n")
}

fn file_names(c: piperine_lang::pom::Crate<'_>) -> Vec<String> {
    c.files().iter().map(|f| f.name().to_string()).collect()
}

#[test]
fn graph_crates_become_design_crates_in_order() {
    let design = util_sources(Design::builder()).crates(app_util(std::path::Path::new(""))).build();
    assert_eq!(codes(&design), Vec::<String>::new());
    let crates = design.crates();
    assert_eq!(crates.iter().map(|c| c.name()).collect::<Vec<_>>(), ["app", "util", "std"]);
    assert!(crates[0].is_root() && !crates[1].is_root() && crates[2].is_std());
    // Files are numbered crate by crate in graph order, std last.
    assert_eq!(file_names(crates[0]), ["app/lib.msl", "local.msl"]);
    assert_eq!(file_names(crates[1]), ["util/lib.msl", "cells.msl", "deep.msl"]);
    let json = serde_json::to_value(&design).unwrap();
    assert_eq!(json["crates"][0]["dependencies"], serde_json::json!([{ "name": "util", "krate": 1 }]));
    assert_eq!(json["crates"][1]["dependencies"], serde_json::json!([]));
    assert_eq!(json["crates"][0]["name"], "app");
    // An empty graph is no graph.
    let plain = Design::builder().crates(CrateGraph::new()).source("top.msl", "").build();
    assert_eq!(plain.crates().iter().map(|c| c.name()).collect::<Vec<_>>(), ["top", "std"]);
}

#[test]
fn mod_files_load_beside_their_own_crate_root() {
    let design = util_sources(Design::builder()).crates(app_util(std::path::Path::new(""))).build();
    assert_eq!(codes(&design), Vec::<String>::new());
    for (module, file, krate) in [("Local", "local.msl", "app"), ("Cell", "cells.msl", "util"), ("Deep", "deep.msl", "util")] {
        let m = design.module(module).unwrap();
        assert_eq!((m.file().name(), m.krate().name()), (file, krate), "{module}");
    }
}

#[test]
fn an_unreadable_crate_root_is_one_e0161() {
    let design = Design::builder().source("app/lib.msl", "module A() {}\n").crates(app_util(std::path::Path::new(""))).build();
    let found: Vec<_> = design.diagnostics().iter().collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].diagnostic.code, "E0161");
    assert!(found[0].diagnostic.message.contains("util/lib.msl"), "{}", found[0].diagnostic.message);
    let util = design.crates()[1];
    assert_eq!(found[0].file, util.root().file().id());
    assert_eq!(util.name(), "util");
    assert!(util.root().items().is_empty());
    assert!(design.module("A").is_some(), "the other crates still build");
    // The same fault from disk.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("app")).unwrap();
    std::fs::write(dir.path().join("app/lib.msl"), "module A() {}\n").unwrap();
    let disk = Design::load_crates(app_util(dir.path()));
    assert_eq!(codes(&disk), ["E0161"]);
    assert!(disk.diagnostics()[0].diagnostic.message.contains("util"));
}

#[test]
fn every_crate_keeps_its_authored_tree() {
    let design = util_sources(Design::builder()).crates(app_util(std::path::Path::new(""))).build();
    for file in design.files() {
        assert_eq!(file.to_ast(), piperine_lang::msl::parser::parse(file.text()).file, "{}", file.name());
    }
    // The declarations of each crate hang under that crate's own root namespace.
    let (app, util) = (design.crates()[0], design.crates()[1]);
    let names = |c: piperine_lang::pom::Crate<'_>| c.root().items().iter().filter_map(|d| d.name().map(str::to_string)).collect::<Vec<_>>();
    assert_eq!(names(app), ["local"]);
    assert_eq!(names(util), ["cells"]);
    for module in ["Local", "Cell", "Deep"] {
        let mut at = design.module(module).unwrap().decl();
        while let Some(parent) = at.declared_in() {
            at = parent;
        }
        let root_of_its_crate = at.krate().root().decl();
        assert_eq!(at, root_of_its_crate, "{module}");
    }
    assert_eq!(design.module("Cell").unwrap().decl().declared_in().unwrap().declared_in().unwrap(), util.root().decl());
}

#[test]
fn disk_and_memory_graphs_build_the_same_design() {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in [
        ("app/lib.msl", "mod local;\n"),
        ("app/local.msl", "pub module Local() {}\n"),
        ("util/lib.msl", "pub mod cells;\n"),
        ("util/cells.msl", "pub module Cell() {}\nmod deep;\n"),
        ("util/cells/deep.msl", "pub module Deep() {}\n"),
    ] {
        std::fs::create_dir_all(dir.path().join(path).parent().unwrap()).unwrap();
        std::fs::write(dir.path().join(path), text).unwrap();
    }
    let disk = Design::load_crates(app_util(dir.path()));
    let memory = util_sources(Design::builder()).crates(app_util(std::path::Path::new(""))).build();
    assert_eq!(disk.dump(), memory.dump());
    assert_eq!(disk.crates().iter().map(|c| (c.name().to_string(), c.files().len())).collect::<Vec<_>>(), memory.crates().iter().map(|c| (c.name().to_string(), c.files().len())).collect::<Vec<_>>());
    assert_eq!(codes(&disk), codes(&memory));
}
