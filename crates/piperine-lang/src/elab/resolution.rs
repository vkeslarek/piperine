//! # `ResolutionIndex` — the keystone LSP side artifact (LSP-03/05)
//!
//! The elaborator resolves every module's ports/params/wires/vars/instances/
//! behaviors into the POM (`Design`) — but the POM alone does not let a host
//! ask "which declaration does this identifier refer to?" without falling
//! back to a text/word scan. `ResolutionIndex` is an additive side artifact
//! (MD-25 spirit — it never mutates `Design`) that gives every declaration a
//! stable [`BindingId`] and records its declaration span, kind, doc, and the
//! set of spans that refer to it.
//!
//! ## Scope of this pass (SPEC_DEVIATION)
//!
//! The index is built by walking the already-elaborated [`Design`] (POM),
//! not by re-implementing scoping — it *reuses* what elaboration already
//! decided (LSP-05: no server-side re-scoping). It indexes exactly the decl
//! kinds that carry a POM span today: module, port, param, wire, var,
//! instance, behavior.
//!
//! **Known limitation:** `piperine-lang`'s AST does not track a span per
//! `Expr::Ident` occurrence (only whole declarations/statements carry
//! spans) — adding one would mean threading a span field through every
//! `Expr` variant and every consumer across `piperine-lang` *and*
//! `piperine-codegen` (which lowers these same AST bodies), which is far
//! outside a single task's surgical-change budget and touches
//! correctness-critical, "edit with care" code (CLAUDE.md). So today each
//! binding's `use_spans` contains only its own declaration span (a
//! reflexive "use" — satisfying LSP-03's "decl and use of the same binding
//! share one identity" for the declaration site itself). Real in-expression
//! use-site tracking (e.g. `V(p, n) / r` resolving `r`'s occurrence to the
//! `r` param binding) is a follow-up once `Expr` carries spans — tracked as
//! a known gap, not silently pretended away.
//!
//! This still gives the language server (T6/T7) a real, correct,
//! binding-identity-keyed structure to resolve declarations and
//! declaration-site cursor positions against, rather than a global
//! first-match word scan.

use std::collections::HashMap;

use crate::pom::Design;

/// A stable identity for one declaration, shared by every span that refers
/// to it (today: only the declaration's own span — see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId(pub u32);

/// What kind of declaration a [`BindingId`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Module,
    Port,
    Param,
    Wire,
    Var,
    Instance,
    Behavior,
}

/// Everything the language server needs about one binding.
#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub decl_span: miette::SourceSpan,
    pub kind: BindingKind,
    pub name: String,
    pub doc: Option<String>,
    pub use_spans: Vec<miette::SourceSpan>,
    /// The enclosing module's name, for scope-aware lookup — `None` for the
    /// module binding itself (a top-level declaration).
    pub owner_module: Option<String>,
    /// The source file this binding was declared in. Single-file
    /// elaboration (this pass) always leaves this `None`; multi-file
    /// project indexing (a later phase) is expected to fill it in.
    pub file: Option<String>,
}

/// Maps identifier-occurrence spans to the [`BindingId`] they refer to, and
/// every [`BindingId`] to its full [`BindingInfo`] (LSP-03/05).
#[derive(Debug, Clone, Default)]
pub struct ResolutionIndex {
    /// Sorted by span start byte offset, for cursor-offset lookup.
    uses: Vec<(miette::SourceSpan, BindingId)>,
    bindings: HashMap<BindingId, BindingInfo>,
}

impl ResolutionIndex {
    /// Look up a binding's full info.
    pub fn binding(&self, id: BindingId) -> Option<&BindingInfo> {
        self.bindings.get(&id)
    }

    /// Iterate all bindings recorded in this index.
    pub fn bindings(&self) -> impl Iterator<Item = (&BindingId, &BindingInfo)> {
        self.bindings.iter()
    }

    /// The number of bindings recorded.
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Resolve a byte offset to the [`BindingId`] whose span contains it —
    /// the innermost (smallest) containing span wins when spans nest.
    pub fn resolve_at(&self, offset: usize) -> Option<BindingId> {
        let mut best: Option<(usize, BindingId)> = None;
        for (span, id) in &self.uses {
            let start = span.offset();
            let end = start + span.len();
            if offset >= start && offset < end {
                match best {
                    Some((best_len, _)) if span.len() >= best_len => {}
                    _ => best = Some((span.len(), *id)),
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Every span recorded as a use of `id` (LSP-10/11/13's shared source).
    pub fn occurrences(&self, id: BindingId) -> &[miette::SourceSpan] {
        self.bindings.get(&id).map(|b| b.use_spans.as_slice()).unwrap_or(&[])
    }

    /// Stamp every binding currently recorded in this index with `file`
    /// (multi-file project indexing, T12/LSP-14) — call once on a
    /// single-file index (whose bindings all leave `file: None`, see the
    /// module docs) before folding it into a project-wide index with
    /// [`merge`](Self::merge).
    pub fn set_file(&mut self, file: String) {
        for info in self.bindings.values_mut() {
            info.file = Some(file.clone());
        }
    }

    /// Fold `other`'s uses and bindings into `self`, remapping `other`'s
    /// [`BindingId`]s so they never collide with `self`'s own (each
    /// single-file index independently numbers bindings from 0) —
    /// multi-file project indexing (T12/LSP-14). `self.uses` stays sorted
    /// by span offset afterward.
    pub fn merge(&mut self, other: ResolutionIndex) {
        let offset = self.bindings.keys().map(|id| id.0 + 1).max().unwrap_or(0);
        for (span, id) in other.uses {
            self.uses.push((span, BindingId(id.0 + offset)));
        }
        for (id, info) in other.bindings {
            self.bindings.insert(BindingId(id.0 + offset), info);
        }
        self.uses.sort_by_key(|(span, _)| span.offset());
    }

    fn insert(&mut self, next_id: &mut u32, kind: BindingKind, name: String, span: Option<miette::SourceSpan>, doc: Option<String>, owner_module: Option<String>) {
        let Some(span) = span else { return };
        let id = BindingId(*next_id);
        *next_id += 1;
        self.uses.push((span, id));
        self.bindings.insert(id, BindingInfo {
            decl_span: span,
            kind,
            name,
            doc,
            use_spans: vec![span],
            owner_module,
            file: None,
        });
    }
}

/// Build a [`ResolutionIndex`] over an elaborated [`Design`] — walks every
/// module's ports/params/wires/vars/instances/behaviors and assigns each a
/// [`BindingId`] (LSP-05: reuses the elaborator's already-resolved POM,
/// never re-scopes). Does not mutate `design`.
pub fn index_design(design: &Design) -> ResolutionIndex {
    let mut idx = ResolutionIndex::default();
    let mut next_id: u32 = 0;

    for m in design.modules() {
        idx.insert(&mut next_id, BindingKind::Module, m.name.clone(), m.span, m.doc.clone(), None);

        for p in &m.ports {
            idx.insert(&mut next_id, BindingKind::Port, p.name.clone(), p.span, p.doc.clone(), Some(m.name.clone()));
        }
        for p in &m.params {
            idx.insert(&mut next_id, BindingKind::Param, p.name.clone(), p.span, p.doc.clone(), Some(m.name.clone()));
        }
        for w in &m.wires {
            idx.insert(&mut next_id, BindingKind::Wire, w.name.clone(), w.span, w.doc.clone(), Some(m.name.clone()));
        }
        for v in &m.vars {
            idx.insert(&mut next_id, BindingKind::Var, v.name.clone(), v.span, v.doc.clone(), Some(m.name.clone()));
        }
        for i in &m.instances {
            // LSB-07..10 (T8): index by the same token-level span convention
            // `symbol_index.rs`'s Instance resolve arm uses, so
            // `occurrences_for_decl_span`'s exact offset+len match succeeds
            // byte-for-byte — label_span when labeled, type_span when not,
            // falling back to the whole-statement `i.span` only if the
            // token-level span is genuinely absent.
            let decl_span = if i.label.is_some() {
                i.label_span.or(i.type_span).or(i.span)
            } else {
                i.type_span.or(i.span)
            };
            idx.insert(&mut next_id, BindingKind::Instance, i.name().to_string(), decl_span, i.doc.clone(), Some(m.name.clone()));
        }
        for b in &m.behaviors {
            idx.insert(&mut next_id, BindingKind::Behavior, b.name.clone(), b.span, b.doc.clone(), Some(m.name.clone()));
        }
    }

    idx.uses.sort_by_key(|(span, _)| span.offset());
    idx
}
