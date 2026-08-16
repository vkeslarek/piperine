//! Source provenance: maps positions in the preprocessor's flat expanded
//! buffer back to the real file (and, for macro-expanded text, the
//! invocation/definition chain) they came from.
//!
//! `.specs/features/preprocessor-span-mapping/design.md` — the internal,
//! unresolved position type lexer/grammar/AST/codegen mint is
//! [`crate::error::PreprocessedSpan`] (today's `Span`, renamed); the
//! [`Span`] in this module is the *resolved*, human-facing type, produced
//! only by [`SourceProvenance::resolve`] — never constructed directly by a
//! consumer.

/// Opaque handle into [`SourceProvenance`]'s source table — one per unique
/// loaded file path (repeated `` `include ``s of the same path share one
/// id) plus one for the top-level input string itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(pub u32);

/// Resolved — the only type a diagnostic renderer may consume. A location
/// that is always real: a source id, a byte range valid in that source's
/// own text, and (optionally) the chain of macro invocation/definition
/// frames that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub source_id: SourceId,
    pub start: usize,
    pub end: usize,
    /// Empty for plain source / include-only spans. One frame per
    /// expansion hop for macro-expanded spans, innermost first.
    pub expansion_chain: Vec<ExpansionFrame>,
}

/// One hop in a macro-expansion chain — the invocation site's `Span`, the
/// macro's own `` `define `` body location, and the resolved argument
/// spans passed at this invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionFrame {
    pub macro_name: String,
    /// Where the macro was invoked (`` `NAME(...) `` in the calling text)
    /// — primary attribution.
    pub invocation: Span,
    /// Where the macro's own `` `define `` body text lives — secondary
    /// "expanded from here" label.
    pub definition: Span,
    /// Each declared parameter paired with the resolved `Span` of the
    /// actual argument text passed at this invocation.
    pub args: Vec<(String, Span)>,
}
