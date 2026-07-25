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
    /// Item provenance recorded during expansion: declared item name → the
    /// package it came from (the first `use`-path segment). Root-source
    /// items are not recorded — absence means "this project". Consumed by
    /// [`crate::pom::Project::origins`] after elaboration.
    origins: HashMap<String, String>,
    /// Item-name → the real on-disk file it was declared in (BUG-1/
    /// LSB-01..03). Populated for prelude items (hardcoded
    /// `CARGO_MANIFEST_DIR`-relative paths for the embedded headers) and for
    /// `use`-loaded items (the real path already computed by
    /// [`Self::load_source`]). Consumed by
    /// [`crate::pom::Project::item_file`] after elaboration.
    item_files: HashMap<String, PathBuf>,
    /// Resolved on-disk path for each `use`-path segment list already
    /// loaded via [`Self::load_source`] — lets [`Self::expand_inner`] tag
    /// [`Self::item_files`] without recomputing path resolution.
    file_paths: HashMap<Vec<String>, PathBuf>,
}

impl<'a> Resolver<'a> {
    /// Create a resolver with the given SourceMap.
    pub fn new(source_map: &'a SourceMap) -> Self {
        Self {
            source_map,
            cache: HashMap::new(),
            origins: HashMap::new(),
            item_files: HashMap::new(),
            file_paths: HashMap::new(),
        }
    }

    /// The item-name → package provenance recorded by [`expand`](Self::expand)
    /// (and by the prelude, recorded as `piperine`).
    pub fn take_origins(&mut self) -> HashMap<String, String> {
        std::mem::take(&mut self.origins)
    }

    /// The item-name → real on-disk declaring file recorded by
    /// [`prelude_items`](Self::prelude_items) and [`expand`](Self::expand).
    pub fn take_item_files(&mut self) -> HashMap<String, PathBuf> {
        std::mem::take(&mut self.item_files)
    }

    /// Tag every named item in `items` with `path` in [`Self::item_files`].
    fn tag_item_files(&mut self, items: &[ast::Item], path: &str) {
        self.tag_item_files_owned(items, PathBuf::from(path));
    }

    /// Owned-`PathBuf` variant of [`Self::tag_item_files`] (avoids
    /// re-parsing a `&str` path for dynamically-resolved on-disk files).
    fn tag_item_files_owned(&mut self, items: &[ast::Item], path: PathBuf) {
        for item in items {
            if let Some(name) = item.name() {
                self.item_files.insert(name.to_string(), path.clone());
            }
        }
    }

    /// Items always in scope, loaded from prelude_path if provided.
    pub fn prelude_items(&mut self) -> Vec<ast::Item> {
        let mut items = Vec::new();

        // `types` (declared-language-surface DLS-17) loads first — the
        // seven primitive value types every other header's field/param
        // types refer to. Embedded via `include_str!` (not the on-disk,
        // `SourceMap`-relative `load_source` used below) because these
        // types must be load-bearing everywhere regardless of the caller's
        // working directory or `SourceMap` configuration — unlike
        // capabilities/collections/prelude, which are optional stdlib
        // sugar today, every module in the workspace implicitly depends on
        // `Real`/`Integer`/etc. resolving. The embedded text is the exact
        // on-disk `headers/types.phdl` (kept in sync by the compiler).
        if let Ok(source) = parse_str(include_str!("../headers/types.phdl")) {
            self.tag_item_files(&source.items, concat!(env!("CARGO_MANIFEST_DIR"), "/headers/types.phdl"));
            items.extend(source.items);
        }

        // `math` (declared-language-surface DLS-18) — the libm intrinsics
        // (`sin`, `pow`, …). Embedded the same way as `types` above: math
        // functions are called pervasively across every stdlib device
        // model (`headers/spice/*.phdl`), so they must resolve regardless
        // of the caller's working directory, not just when `SourceMap`
        // happens to resolve the on-disk `piperine::math` path.
        if let Ok(source) = parse_str(include_str!("../headers/math.phdl")) {
            self.tag_item_files(&source.items, concat!(env!("CARGO_MANIFEST_DIR"), "/headers/math.phdl"));
            items.extend(source.items);
        }

        // `tasks` (declared-language-surface DLS-19) — system tasks
        // (`$display`, `$temperature`, …), same embedding rationale as
        // `types`/`math` above (called from any analog/digital body).
        if let Ok(source) = parse_str(include_str!("../headers/tasks.phdl")) {
            self.tag_item_files(&source.items, concat!(env!("CARGO_MANIFEST_DIR"), "/headers/tasks.phdl"));
            items.extend(source.items);
        }

        // `operators` (declared-language-surface DLS-20) — runtime
        // operators (`ddt`, `delay`, `white_noise`, …), same embedding
        // rationale as `types`/`math`/`tasks` above (called from any
        // analog behavior body across every stdlib device model).
        if let Ok(source) = parse_str(include_str!("../headers/operators.phdl")) {
            self.tag_item_files(&source.items, concat!(env!("CARGO_MANIFEST_DIR"), "/headers/operators.phdl"));
            items.extend(source.items);
        }

        // `introspection` (phdl-introspection-attributes PIA-04) — the
        // device-introspection metadata schemas (`@model`/`@name`/`@unit`/
        // `@description`/`@kind`), textually declared (MD-24) so LSP
        // go-to-definition lands on this header. Embedded the same way as
        // `types`/`math`/`tasks`/`operators`: introspection metadata is a
        // stdlib concern every project needs (every device author can name
        // a var or classify a terminal), not a plugin-gated surface like
        // `device_port.phdl`. Loaded after `types.phdl` because the field
        // types reference the primitive `String`.
        if let Ok(source) = parse_str(include_str!("../headers/introspection.phdl")) {
            self.tag_item_files(&source.items, concat!(env!("CARGO_MANIFEST_DIR"), "/headers/introspection.phdl"));
            items.extend(source.items);
        }

        // Load the standard library built-ins dynamically if they resolve.
        // We ignore errors so that a bare-bones SourceMap doesn't panic.
        let cap_key = vec!["piperine".to_string(), "capabilities".to_string()];
        if let Ok(src) = self.load_source(&cap_key) {
            let src_items = src.items.clone();
            if let Some(fp) = self.file_paths.get(&cap_key).cloned() {
                self.tag_item_files_owned(&src_items, fp);
            }
            items.extend(src_items);
        }

        let col_key = vec!["piperine".to_string(), "collections".to_string()];
        if let Ok(src) = self.load_source(&col_key) {
            let src_items = src.items.clone();
            if let Some(fp) = self.file_paths.get(&col_key).cloned() {
                self.tag_item_files_owned(&src_items, fp);
            }
            items.extend(src_items);
        }

        let pre_key = vec!["piperine".to_string(), "prelude".to_string()];
        if let Ok(src) = self.load_source(&pre_key) {
            let src_items = src.items.clone();
            if let Some(fp) = self.file_paths.get(&pre_key).cloned() {
                self.tag_item_files_owned(&src_items, fp);
            }
            items.extend(src_items);
        }

        if let Some(prelude_path) = &self.source_map.prelude_path {
            // Load custom prelude dynamically
            if let Ok(src) = std::fs::read_to_string(prelude_path)
                && let Ok(source) = parse_str(&src) {
                    items.extend(source.items);
                }
        }
        // Everything the prelude injects belongs to the `piperine` package.
        for item in &items {
            if let Some(name) = item.name() {
                self.origins.insert(name.to_string(), "piperine".to_string());
            }
        }
        items
    }

    /// Expand all `use` declarations in `source` (transitively) into a flat
    /// item list with no `UseDecl` items.
    ///
    /// Diamond dependencies are handled via `seen` deduplication.
    ///
    /// **Privacy:** items from the root source are always included.
    /// Items from `use`d files are only included if declared `pub`.
    /// The prelude (injected via [`prelude_items`](Self::prelude_items))
    /// is always fully included — it is compiler-injected, not user-imported.
    pub fn expand(&mut self, source: SourceFile) -> Result<Vec<ast::Item>, ResolveError> {
        let mut seen: HashSet<Vec<String>> = HashSet::new();
        self.expand_inner(source, &mut seen, None, None)
    }

    /// Recursively expand `use` declarations in a source file, tracking
    /// already-visited paths in `seen` to break diamond dependencies.
    ///
    /// `package`: the first `use`-path segment this source was loaded under,
    /// `None` for the root source. Non-`pub` items from used sources are
    /// filtered out — except for the `piperine` package (the standard
    /// library), whose items are always exported. Kept items from a used
    /// source are recorded in [`Self::origins`] under their package.
    fn expand_inner(
        &mut self,
        source: SourceFile,
        seen: &mut HashSet<Vec<String>>,
        package: Option<&str>,
        current_file: Option<PathBuf>,
    ) -> Result<Vec<ast::Item>, ResolveError> {
        let mut result = Vec::new();
        for item in source.items {
            match item {
                ast::Item::UseDecl(path) => {
                    if seen.contains(&path.segments) {
                        continue;
                    }
                    seen.insert(path.segments.clone());
                    let resolved = self.load_source(&path.segments)?.clone();
                    let used_package = path.segments.first().cloned().unwrap_or_default();
                    let used_file = self.file_paths.get(&path.segments).cloned();
                    result.extend(self.expand_inner(resolved, seen, Some(&used_package), used_file)?);
                }
                other => {
                    // stdlib items are always exported; user items require `pub`.
                    let is_stdlib = package == Some("piperine");
                    if std::env::var("PIPERINE_DEBUG_FNS").is_ok() {
                        eprintln!("DBG expand pkg={package:?} item={:?} pub={}", other.name(), other.is_pub());
                    }
                    if package.is_some() && !is_stdlib && !other.is_pub() {
                        continue;
                    }
                    if let (Some(pkg), Some(name)) = (package, other.name()) {
                        self.origins.insert(name.to_string(), pkg.to_string());
                        if let Some(fp) = &current_file {
                            self.item_files.insert(name.to_string(), fp.clone());
                        }
                    }
                    result.push(other);
                }
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

        self.file_paths.insert(path.to_vec(), file_path);
        self.cache.insert(path.to_vec(), source);
        Ok(self.cache.get(path).unwrap())
    }
}
