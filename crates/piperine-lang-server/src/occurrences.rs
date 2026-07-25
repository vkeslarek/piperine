//! Occurrence engine (LSP-10/13 base): the shared source references,
//! rename, and document-highlight (T9-T11) read from, instead of
//! `DocumentState::word_occurrences`'s whole-word text scan.
//!
//! Given a resolved declaration's `decl_span` (already scope-correct —
//! `symbol_index::resolve_at` picked it via cursor context + innermost
//! shadowing, T6), find the matching binding in the elaborator's
//! [`ResolutionIndex`] and return every span recorded as a use of it.
//!
//! ## SPEC_DEVIATION (inherited from T5)
//!
//! `ResolutionIndex.use_spans` today holds only each binding's own
//! declaration span (a reflexive use) — the AST carries no per-occurrence
//! span for `Expr::Ident`, so in-expression uses (e.g. `r` inside
//! `V(p, n) / r`) are not tracked yet (see
//! `piperine-lang/src/elab/resolution.rs`'s module docs). This engine
//! returns exactly what the index provides — a one-element list for most
//! bindings — it does not invent occurrences the index doesn't have.
//!
//! Reason: fixing the underlying gap means threading a span through every
//! `Expr` variant across two crates (`piperine-lang` *and*
//! `piperine-codegen`), well outside a single task's surgical-change
//! budget. Tracked as a known follow-up, not silently pretended away.
//!
//! `Resolution`s that have no counterpart in `ResolutionIndex` at all
//! (`extern` fn/type/operator/attribute-schema/impl-method lookups, which
//! live in `ElabContext` registries, not the POM `ResolutionIndex`
//! indexes) still resolve to *something*: their own `decl_span`, the one
//! occurrence we know for certain — never an empty result for a symbol
//! that did resolve.

use piperine_lang::ResolutionIndex;

/// Every span recorded as a use of the binding whose `decl_span` exactly
/// matches `decl_span`, per `index`. Returns an empty vec when `index` has
/// no binding at that span (e.g. the resolution came from an `ElabContext`
/// registry, not the POM-derived index) — callers fall back to the
/// `decl_span` itself as the sole known occurrence.
pub fn occurrences_for_decl_span(
    index: &ResolutionIndex,
    decl_span: miette::SourceSpan,
) -> Vec<miette::SourceSpan> {
    index
        .bindings()
        .find(|(_, info)| {
            info.decl_span.offset() == decl_span.offset() && info.decl_span.len() == decl_span.len()
        })
        .map(|(id, _)| index.occurrences(*id).to_vec())
        .unwrap_or_default()
}
