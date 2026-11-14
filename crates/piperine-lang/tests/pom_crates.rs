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

// ---- PKG-05 AC 1 to 10: names resolve across crates (T10) ----

/// A design of two crates, `app -> util` (the dependency called `dep`, the crate called `util`),
/// over virtual sources, with the given std.
fn two(app: &str, util: &str, dep: &str, std: Option<&str>) -> Design {
    let mut g = CrateGraph::new();
    let a = g.add_crate("app", "app/lib.msl").unwrap();
    let u = g.add_crate("util", "util/lib.msl").unwrap();
    g.add_dependency(a, dep, u).unwrap();
    let mut b = Design::builder().source("app/lib.msl", app).source("util/lib.msl", util).crates(g);
    if let Some(std) = std {
        b = b.std_source("std/lib.msl", std);
    }
    b.build()
}

/// Every resolution list of the paths written `text` (segments joined by `::`).
fn path_res(design: &Design, text: &str) -> Vec<Vec<Resolution>> {
    (0..design.path_count())
        .map(|i| design.path_node(PathId(i as u32)))
        .filter(|p| p.segments.iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::") == text)
        .map(|p| p.segments.iter().map(|s| s.res).collect())
        .collect()
}

/// The crate name of the declaration a resolution names.
fn krate_of(design: &Design, res: Resolution) -> String {
    let Resolution::Decl(d) = res else { panic!("{res:?} is not a declaration") };
    design.decl(d).krate().name().to_string()
}

#[test]
fn a_dependency_name_starts_a_path() {
    let design = two("const X: u32 = util::K;\n", "pub const K: u32 = 1;\n", "util", None);
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = path_res(&design, "util::K");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0][0], Resolution::Decl(design.crates()[1].root().decl().id()), "the first segment is the root namespace of util");
    assert_eq!(krate_of(&design, found[0][1]), "util");
    assert_eq!(design.decl(match found[0][1] { Resolution::Decl(d) => d, _ => unreachable!() }).name(), Some("K"));
}

#[test]
fn use_forms_of_a_dependency_resolve() {
    let util = "pub mod a { pub const K: u32 = 1; pub const L: u32 = 2; }\n";
    let app = "use util::a::*;\nuse util::a::{K as K2, L as L2};\nuse util as u;\nconst X: u32 = K + K2 + u::a::L + L2;\n";
    let design = two(app, util, "util", None);
    assert_eq!(codes(&design), Vec::<String>::new());
    assert_eq!(krate_of(&design, path_res(&design, "u::a::L")[0][2]), "util");
}

#[test]
fn an_instance_links_to_a_module_of_a_dependency() {
    let app = "use util::cells::Cell;\nmodule Top() { let c = Cell(); }\n";
    let design = two(app, "pub mod cells { pub module Cell() {} }\n", "util", None);
    assert_eq!(codes(&design), Vec::<String>::new());
    let module = design.module("Top").unwrap().instance("c").unwrap().module().unwrap();
    assert_eq!(module.krate().name(), "util");
    assert_eq!(Some(module), design.module("Cell"));
}

#[test]
fn crate_names_the_root_of_the_containing_crate() {
    let design = two("pub const A: u32 = 2;\n", "pub const A: u32 = 1;\npub mod m { pub const B: u32 = crate::A; }\n", "util", None);
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = path_res(&design, "crate::A");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0][0], Resolution::Decl(design.crates()[1].root().decl().id()));
    assert_eq!(krate_of(&design, found[0][1]), "util");
}

#[test]
fn a_renamed_dependency_resolves_by_its_dependency_name() {
    let util = "pub const K: u32 = 1;\n";
    let design = two("const X: u32 = cells::K;\n", util, "cells", None);
    assert_eq!(codes(&design), Vec::<String>::new());
    assert_eq!(krate_of(&design, path_res(&design, "cells::K")[0][1]), "util");
    let design = two("const X: u32 = util::K;\n", util, "cells", None);
    assert_eq!(codes(&design), ["E0161"], "the crate's own name is not in scope");
}

#[test]
fn a_local_name_wins_over_a_dependency_in_a_path() {
    let util = "pub const K: u32 = 1;\npub struct T {}\n";
    for app in [
        "mod util { pub const K: u32 = 5; pub struct T {} }\nconst X: u32 = util::K;\nfn f(x: util::T) {}\n",
        "mod inner { pub const K: u32 = 5; pub struct T {} }\nuse inner as util;\nconst X: u32 = util::K;\nfn f(x: util::T) {}\n",
    ] {
        let design = two(app, util, "util", None);
        assert_eq!(codes(&design), Vec::<String>::new(), "{app}");
        for text in ["util::K", "util::T"] {
            let found = path_res(&design, text);
            assert_eq!(found.len(), 1, "{text}");
            assert_eq!(krate_of(&design, found[0][1]), "app", "{text} in {app}");
        }
    }
}

#[test]
fn a_use_path_that_names_a_local_and_a_dependency_is_e0163() {
    let util = "pub const K: u32 = 1;\n";
    for app in [
        "mod util { pub const K: u32 = 5; }\nuse util::K;\n",
        "mod util { pub const K: u32 = 5; }\nuse util::*;\n",
        "mod util { pub const K: u32 = 5; }\nuse util as u;\n",
        "mod util { pub const K: u32 = 5; }\nfn f() { use util::K; }\n",
        "mod inner { pub const K: u32 = 5; }\nuse inner as util;\nuse util::K;\n",
    ] {
        let design = two(app, util, "util", None);
        assert_eq!(codes(&design), ["E0163"], "{app}: {:?}", design.diagnostics());
        let d = &design.diagnostics()[0].diagnostic;
        let help = d.help.as_deref().unwrap_or("");
        assert!(help.contains("package = \"") && help.contains("crate::util"), "{help}");
        for res in path_res(&design, "util::K").into_iter().chain(path_res(&design, "util")).chain(path_res(&design, "util::*")) {
            assert!(res.iter().all(|r| *r == Resolution::Unresolved), "the use path stays unresolved: {res:?}");
        }
    }
}

#[test]
fn crate_prefix_names_the_local_one_in_a_clash() {
    let design = two("mod util { pub const K: u32 = 5; }\nuse crate::util::K;\nconst X: u32 = K;\n", "pub const K: u32 = 1;\n", "util", None);
    assert_eq!(codes(&design), Vec::<String>::new());
    assert_eq!(krate_of(&design, path_res(&design, "K")[0][0]), "app");
}

const STD_P: &str = "pub mod prelude { pub const P: u32 = 1; pub mod util { pub const K: u32 = 9; } }\n";

#[test]
fn std_is_a_dependency_of_every_crate() {
    let design = two("const X: u32 = std::prelude::P;\n", "pub const Y: u32 = std::prelude::P;\n", "util", Some(STD_P));
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = path_res(&design, "std::prelude::P");
    assert_eq!(found.len(), 2);
    for res in found {
        assert_eq!(res[0], Resolution::Decl(design.std_crate().root().decl().id()));
        assert_eq!(krate_of(&design, res[2]), "std");
    }
}

#[test]
fn the_prelude_reaches_every_crate() {
    let design = two("const X: u32 = P;\n", "pub const Y: u32 = P;\nmod deep { pub const Z: u32 = P; }\n", "util", Some(STD_P));
    assert_eq!(codes(&design), Vec::<String>::new());
    let found = path_res(&design, "P");
    assert_eq!(found.len(), 3);
    for res in found {
        assert_eq!(krate_of(&design, res[0]), "std");
    }
}

#[test]
fn a_dependency_name_hides_a_prelude_item() {
    // `std::prelude` has a `pub mod util`, and `util` is also a dependency: the dependency wins.
    let design = two("const X: u32 = util::K;\n", "pub const K: u32 = 1;\n", "util", Some(STD_P));
    assert_eq!(codes(&design), Vec::<String>::new());
    assert_eq!(krate_of(&design, path_res(&design, "util::K")[0][1]), "util");
}

// ---- PKG-05 AC 11 to 16: pub is the only way across a crate (T11) ----

/// `app -> util -> deep` (each depends on the next under its own name), over virtual sources.
fn chain(app: &str, util: &str, deep: &str) -> Design {
    let mut g = CrateGraph::new();
    let a = g.add_crate("app", "app/lib.msl").unwrap();
    let u = g.add_crate("util", "util/lib.msl").unwrap();
    let d = g.add_crate("deep", "deep/lib.msl").unwrap();
    g.add_dependency(a, "util", u).unwrap();
    g.add_dependency(u, "deep", d).unwrap();
    Design::builder().source("app/lib.msl", app).source("util/lib.msl", util).source("deep/lib.msl", deep).crates(g).build()
}

#[test]
fn a_crate_that_is_no_dependency_is_e0161_with_help() {
    // A dependency of a dependency, and the crate's own name.
    for app in ["const X: u32 = deep::K;\n", "pub const A: u32 = 1;\nconst X: u32 = app::A;\n"] {
        let design = chain(app, "pub const K: u32 = 1;\n", "pub const K: u32 = 1;\n");
        assert_eq!(codes(&design), ["E0161"], "{app}");
        let help = design.diagnostics()[0].diagnostic.help.clone().unwrap_or_default();
        assert_eq!(help, "name it under [dependencies] in Piperine.toml", "{app}");
    }
    // A later segment that is missing gets no such help; neither does a plain unknown name.
    for app in ["const X: u32 = util::Missing;\n", "const X: u32 = nowhere::K;\n"] {
        let design = chain(app, "pub const K: u32 = 1;\n", "");
        assert_eq!(codes(&design), ["E0161"], "{app}");
        assert_eq!(design.diagnostics()[0].diagnostic.help, None, "{app}");
    }
}

#[test]
fn a_private_item_of_another_crate_is_e0161() {
    let design = chain("const X: u32 = util::K;\n", "const K: u32 = 1;\n", "");
    assert_eq!(codes(&design), ["E0161"]);
    assert!(design.diagnostics()[0].diagnostic.message.contains("private"), "{}", design.diagnostics()[0].diagnostic.message);
}

#[test]
fn a_pub_item_in_a_private_namespace_is_e0161() {
    let design = chain("const X: u32 = util::inner::K;\n", "mod inner { pub const K: u32 = 1; }\n", "");
    assert_eq!(codes(&design), ["E0161"]);
}

#[test]
fn super_at_a_crate_root_is_e0161() {
    let design = chain("pub const A: u32 = 1;\n", "pub const K: u32 = super::A;\n", "");
    assert_eq!(codes(&design), ["E0161"]);
    let design = chain("const X: u32 = super::A;\n", "", "");
    assert_eq!(codes(&design), ["E0161"]);
}

#[test]
fn a_glob_of_a_dependency_brings_only_pub_items() {
    let util = "pub const A: u32 = 1;\nconst B: u32 = 2;\n";
    let design = chain("use util::*;\nconst X: u32 = A;\n", util, "");
    assert_eq!(codes(&design), Vec::<String>::new());
    assert_eq!(krate_of(&design, path_res(&design, "A")[0][0]), "util");
    let design = chain("use util::*;\nconst Y: u32 = B;\n", util, "");
    assert_eq!(codes(&design), ["E0161"]);
}

#[test]
fn a_pub_use_re_exports_across_crates() {
    let deep = "pub const K: u32 = 1;\n";
    // app does not depend on deep: it reaches K through util's `pub use`, by path and by import.
    let design = chain("use util::K;\nconst X: u32 = K;\nconst Y: u32 = util::K;\n", "pub use deep::K;\n", deep);
    assert_eq!(codes(&design), Vec::<String>::new());
    for res in path_res(&design, "K").into_iter().chain(path_res(&design, "util::K")) {
        assert_eq!(krate_of(&design, *res.last().unwrap()), "deep");
    }
    // Without `pub` the import is private to util.
    let design = chain("const X: u32 = util::K;\n", "use deep::K;\n", deep);
    assert_eq!(codes(&design), ["E0161"]);
}

// ---- PKG-07 AC 3 to 7: reflection over crates (T12) ----

/// `app -> {zeta: b, alpha: c}` over virtual sources; crate order is `app`, `b`, `c`, `std`.
fn three_crates(app: &str, b: &str, c: &str) -> Design {
    let mut g = CrateGraph::new();
    let a = g.add_crate("app", "app/lib.msl").unwrap();
    let bb = g.add_crate("b", "b/lib.msl").unwrap();
    let cc = g.add_crate("c", "c/lib.msl").unwrap();
    g.add_dependency(a, "zeta", bb).unwrap();
    g.add_dependency(a, "alpha", cc).unwrap();
    Design::builder().source("app/lib.msl", app).source("b/lib.msl", b).source("c/lib.msl", c).crates(g).build()
}

#[test]
fn a_crate_lists_its_declared_dependencies() {
    let design = three_crates("", "", "");
    let app = design.root_crate();
    let deps = app.dependencies();
    assert_eq!(deps.iter().map(|(n, c)| (*n, c.name())).collect::<Vec<_>>(), [("zeta", "b"), ("alpha", "c")]);
    assert!(app.dependency("std").is_none(), "std is implicit and not listed");
    assert_eq!(app.dependency("zeta").unwrap().name(), "b");
    assert_eq!(app.dependency("alpha").unwrap().name(), "c");
    assert!(app.dependency("b").is_none(), "a dependency is found by the name the crate gives it");
    assert!(app.dependency("nope").is_none());
    assert!(design.crates()[1].dependencies().is_empty());
    assert!(design.std_crate().dependencies().is_empty());
}

const KINDS: &str = "pub module {M}() {}\npub fn {F}() -> u32 { 1 }\npub struct {S} { x: u32 }\npub enum {E} { A }\npub trait {T} {}\npub const {K}: u32 = 1;\nimpl {S} { pub const Z: u32 = 2; }\npub mod {N} {}\npub nature {Nat} { value: f64 }\n";

fn kinds(suffix: &str) -> String {
    let mut text = KINDS.to_string();
    for key in ["M", "F", "S", "E", "T", "K", "N", "Nat"] {
        text = text.replace(&format!("{{{key}}}"), &format!("{key}{suffix}"));
    }
    text
}

#[test]
fn collections_list_the_root_crate_and_a_crate_lists_its_own() {
    let design = three_crates(&kinds("app"), &kinds("b"), &kinds("c"));
    let names = |v: Vec<Option<&str>>| v.into_iter().map(|n| n.unwrap_or("-").to_string()).collect::<Vec<_>>();
    assert_eq!(names(design.modules().iter().map(|x| x.name()).collect()), ["Mapp"]);
    assert_eq!(names(design.functions().iter().map(|x| x.name()).collect()), ["Fapp"]);
    assert_eq!(names(design.structs().iter().map(|x| x.name()).collect()), ["Sapp"]);
    assert_eq!(names(design.enums().iter().map(|x| x.name()).collect()), ["Eapp"]);
    assert_eq!(names(design.traits().iter().map(|x| x.name()).collect()), ["Tapp"]);
    assert_eq!(names(design.consts().iter().map(|x| x.name()).collect()), ["Kapp", "Z"]);
    assert_eq!(design.impls().len(), 1);
    assert!(design.impls().iter().all(|i| i.decl().krate() == design.root_crate()));
    assert_eq!(names(design.namespaces().iter().map(|x| x.name()).collect()), ["Napp"]);
    assert_eq!(names(design.natures().iter().map(|x| x.name()).collect()), ["Natapp"]);

    let b = design.krate("b").unwrap();
    assert!(b.decls().iter().all(|d| d.krate() == b));
    assert!(b.decls().iter().any(|d| d.name() == Some("Fb")));
    assert!(!b.decls().iter().any(|d| d.name() == Some("Fapp") || d.name() == Some("Fc")));
    assert_eq!(b.modules().iter().map(|m| m.name().unwrap()).collect::<Vec<_>>(), ["Mb"]);
    assert_eq!(design.root_crate().modules().iter().map(|m| m.name().unwrap()).collect::<Vec<_>>(), ["Mapp"]);
    assert_eq!(design.std_crate().modules().iter().filter(|m| m.name() == Some("Mapp")).count(), 0);
}

#[test]
fn by_name_lookups_prefer_the_root_crate_then_crate_order() {
    let home = |d: Option<piperine_lang::pom::Decl<'_>>| d.map(|d| d.krate().name().to_string());
    let some = |s: &str| Some(s.to_string());
    // The name in every crate: the root crate's.
    let src = "pub module X() {}\npub fn f() -> u32 { 1 }\npub struct S { x: u32 }\npub nature Nat { value: f64 }\n";
    let design = three_crates(src, src, src);
    assert_eq!(home(design.module("X").map(|m| m.decl())), some("app"));
    assert_eq!(home(design.function("f").map(|m| m.decl())), some("app"));
    assert_eq!(home(design.struct_named("S").map(|m| m.decl())), some("app"));
    assert_eq!(home(design.nature("Nat").map(|m| m.decl())), some("app"));
    // Not in the root crate: the first crate in order, which is `b`, not the dependency named `alpha`.
    let design = three_crates("", src, src);
    assert_eq!(home(design.module("X").map(|m| m.decl())), some("b"));
    assert_eq!(home(design.function("f").map(|m| m.decl())), some("b"));
    assert_eq!(home(design.struct_named("S").map(|m| m.decl())), some("b"));
    assert_eq!(home(design.nature("Nat").map(|m| m.decl())), some("b"));
    assert_eq!(home(design.find("X")), some("b"));
    // Only the later dependency has it.
    let design = three_crates("", "", src);
    assert_eq!(home(design.module("X").map(|m| m.decl())), some("c"));
    // A dependency before std: a name in `c` and in std is `c`'s.
    let std = "pub mod prelude {}\npub module X() {}\n";
    let mut g = CrateGraph::new();
    let a = g.add_crate("app", "app/lib.msl").unwrap();
    let c = g.add_crate("c", "c/lib.msl").unwrap();
    g.add_dependency(a, "c", c).unwrap();
    let design = Design::builder().std_source("std/lib.msl", std).source("app/lib.msl", "").source("c/lib.msl", "pub module X() {}\n").crates(g).build();
    assert_eq!(home(design.module("X").map(|m| m.decl())), some("c"));
    // Only std has it: std's.
    let design = Design::builder().std_source("std/lib.msl", std).source("app/lib.msl", "").build();
    assert_eq!(home(design.module("X").map(|m| m.decl())), some("std"));
}

#[test]
fn crates_are_found_by_name() {
    let design = three_crates("", "", "");
    assert_eq!(design.crates().iter().map(|c| c.name()).collect::<Vec<_>>(), ["app", "b", "c", "std"]);
    assert_eq!(design.krate("b").unwrap().id(), CrateId(1));
    assert_eq!(design.krate("std").unwrap(), design.std_crate());
    assert_eq!(design.krate("app").unwrap(), design.root_crate());
    assert!(design.krate("zeta").is_none(), "a crate is found by its own name, not a dependency name");
    assert!(design.krate("").is_none());
    // The first of a name: two crates called `b`.
    let mut g = CrateGraph::new();
    let a = g.add_crate("app", "app/lib.msl").unwrap();
    let b1 = g.add_crate("b", "b1/lib.msl").unwrap();
    let b2 = g.add_crate("b", "b2/lib.msl").unwrap();
    g.add_dependency(a, "one", b1).unwrap();
    g.add_dependency(a, "two", b2).unwrap();
    let design = Design::builder().source("app/lib.msl", "").source("b1/lib.msl", "").source("b2/lib.msl", "").crates(g).build();
    assert_eq!(design.krate("b").unwrap().id(), CrateId(1));
}

#[test]
fn crate_find_inverts_qualified_name() {
    let src = "pub mod a { pub module M(out o: f64) { let w = 1.0; } pub mod b { pub fn deep() -> u32 { 1 } } }\npub struct S { x: u32 }\nimpl S { pub const X: u32 = 1; }\npub enum E { V }\n";
    let design = three_crates("", src, "pub struct S { y: u32 }\n");
    let b = design.krate("b").unwrap();
    let mut seen = 0;
    for d in b.decls() {
        if d.name().is_none() {
            continue;
        }
        let q = d.qualified_name();
        if q.is_empty() {
            continue;
        }
        let found = b.find(&q).unwrap_or_else(|| panic!("`{q}` not found"));
        assert_eq!(found.qualified_name(), q);
        assert_eq!(found.krate(), b, "`{q}`");
        seen += 1;
    }
    assert!(seen >= 8, "{seen}");
    assert_eq!(b.find("a::M").unwrap().krate(), b);
    assert_eq!(b.find("a::M.w").unwrap().qualified_name(), "a::M.w");
    assert_eq!(b.find("impl S.X").unwrap().krate(), b);
    // Another crate's declaration is not found through this crate, even under the same name.
    let c = design.krate("c").unwrap();
    assert!(c.find("a::M").is_none());
    assert_eq!(c.find("S").unwrap().krate(), c);
    assert!(b.find("nope").is_none());
    // `qualified_name` is relative to the declaration's own crate.
    assert_eq!(b.find("a::M").unwrap().qualified_name(), "a::M");
}
