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
use std::path::PathBuf;

use crate::parse::{ast, parse_str, SourceFile};

// ─────────────────────────────── Error ──────────────────────────────────────

/// Errors produced during `use` resolution.
#[derive(Debug, Clone)]
pub enum ResolveError {
    /// No built-in or file could be found for the given path.
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
///
/// Create with [`Resolver::new`] for a binary-only resolver (no file I/O), or
/// with [`Resolver::with_root`] to also allow project-local file resolution.
pub struct Resolver {
    root: Option<PathBuf>,
    /// Embedded built-in sources, keyed by path segments.
    builtins: HashMap<Vec<String>, &'static str>,
    /// Parsed and cached source files, keyed by path segments.
    cache: HashMap<Vec<String>, SourceFile>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    /// Create a resolver with only built-in (embedded) modules.
    pub fn new() -> Self {
        let mut builtins: HashMap<Vec<String>, &'static str> = HashMap::new();
        builtins.insert(
            vec!["piperine".into(), "capabilities".into()],
            include_str!("../stdlib/capabilities.phdl"),
        );
        builtins.insert(
            vec!["piperine".into(), "collections".into()],
            include_str!("../stdlib/collections.phdl"),
        );
        Self { root: None, builtins, cache: HashMap::new() }
    }

    /// Create a resolver that also searches `root` for file-based modules.
    pub fn with_root(root: PathBuf) -> Self {
        let mut r = Self::new();
        r.root = Some(root);
        r
    }

    /// Items always in scope — stdlib capabilities and collection functions.
    ///
    /// These are injected before every elaboration run; no explicit `use` needed.
    pub fn prelude_items(&mut self) -> Vec<ast::Item> {
        let cap_key: Vec<String> = vec!["piperine".into(), "capabilities".into()];
        let col_key: Vec<String> = vec!["piperine".into(), "collections".into()];
        let mut items = self.load_source(&cap_key)
            .map(|s| s.items.clone())
            .unwrap_or_default();
        items.extend(
            self.load_source(&col_key)
                .map(|s| s.items.clone())
                .unwrap_or_default(),
        );
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

        let source = if let Some(src) = self.builtins.get(path) {
            parse_str(src).map_err(ResolveError::ParseError)?
        } else if let Some(root) = &self.root.clone() {
            let mut file_path = root.clone();
            for seg in path {
                file_path.push(seg);
            }
            file_path.set_extension("phdl");
            let src = std::fs::read_to_string(&file_path)
                .map_err(|e| ResolveError::IoError(e.to_string()))?;
            parse_str(&src).map_err(ResolveError::ParseError)?
        } else {
            return Err(ResolveError::NotFound(path.to_vec()));
        };

        self.cache.insert(path.to_vec(), source);
        Ok(self.cache.get(path).unwrap())
    }
}
