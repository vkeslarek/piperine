//! # `use` declaration resolution
//!
//! Turns `use foo::bar;` declarations in a [`SourceFile`] into flat item lists
//! before the elaborator runs.  The elaborator never sees `UseDecl` items.
//!
//! ## Resolution order
//!
//! 1. **Built-ins** — paths under `piperine::` are resolved from sources embedded
//!    in the binary via `include_str!`.  No file I/O.
//! 2. **File-based** — any other path is mapped to `{root}/{seg0}/{seg1}.phdl`
//!    relative to the configured project root.
//!
//! ## Prelude
//!
//! [`Resolver::prelude_items`] returns the standard library items that are
//! injected into every compilation unit automatically (analogous to Rust's
//! `std::prelude`).  They do **not** require an explicit `use` declaration.
//!
//! ## Transitive `use`
//!
//! Resolved files may themselves contain `use` declarations.  [`Resolver`]
//! expands them recursively and tracks seen paths to handle diamond dependencies.
//!
//! ## Future: package management
//!
//! The first path segment is the package name.  Today the only known package is
//! `piperine` (the built-in stdlib).  When a package registry is added, the
//! resolver will look up unknown first segments there before falling back to
//! file-based resolution.

use std::collections::{HashMap, HashSet};

use crate::parse::{ast, parse_str, SourceFile};
use crate::source_map::SourceMap;

// ─────────────────────────────── Error ──────────────────────────────────────

/// Errors produced during `use` resolution.
#[derive(Debug, Clone)]
pub enum ResolveError {
    /// No file could be found for the given path.
    #[allow(dead_code)]
    NotFound(Vec<String>),
    /// The resolved file contained a parse error.
    ParseError(String),
    /// A file-system read failed.
    IoError(String),
    /// A `use` cycle was detected (should not occur after dedup, kept for safety).
    Cycle(Vec<String>),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::NotFound(p) => write!(f, "module not found: `{}`", p.join("::")),
            ResolveError::ParseError(e) => write!(f, "parse error in resolved module: {e}"),
            ResolveError::IoError(e) => write!(f, "I/O error resolving module: {e}"),
            ResolveError::Cycle(p) => write!(f, "use cycle detected at `{}`", p.join("::")),
        }
    }
}

// ─────────────────────────────── Resolver ───────────────────────────────────

/// Resolves `use` declarations and provides the standard-library prelude.
pub struct Resolver<'a> {
    source_map: &'a SourceMap,
    /// Parsed and cached source files, keyed by path segments.
    cache: HashMap<Vec<String>, SourceFile>,
}

impl<'a> Resolver<'a> {
    /// Create a resolver with the given SourceMap.
    pub fn new(source_map: &'a SourceMap) -> Self {
        Self {
            source_map,
            cache: HashMap::new(),
        }
    }

    /// Items always in scope, loaded from prelude_path if provided.
    pub fn prelude_items(&mut self) -> Vec<ast::Item> {
        let mut items = Vec::new();
        
        // Load the standard library built-ins dynamically if they resolve.
        // We ignore errors so that a bare-bones SourceMap doesn't panic.
        let cap_key = vec!["piperine".to_string(), "capabilities".to_string()];
        if let Ok(src) = self.load_source(&cap_key) {
            items.extend(src.items.clone());
        }
        
        let col_key = vec!["piperine".to_string(), "collections".to_string()];
        if let Ok(src) = self.load_source(&col_key) {
            items.extend(src.items.clone());
        }

        let pre_key = vec!["piperine".to_string(), "prelude".to_string()];
        if let Ok(src) = self.load_source(&pre_key) {
            items.extend(src.items.clone());
        }

        if let Some(prelude_path) = &self.source_map.prelude_path {
            // Load custom prelude dynamically
            if let Ok(src) = std::fs::read_to_string(prelude_path)
                && let Ok(source) = parse_str(&src) {
                    items.extend(source.items);
                }
        }
        items
    }

    /// Expand all `use` declarations in `source` (transitively) into a flat
    /// item list with no `UseDecl` items.
    ///
    /// Diamond dependencies are handled via `seen` deduplication.
    pub fn expand(&mut self, source: SourceFile) -> Result<Vec<ast::Item>, ResolveError> {
        let mut seen: HashSet<Vec<String>> = HashSet::new();
        self.expand_inner(source, &mut seen)
    }

    /// Recursively expand `use` declarations in a source file, tracking
    /// already-visited paths in `seen` to break diamond dependencies.
    fn expand_inner(
        &mut self,
        source: SourceFile,
        seen: &mut HashSet<Vec<String>>,
    ) -> Result<Vec<ast::Item>, ResolveError> {
        let mut result = Vec::new();
        for item in source.items {
            match item {
                ast::Item::UseDecl(path) => {
                    if seen.contains(&path.segments) {
                        continue; // already expanded; skip (diamond dependency)
                    }
                    seen.insert(path.segments.clone());
                    let resolved = self.load_source(&path.segments)?.clone();
                    result.extend(self.expand_inner(resolved, seen)?);
                }
                other => result.push(other),
            }
        }
        Ok(result)
    }

    /// Load (and cache) a source file for the given path.
    fn load_source(&mut self, path: &[String]) -> Result<&SourceFile, ResolveError> {
        if self.cache.contains_key(path) {
            return Ok(self.cache.get(path).unwrap());
        }

        if path.is_empty() {
            return Err(ResolveError::NotFound(path.to_vec()));
        }

        let first_seg = &path[0];
        let mut file_path = if let Some(base) = self.source_map.namespaces.get(first_seg) {
            let mut p = base.clone();
            for seg in &path[1..] {
                p.push(seg);
            }
            p
        } else {
            let mut p = self.source_map.root_path.clone();
            for seg in path {
                p.push(seg);
            }
            p
        };

        file_path.set_extension("phdl");
        let src = std::fs::read_to_string(&file_path)
            .map_err(|e| ResolveError::IoError(format!("{}: {}", file_path.display(), e)))?;
        let source = parse_str(&src).map_err(|e| ResolveError::ParseError(e.to_string()))?;

        self.cache.insert(path.to_vec(), source);
        Ok(self.cache.get(path).unwrap())
    }
}
