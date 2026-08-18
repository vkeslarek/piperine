//! Preprocessor-produced ch.10 compiler-directive state, carried on
//! [`crate::ast::SourceFile`] and copied onto [`crate::pom::Design`] by
//! elaboration (design.md P0.3).
//!
//! Only `default_discipline`/`default_transition` are actually written by
//! a directive handler in this task (T6, LRMC-C10-01/02's parse half) —
//! the remaining fields are structurally present so the P4 tasks that
//! implement `` `default_nettype ``/`` `begin_keywords ``/
//! `` `celldefine ``/`` `unconnected_drive ``/`` `line `` don't need to
//! change this struct's shape, per design.md's own P0.3 component note
//! ("you only need to populate `default_discipline`/`default_transition`
//! in this task ... the other fields are structurally present ... don't
//! implement their semantics now, just don't block the struct shape").

use crate::error::PreprocessedSpan;

/// The `` `default_nettype `` net kind in force — structurally present,
/// not yet written by any directive handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum NetTypeDefault {
    #[default]
    Wire,
    None,
}

/// One `` `begin_keywords "version" `` scope — structurally present, not
/// yet written by any directive handler.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KeywordScope {
    pub version: String,
    #[serde(skip)]
    pub span: Option<PreprocessedSpan>,
}

/// The drive strength `` `unconnected_drive `` sets for unconnected ports
/// — structurally present, not yet written by any directive handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Drive {
    Pull0,
    Pull1,
}

/// `` `line ``'s file/line remapping table — structurally present, not yet
/// written by any directive handler. Each entry is `(source byte offset,
/// file name, line number)`.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct LineMap {
    pub entries: Vec<(usize, String, usize)>,
}

/// `` `timescale time_unit / time_precision `` (1364-2005-inherited, T53,
/// LRMC-C10-10): both fields as base-10 exponents relative to one second
/// (e.g. `-9` for `ns`), since every legal `` `timescale `` value is
/// `{1,10,100} × {s,ms,us,ns,ps,fs}` — an exact power of ten.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Timescale {
    pub unit_exponent: i32,
    pub precision_exponent: i32,
}

/// State the preprocessor accumulates while scanning ch.10 compiler
/// directives.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct DirectiveState {
    /// The discipline name in force per the most recent unqualified
    /// `` `default_discipline <name> `` directive. `None` both before any
    /// such directive and after a bare `` `default_discipline `` (no name)
    /// clears it — LRM §10.2: "if this directive is used without a
    /// discipline name, discipline resolution will not use a default
    /// discipline ... after this directive is encountered."
    ///
    /// SPEC_DEVIATION (documented, not silent): the LRM allows multiple
    /// simultaneously-active `default_discipline` directives distinguished
    /// by *qualifier* (`integer`/`reg`/`wire`/...) — "more than one such
    /// directive can be in force simultaneously, provided each differs in
    /// qualifier" (§10.2). This field tracks one unqualified default only,
    /// matching design.md's own stated field shape
    /// (`default_discipline: Option<String>`); qualifier-scoped precedence
    /// is not implemented by this task.
    pub default_discipline: Option<String>,
    /// The `(rise, fall)` time in force per the most recent
    /// `` `default_transition <time> `` directive (LRM §10.3: one
    /// `transition_time` value used for both rise and fall). `None`
    /// before any such directive is seen.
    pub default_transition: Option<(f64, f64)>,
    /// `` `default_nettype `` — structurally present, not yet written.
    pub default_nettype: NetTypeDefault,
    /// `` `begin_keywords ``/`` `end_keywords `` scopes — structurally
    /// present, not yet written.
    pub keyword_scopes: Vec<KeywordScope>,
    /// `` `celldefine ``/`` `endcelldefine `` spans — structurally
    /// present, not yet written.
    #[serde(skip)]
    pub celldefine_spans: Vec<PreprocessedSpan>,
    /// `` `unconnected_drive `` — structurally present, not yet written.
    pub unconnected_drive: Option<Drive>,
    /// `` `line ``'s remapping table — structurally present, not yet
    /// written.
    pub line_map: LineMap,
    /// The most recent `` `timescale ``'s `(unit, precision)` exponents —
    /// `None` before any such directive is seen.
    pub timescale: Option<Timescale>,
    /// Every `` `timescale `` seen, with the byte offset (into the fully
    /// preprocessed text) where it took effect — what makes the directive
    /// *per-module* resolvable (1364-2005: a module is governed by the
    /// last `` `timescale `` that precedes it). The preprocessor records
    /// `out.len()` at consumption, the same anchor `` `line `` uses.
    #[serde(skip)]
    pub timescale_spans: Vec<(usize, Timescale)>,
}

impl DirectiveState {
    /// The `` `timescale `` in force at byte `offset` into the fully
    /// preprocessed text — the last directive recorded at or before
    /// `offset` (1364-2005: a module is governed by the last
    /// `` `timescale `` that precedes it). `None` when no directive
    /// precedes `offset` — delays then carry raw simulation-time units.
    pub fn timescale_at(&self, offset: usize) -> Option<Timescale> {
        self.timescale_spans
            .iter()
            .filter(|(at, _)| *at <= offset)
            .next_back()
            .map(|(_, ts)| *ts)
            .or(self.timescale_if_single_before(offset))
    }

    /// The single-directive fallback for a deserialized `DirectiveState`
    /// (`` `timescale_spans `` is not serialized; a last-wins `timescale`
    /// survives): when only positions are missing, a recorded `timescale`
    /// governs every offset — the honest approximation a serialized
    /// design can carry.
    fn timescale_if_single_before(&self, _offset: usize) -> Option<Timescale> {
        if self.timescale_spans.is_empty() {
            self.timescale
        } else {
            None
        }
    }

    /// The LRM §10's user-facing line number for a byte `offset` into the
    /// *fully preprocessed* text — 1-based physical line count, unless a
    /// `` `line number "file" level `` directive is in force at or before
    /// `offset`, in which case it's `number` plus however many newlines
    /// separate the directive from `offset` (§10's remapping is exactly
    /// this: "the following line" starts counting from `number`).
    pub fn line_for_offset(&self, source: &str, offset: usize) -> usize {
        let offset = offset.min(source.len());
        let most_recent = self.line_map.entries.iter().filter(|(pos, _, _)| *pos <= offset).next_back();
        match most_recent {
            Some((pos, _file, line_no)) => {
                // `pos` is recorded at the tail of the (now-erased)
                // `` `line `` directive's own line, before its still-
                // emitted trailing newline — so the first newline after
                // `pos` only completes *that* line, and `line_no` names
                // the line after it. Subtract one to not double-count it.
                let delta = source[*pos..offset].matches('\n').count().saturating_sub(1);
                line_no + delta
            }
            None => 1 + source[..offset].matches('\n').count(),
        }
    }
}
