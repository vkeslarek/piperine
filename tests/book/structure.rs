//! Scope: chapter 21 of the Book, "How Piperine is organized" (feature `root-e2e`, RE-03): the sentences
//! of the crate table that state what a crate knows or does, held to the manifests of the crates and to
//! the compiled backend's cache, and the one about the command line (a `binary` row). The chapter has no
//! block. Each test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod manifests;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use piperine::{Overrides, Project, Session};
use piperine_transpile::Settings;
use support::faulty::Faulty;
use support::manifests::Manifests;
use support::model::Model;

/// A session opened on the compiled backend over a cache directory the test owns, so two openings can
/// be compared.
struct Compilation;

impl Compilation {
    /// Opens a session of `model` on the compiled backend with `cache` as its cache directory.
    fn open(model: &Model, cache: &Path) {
        let project = Project::file(model.opened_path()).unwrap_or_else(|failure| panic!("{failure}"));
        let overrides = Overrides { backend: Some("rust".to_string()), ..Faulty::operating_point_of("Top") };
        let spec = project.request(None, &overrides).unwrap_or_else(|failure| panic!("{failure}"));
        Session::open(&project, &spec, &Settings { rustc: None, cache: cache.to_path_buf() }).unwrap_or_else(|failure| panic!("{failure}"));
    }

    /// Every file under `directory` with the time it was last written, by path.
    fn written_files(directory: &Path) -> Vec<(PathBuf, SystemTime)> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(directory).expect("a readable cache directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                files.extend(Self::written_files(&path));
            } else {
                let modified = std::fs::metadata(&path).and_then(|meta| meta.modified()).expect("a modification time");
                files.push((path, modified));
            }
        }
        files.sort();
        files
    }
}

#[test]
fn analog_and_digital_meet_only_in_the_owner_of_time() {
    // structure~00d9ae38 "Everything that crosses between analog and digital is an event the owner of time
    // carries.": the analog solver knows nothing digital and the digital solver nothing analog (neither
    // lists the other), and the owner, `piperine-solver`, lists both
    assert!(!Manifests::depends_on("piperine-analog-solver", "piperine-digital-solver"), "the analog side lists the digital side");
    assert!(!Manifests::depends_on("piperine-digital-solver", "piperine-analog-solver"), "the digital side lists the analog side");
    assert!(Manifests::depends_on("piperine-solver", "piperine-analog-solver") && Manifests::depends_on("piperine-solver", "piperine-digital-solver"), "the owner of time lists both");
}

#[test]
fn the_intermediate_form_crate_knows_neither_the_language_nor_the_solver() {
    // structure~b45cc292 "It knows neither the language nor the solver, and prints in a stable form for
    // reading.": `piperine-ir` lists no crate of this repository (the stable print is held by
    // compiler~696ad77d in `book_compiler`)
    let in_tree: Vec<String> = Manifests::dependencies_of("piperine-ir").into_iter().filter(|name| name.starts_with("piperine-")).collect();
    assert!(in_tree.is_empty(), "piperine-ir lists {in_tree:?}");
}

#[test]
fn the_compiled_backend_compiles_once_and_keeps_the_result_in_its_cache() {
    // structure~f9cbf76a "It prints the design as Rust, compiles it once with rustc, keeps the result in a
    // cache and loads it.": a second session of the same design over the same cache builds nothing new
    let model = Model::written("m.msl", "module Top() { signal a: Electrical; }\nanalog for Top { v(a) +> 1.0; }\n");
    let cache = tempfile::tempdir().expect("a cache directory");
    Compilation::open(&model, cache.path());
    let first = Compilation::written_files(cache.path());
    assert!(first.iter().any(|(path, _)| path.ends_with("design.rs")) && first.iter().any(|(path, _)| path.ends_with("libdesign.so")), "the first session printed the source and built the library: {first:?}");
    Compilation::open(&model, cache.path());
    assert_eq!(Compilation::written_files(cache.path()), first, "the second session loaded what the first one kept: no file added, none written again");
}
