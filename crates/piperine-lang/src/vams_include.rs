//! [`SourceMapIncludeResolver`] — the `` `include`` search-path resolver
//! `crate::preprocessor::IncludeResolver` needs (LRM ch. 10,
//! VAMS-16). `piperine-vams` is purity-gated (no filesystem I/O in the
//! parse crate); this is the boundary where that I/O happens, reusing
//! [`SourceMap`]'s existing namespace/root-path search order rather than
//! inventing a second one.

use std::fs;
use std::path::PathBuf;

use crate::preprocessor::IncludeResolver;

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

/// Legacy Cadence-era include names, still ubiquitous in real-world
/// Verilog-A sources, that alias LRM Annex D's own file names
/// (`disciplines.vams`/`constants.vams`) — not an LRM production
/// (Annex D names only the `.vams` files), but common enough in the wild
/// that failing loud on it just sends every such source back to its
/// author for a one-line edit. Checked only as a fallback, after every
/// namespace/root/as-is candidate for the literal requested name has
/// already missed, so a project that ships its own `discipline.h` is
/// never shadowed by this alias.
const LEGACY_INCLUDE_ALIASES: &[(&str, &str)] = &[
    ("discipline.h", "disciplines.vams"),
    ("disciplines.h", "disciplines.vams"),
    ("constants.h", "constants.vams"),
];

impl IncludeResolver for SourceMapIncludeResolver {
    fn resolve(&self, path: &str) -> Option<String> {
        let candidates = |p: &str| {
            self.namespace_paths
                .iter()
                .map(|ns| ns.join(p))
                .chain(std::iter::once(self.root_path.join(p)))
                .chain(std::iter::once(PathBuf::from(p)))
                .collect::<Vec<_>>()
        };
        if let Some(found) = candidates(path).iter().find_map(|c| fs::read_to_string(c).ok()) {
            return Some(found);
        }
        let basename = path.rsplit('/').next().unwrap_or(path);
        let alias = LEGACY_INCLUDE_ALIASES.iter().find(|(legacy, _)| *legacy == basename)?.1;
        candidates(alias).iter().find_map(|c| fs::read_to_string(c).ok())
    }
}
