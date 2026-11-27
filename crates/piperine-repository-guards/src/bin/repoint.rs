//! `repoint <map.tsv>...`: applies the repoint maps of tasks to the feature files, the Book's
//! inventory and the prose files that name tests.

use piperine_repository_guards::{RepointMap, Repointer, SourceTree};

/// Applies the maps named on the command line, stopping at the first one that fails.
fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: repoint <map.tsv>...");
        std::process::exit(2);
    }
    let repointer = Repointer::new(SourceTree::workspace_root());
    for path in paths {
        let result = RepointMap::read(std::path::Path::new(&path)).and_then(|map| repointer.apply(&map));
        if let Err(message) = result {
            eprintln!("repoint: {path}: {message}");
            std::process::exit(1);
        }
    }
}
