//! Per-document state: parsed design, errors, and version tracking.

use std::collections::HashMap;
use std::path::PathBuf;
use lsp_types::Uri;

use piperine_lang::elab::registry::ElabContext;
use piperine_lang::Design;

/// Holds the current state of each open document.
pub struct ServerState {
    /// Parsed designs keyed by document URI.
    pub documents: HashMap<Uri, DocumentState>,
    /// Multi-file project units (T12/LSP-14), keyed by project root —
    /// built lazily the first time a document belonging to that project is
    /// analyzed (`analyze_document`), then reused. A document outside any
    /// `Piperine.toml` never gets an entry here (`DocumentState::
    /// project_root` stays `None`) — the single-file fallback, LSP-17.
    pub projects: HashMap<PathBuf, crate::project::ProjectUnit>,
}

pub struct DocumentState {
    /// The raw source text of the document.
    pub source: String,
    /// Document version number (from didChange notifications).
    pub version: i32,
    /// The elaborated design, if parsing succeeded.
    pub design: Option<Design>,
    /// The `ElabContext` registries populated alongside `design` — carries
    /// every `extern`-declared type/fn/task/operator/attribute-schema/impl
    /// method's `decl_span` (declared-language-surface T14). `None` until
    /// the first successful `analyze()`, same lifecycle as `design`.
    pub ctx: Option<ElabContext>,
    /// The `ResolutionIndex` built over `design` (LSP-03/05/10/13) — every
    /// module/port/param/wire/var/instance/behavior binding keyed by a
    /// stable `BindingId`, feeding the occurrence engine
    /// (`occurrences_at`) that references/rename/highlight consume. Same
    /// lifecycle as `design`/`ctx` (`None` until the first successful
    /// `analyze()`).
    pub resolution_index: Option<piperine_lang::ResolutionIndex>,
    /// The enclosing project's root directory (T12/LSP-14), when one was
    /// discovered by walking up from this document to a `Piperine.toml`.
    /// `None` for a standalone document — the single-file fallback
    /// (LSP-17); set by `ServerState::analyze_document`.
    pub project_root: Option<PathBuf>,
    /// The raw parsed AST.
    pub ast: Option<piperine_lang::parse::ast::SourceFile>,
    /// Parse/elaboration error messages if any.
    pub errors: Vec<ParseError>,
}

/// A parse or elaboration error with optional source position.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    /// Span in the source.
    pub span: Option<miette::SourceSpan>,
    /// The structured diagnostic code (T17/LSP-19), e.g. `"E2002"` for
    /// `ElabErrorKind::UndefinedType` or `"E1002"` for a parser
    /// `UnexpectedTok` — both error enums already carry one per variant via
    /// `#[diagnostic(code(...))]` (`miette::Diagnostic::code()`). `None`
    /// only for an error kind that genuinely predates that derive (should
    /// not happen for `piperine_lang::parse::error::ParseError`/`ElabError`
    /// today — both enums are fully coded).
    pub code: Option<String>,
}

impl ServerState {
    pub fn new() -> Self {
        Self { documents: HashMap::new(), projects: HashMap::new() }
    }

    /// Create a ServerState for testing (no connection needed).
    #[allow(dead_code)]
    pub fn dummy() -> Self {
        Self { documents: HashMap::new(), projects: HashMap::new() }
    }

    /// Analyze the document at `uri`: discover its enclosing project (if
    /// any), analyze it against that project's `SourceMap`, and — when a
    /// project root was found — ensure `self.projects` holds a
    /// `ProjectUnit` for that root (built once, then reused) and record it
    /// on the document. This is the single seam every notification/request
    /// handler that re-elaborates a document should call instead of
    /// discovering the project and calling `DocumentState::analyze`
    /// separately, so `projects`/`project_root` never drift out of sync
    /// with `documents` (T12/LSP-14).
    pub fn analyze_document(&mut self, uri: &Uri) {
        let ctx = crate::project::ProjectContext::discover(uri);
        let source_map = ctx.source_map();
        let root = ctx.root().cloned();

        if let Some(doc) = self.documents.get_mut(uri) {
            doc.analyze(&source_map);
            doc.project_root = root.clone();
        }

        if let Some(root) = root {
            self.projects
                .entry(root.clone())
                .or_insert_with(|| crate::project::ProjectUnit::build(&root, &source_map));
        }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentState {
    /// A fresh document holding `source` at `version`, not yet analyzed.
    pub fn new(source: String, version: i32) -> Self {
        Self {
            source,
            version,
            design: None,
            ctx: None,
            resolution_index: None,
            project_root: None,
            ast: None,
            errors: Vec::new(),
        }
    }

    /// Run the full lexer+parser+elaborator pipeline over the current
    /// source, refreshing `design`, `ast`, and `errors` in place.
    ///
    /// On elaboration failure the **previous** design is kept so that
    /// hover, goto-definition and outline continue working on the last
    /// valid snapshot instead of going completely dark.
    pub fn analyze(&mut self, source_map: &piperine_lang::SourceMap) {
        self.errors.clear();
        let (source_file, parse_errors) =
            piperine_lang::parse::parse_str_tolerant(&self.source);

        for e in parse_errors {
            let code = miette::Diagnostic::code(&e).map(|c| c.to_string());
            self.errors.push(ParseError { message: e.to_string(), span: e.span(), code });
        }

        // LSP-18/T16: accumulates every independent elaboration error
        // instead of stopping at the first (see
        // `Elaborator::elaborate_accumulating`'s docs for exactly which
        // passes accumulate vs. fail fast).
        let (design, ctx, elab_errors) =
            source_file.clone().elaborate_with_context_accumulating(source_map);
        if elab_errors.is_empty() {
            // Update to the new valid design (+ its registries + the
            // ResolutionIndex built over it — LSP-03/05/10/13).
            self.resolution_index = Some(piperine_lang::elab::resolution::index_design(&design));
            self.design = Some(design);
            self.ctx = Some(ctx);
        } else {
            // Record every error but keep the previous design alive so
            // language features (hover, go-to-def, outline) keep working.
            for e in elab_errors {
                // `ElabError::kind` (not `ElabError` itself) carries the
                // `#[diagnostic(code(...))]` — `#[diagnostic_source]`
                // forwards `.diagnostic_source()`/`.source()`, not `.code()`.
                let code = miette::Diagnostic::code(&e.kind).map(|c| c.to_string());
                self.errors.push(ParseError { message: e.to_string(), span: e.span, code });
            }
            // `self.design`/`self.ctx` intentionally left unchanged
            // (stale-but-valid).
        }
        self.ast = Some(source_file);
    }

    /// Resolve the identifier at `byte_offset` against the elaborated
    /// design and its registries (None when the document has no design or
    /// no symbol matches).
    pub fn resolve_at(&self, byte_offset: usize) -> Option<crate::symbol_index::Resolution> {
        // Go-to-definition on a `use` statement's path opens the file it
        // loads (`use spice::passives;` → `.../headers/spice/passives.phdl`).
        if let Some(res) = self.resolve_use_path_at(byte_offset) {
            return Some(res);
        }
        crate::symbol_index::resolve_at(self.design.as_ref()?, &self.source, byte_offset, self.ctx.as_ref())
    }

    /// When `byte_offset` falls inside a `use <path>;` statement's path,
    /// resolve to the on-disk file that path loads (from
    /// `Design::project().use_file`). Returns a `Module`-kind resolution
    /// carrying `file` + a `(0, 1)` `decl_span` so `goto_def` opens the
    /// target file at its top. `None` when the cursor isn't on a `use` path
    /// or the path never resolved.
    fn resolve_use_path_at(&self, byte_offset: usize) -> Option<crate::symbol_index::Resolution> {
        let design = self.design.as_ref()?;
        let src = &self.source;
        // The line containing byte_offset.
        let line_start = src[..byte_offset.min(src.len())].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = src[line_start..].find('\n').map(|i| line_start + i).unwrap_or(src.len());
        let line = &src[line_start..line_end];

        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let rest = trimmed.strip_prefix("use ")?;
        // The path runs from after `use ` up to the `;` (or end of line).
        let path_str = rest.split(';').next()?.trim();
        if path_str.is_empty() {
            return None;
        }
        // Byte range of the path within the source.
        let path_start = line_start + indent + "use ".len() + (rest.len() - rest.trim_start().len());
        let path_end = path_start + path_str.len();
        if byte_offset < path_start || byte_offset > path_end {
            return None;
        }
        let segments: Vec<String> = path_str.split("::").map(|s| s.trim().to_string()).collect();
        let file = design.project().use_file(&segments)?;
        Some(crate::symbol_index::Resolution {
            kind: crate::symbol_index::SymbolKind::Module,
            name: segments.last().cloned().unwrap_or_default(),
            decl_span: Some(miette::SourceSpan::new(0usize.into(), 1)),
            type_info: None,
            doc: None,
            file: Some(file.to_path_buf()),
        })
    }

    /// Byte ranges of every occurrence of the binding resolved at
    /// `byte_offset` (LSP-10/13's base engine, T8) — the binding-identity
    /// source references/rename/highlight (T9-T11) read instead of
    /// [`word_occurrences`](Self::word_occurrences)'s text scan. Empty when
    /// nothing resolves at `byte_offset` (keyword/literal/comment — no
    /// symbol, no occurrences).
    pub fn occurrences_at(&self, byte_offset: usize) -> Vec<(usize, usize)> {
        let Some(decl_span) = self.resolve_at(byte_offset).and_then(|r| r.decl_span) else {
            return Vec::new();
        };
        let spans = self
            .resolution_index
            .as_ref()
            .map(|idx| crate::occurrences::occurrences_for_decl_span(idx, decl_span))
            .unwrap_or_default();
        let spans = if spans.is_empty() { vec![decl_span] } else { spans };
        spans.iter().map(|s| (s.offset(), s.offset() + s.len())).collect()
    }

    /// Byte ranges of every whole-word occurrence of `word` in the source.
    pub fn word_occurrences(&self, word: &str) -> Vec<(usize, usize)> {
        let bytes = self.source.as_bytes();
        let is_word_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let mut occurrences = Vec::new();
        let mut start = 0;
        while let Some(idx) = self.source[start..].find(word) {
            let begin = start + idx;
            let end = begin + word.len();
            let bounded_left = begin == 0 || !is_word_byte(bytes[begin - 1]);
            let bounded_right = end == bytes.len() || !is_word_byte(bytes[end]);
            if bounded_left && bounded_right {
                occurrences.push((begin, end));
            }
            start = end;
        }
        occurrences
    }
}
