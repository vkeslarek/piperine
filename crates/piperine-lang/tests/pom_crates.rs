//! Scope: crates in the POM (feature `msl-packages`): the crate graph as plain data (PKG-03),
//! and, as later tasks land, the embedded `std` and its prelude (PKG-06), crates in the design
//! (PKG-04), name resolution across crates (PKG-05) and reflection over crates (PKG-07).

use piperine_lang::pom::{CrateGraph, CrateId, GraphError, is_crate_name};

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
