//! The single entry point: preprocess -> lex -> parse -> [`SourceFile`].
//!
//! Everything upstream (`preprocessor`, `lexer`, `grammar`) is usable on
//! its own for tooling that only needs one stage; `Parser` is the
//! convenience façade that chains all three the way every real caller
//! wants them chained.

use crate::ast::SourceFile;
use crate::error::ParseError;
use crate::lexer::Lexer;
use crate::preprocessor::{IncludeResolver, Preprocessor};
use crate::grammar::Grammar;

/// Parses `.va`/`.vams` source text into a [`SourceFile`].
///
/// `Parser` owns no state across calls — construct one per source unit.
pub struct Parser {
    preprocessor: Preprocessor,
}

impl Parser {
    /// A parser with no `` `include`` support (`NoIncludes` — any
    /// `` `include`` directive fails loud rather than silently no-op'ing,
    /// C6).
    pub fn new() -> Self {
        Self { preprocessor: Preprocessor::new() }
    }

    /// A parser that resolves `` `include`` directives through the given
    /// resolver. The crate itself performs no filesystem I/O (purity,
    /// T1) — the resolver is how a host (`piperine-lang`) supplies it.
    pub fn with_include_resolver(resolver: impl IncludeResolver + 'static) -> Self {
        Self { preprocessor: Preprocessor::with_include_resolver(resolver) }
    }

    /// Runs the full pipeline: preprocess, lex, parse.
    ///
    /// Errors surface with the deepest meaningful span: a preprocessor
    /// failure (e.g. unresolved `` `include``) carries the directive's own
    /// span, a lexical failure carries the offending token's span, and a
    /// grammar failure carries the span of the construct that didn't
    /// parse — never an outer "somewhere in this file" span.
    pub fn parse(mut self, source: &str) -> Result<SourceFile, ParseError> {
        let (preprocessed, source_provenance) = self.preprocessor.run(source)?;
        let directives = self.preprocessor.directives().clone();
        let toks = Lexer::new(&preprocessed).tokenize()?;
        let mut file = Grammar::new(toks).parse_source_file()?;
        file.directives = directives;
        file.source_provenance = source_provenance;
        Ok(file)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
