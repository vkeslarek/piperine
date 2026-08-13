//! [`SourceMapIncludeResolver`] — the `` `include`` search-path resolver
//! `piperine_vams::preprocessor::IncludeResolver` needs (LRM ch. 10,
//! VAMS-16). `piperine-vams` is purity-gated (no filesystem I/O in the
//! parse crate); this is the boundary where that I/O happens, reusing
//! [`SourceMap`]'s existing namespace/root-path search order rather than
//! inventing a second one.

use std::fs;
use std::path::PathBuf;

use piperine_vams::preprocessor::IncludeResolver;

use crate::source_map::SourceMap;

/// Resolves `` `include "path"`` against a [`SourceMap`]'s namespaces and
/// root path. Tries, in order: each registered namespace joined with
/// `path`, then `root_path` joined with `path`, then `path` taken as-is
/// (absolute or relative-to-cwd). Returns `None` (fail loud at the
/// preprocessor, VAMS-16) when nothing resolves.
pub struct SourceMapIncludeResolver {
    root_path: PathBuf,
    namespace_paths: Vec<PathBuf>,
}

impl SourceMapIncludeResolver {
    pub fn new(source_map: &SourceMap) -> Self {
        Self {
            root_path: source_map.root_path.clone(),
            namespace_paths: source_map.namespaces.values().cloned().collect(),
        }
    }
}

impl IncludeResolver for SourceMapIncludeResolver {
    fn resolve(&self, path: &str) -> Option<String> {
        self.namespace_paths
            .iter()
            .map(|ns| ns.join(path))
            .chain(std::iter::once(self.root_path.join(path)))
            .chain(std::iter::once(PathBuf::from(path)))
            .find_map(|candidate| fs::read_to_string(&candidate).ok())
    }
}
