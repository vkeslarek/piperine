//! Per-document state: parsed design, errors, and version tracking.

use std::collections::HashMap;
use lsp_types::Uri;

use piperine_lang::elab::registry::ElabContext;
use piperine_lang::Design;

/// Holds the current state of each open document.
pub struct ServerState {
    /// Parsed designs keyed by document URI.
    pub documents: HashMap<Uri, DocumentState>,
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
}

impl ServerState {
    pub fn new() -> Self {
        Self { documents: HashMap::new() }
    }

    /// Create a ServerState for testing (no connection needed).
    #[allow(dead_code)]
    pub fn dummy() -> Self {
        Self { documents: HashMap::new() }
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
        Self { source, version, design: None, ctx: None, ast: None, errors: Vec::new() }
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
            self.errors.push(ParseError { message: e.to_string(), span: e.span() });
        }

        match source_file.clone().elaborate_with_context(source_map) {
            Ok((d, ctx)) => {
                // Update to the new valid design (+ its registries).
                self.design = Some(d);
                self.ctx = Some(ctx);
            }
            Err(e) => {
                // Record the error but keep the previous design alive so
                // language features (hover, go-to-def, outline) keep working.
                self.errors.push(ParseError { message: e.to_string(), span: e.span });
                // `self.design`/`self.ctx` intentionally left unchanged
                // (stale-but-valid).
            }
        };
        self.ast = Some(source_file);
    }

    /// Resolve the identifier at `byte_offset` against the elaborated
    /// design and its registries (None when the document has no design or
    /// no symbol matches).
    pub fn resolve_at(&self, byte_offset: usize) -> Option<crate::symbol_index::Resolution> {
        crate::symbol_index::resolve_at(self.design.as_ref()?, &self.source, byte_offset, self.ctx.as_ref())
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
