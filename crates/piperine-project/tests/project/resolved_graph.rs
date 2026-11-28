//! The graph a resolution gave, or the one error it refused with.

use piperine_project::{Options, PackageError, PackageGraph, Resolver};

/// The graph a resolution gave, or the one error it refused with, and a file list read as pairs of text.
pub(crate) struct ResolvedGraph;

impl ResolvedGraph {
    pub(crate) fn files(list: &[(String, String)]) -> Vec<(&str, &str)> {
        list.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
    }

    pub(crate) fn ok(dir: &std::path::Path) -> PackageGraph {
        match Resolver::resolve(dir, Options::default()) {
            Ok(graph) => graph,
            Err(errors) => panic!("resolution failed: {errors:?}"),
        }
    }

    pub(crate) fn only(dir: &std::path::Path) -> PackageError {
        let mut errors = Resolver::resolve(dir, Options::default()).err().expect("the project was accepted");
        assert_eq!(errors.len(), 1, "{errors:?}");
        errors.remove(0)
    }
}
