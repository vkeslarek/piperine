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

/// One contiguous run of the preprocessor's flat output buffer, mapped back
/// to a real source's own text.
#[derive(Debug, Clone)]
struct Segment {
    buffer_range: std::ops::Range<usize>,
    source_id: SourceId,
    /// Where `buffer_range.start` maps to in `source_id`'s own text (the
    /// rest of the range follows contiguously).
    local_start: usize,
    /// `Some` only for segments produced by macro-body substitution — the
    /// frame linking this expanded text back to its invocation.
    ///
    /// SPEC_DEVIATION: single-level only here — this is `resolve()`'s
    /// chain of length <= 1. T9 (out of this batch) replaces this with
    /// proper N-deep recursive chain-walking (nested macro-of-macro
    /// expansion, SPANMAP-06); tasks.md's own T9 "Where" field names this
    /// exact stub explicitly ("resolve()'s chain-walking, if T2 stubbed
    /// it"), so this is the intended shape at T2's boundary, not scope
    /// creep or an oversight.
    expanded_from: Option<ExpansionFrame>,
}

/// Built by `Preprocessor::run`; consulted by every render boundary. The
/// registry of every loaded source text plus a segment table mapping
/// preprocessed-buffer byte ranges back to real `(source_id, local_offset)`.
#[derive(Debug)]
pub struct SourceProvenance {
    /// `(name/path, full text)`, indexed by `SourceId`.
    sources: Vec<(String, String)>,
    /// Sorted by `buffer_range.start`, contiguous, no gaps (an invariant
    /// `Preprocessor::run`'s paired recording calls, T6, are responsible
    /// for upholding).
    segments: Vec<Segment>,
}

/// Builder for [`SourceProvenance`] — the API `Preprocessor::run` (T6) calls
/// as it splices text into its output buffer, pairing every emitted byte
/// range with the real source it came from.
#[derive(Default)]
pub struct Builder {
    sources: Vec<(String, String)>,
    segments: Vec<Segment>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a source's full text, returning its stable [`SourceId`].
    /// The same `name` reuses the same id (SPANMAP-03 — repeated
    /// `` `include ``s of one path share one identity).
    pub fn add_source(&mut self, name: impl Into<String>, text: impl Into<String>) -> SourceId {
        let name = name.into();
        if let Some(idx) = self.sources.iter().position(|(n, _)| n == &name) {
            return SourceId(idx as u32);
        }
        let id = SourceId(self.sources.len() as u32);
        self.sources.push((name, text.into()));
        id
    }

    /// Records that `buffer_range` (bytes in the preprocessor's flat output)
    /// came from `source_id`'s own text starting at `local_start`.
    pub fn record(
        &mut self,
        buffer_range: std::ops::Range<usize>,
        source_id: SourceId,
        local_start: usize,
        expanded_from: Option<ExpansionFrame>,
    ) {
        self.segments.push(Segment { buffer_range, source_id, local_start, expanded_from });
    }

    pub fn build(mut self) -> SourceProvenance {
        self.segments.sort_by_key(|s| s.buffer_range.start);
        SourceProvenance { sources: self.sources, segments: self.segments }
    }
}

impl SourceProvenance {
    /// A documented "empty/test" registry: `resolve()` degrades to a
    /// synthetic `<test>` source at the same offset instead of the
    /// loud-fail path, so hand-built test `Design`s that never went
    /// through the real parse pipeline don't need touching (design's Risks
    /// table mitigation).
    pub fn empty_for_test() -> Self {
        Self { sources: Vec::new(), segments: Vec::new() }
    }

    /// Forward lookup: maps an unresolved (preprocessed-buffer) span to its
    /// real originating file and local byte range.
    pub fn resolve(&self, span: crate::error::PreprocessedSpan) -> Span {
        let Some(seg) = self.segment_covering(span.start) else {
            // Documented empty/test degrade path (see `empty_for_test`).
            return Span {
                source_id: SourceId(0),
                start: span.start,
                end: span.end,
                expansion_chain: Vec::new(),
            };
        };
        let local_start = seg.local_start + (span.start - seg.buffer_range.start);
        let local_end = local_start + (span.end - span.start);
        let expansion_chain = seg.expanded_from.iter().cloned().collect();
        Span { source_id: seg.source_id, start: local_start, end: local_end, expansion_chain }
    }

    /// The named source's own full text (for `NamedSource`/LSP document
    /// lookups).
    pub fn text(&self, source_id: SourceId) -> &str {
        self.sources.get(source_id.0 as usize).map(|(_, t)| t.as_str()).unwrap_or("")
    }

    /// The source's file path (or a synthetic name, e.g. `"<test>"`, for a
    /// source id with no registered entry).
    pub fn name(&self, source_id: SourceId) -> &str {
        self.sources.get(source_id.0 as usize).map(|(n, _)| n.as_str()).unwrap_or("<test>")
    }

    /// Finds the segment covering `pos`. `None` only for a genuinely empty
    /// registry (the documented empty/test degrade path) — any non-empty,
    /// correctly built registry covers every offset up to its buffer's
    /// length with no gaps, an invariant `Preprocessor::run`'s paired
    /// recording (T6) upholds; a `pos` past that range is a bug in that
    /// pairing, so this fails loud (debug-asserts) rather than guessing.
    fn segment_covering(&self, pos: usize) -> Option<&Segment> {
        if self.segments.is_empty() {
            return None;
        }
        let idx = self.segments.partition_point(|s| s.buffer_range.start <= pos);
        let idx = idx.saturating_sub(1);
        let seg = &self.segments[idx];
        debug_assert!(
            pos <= seg.buffer_range.end,
            "PreprocessedSpan offset {pos} falls outside every recorded segment (last segment ends at {}) — a gap in Preprocessor::run's paired recording",
            seg.buffer_range.end
        );
        Some(seg)
    }
}
