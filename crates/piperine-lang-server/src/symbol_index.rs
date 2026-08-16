use piperine_lang::elab::registry::ElabContext;
use piperine_lang::pom::Design;
use miette::SourceSpan;
use std::path::PathBuf;

/// Resolves any POM `decl_span` (always in the preprocessor's flat
/// buffer coordinate space — T3's mechanical rename never changed what
/// the numbers mean, only the type's name) to a real, correctly-placed
/// `Location` — same-document or a genuine `` `include ``d file.
///
/// **Why this exists**: every `Resolution::decl_span` is minted from an
/// AST node's `PreprocessedSpan` (`elab/lower/vams.rs`'s span-population
/// fix), which is a *flat-buffer* offset — identical to the open
/// document's own byte offsets only when nothing before the declaration
/// expanded (no earlier `` `include ``/`` `define ``). The moment
/// anything expands earlier in the file, every later declaration's flat
/// offset runs ahead of where that text actually sits in the document's
/// own raw source — building a `Location`/`Range` straight from
/// `decl_span.offset()` against `doc.source` silently mispoints (same
/// file, wrong line) or, when the declaration came from `` `include ``d
/// content entirely, points miles past the document's own length. This
/// is the single root cause behind three of four VS Code Ctrl+click bugs
/// found testing this feature: disciplines/natures from `` `include ``d
/// headers "resolving" into the current file, instances of modules
/// declared via `` `include `` not resolving at all, and even a
/// same-document instance-connection argument (a plain local `Wire`)
/// landing on the wrong line once any `` `include `` precedes it.
///
/// `design.source_provenance` (built by the same parse this `Design`
/// came from) already carries the real per-`` `include ``-file identity
/// and full captured text (T6/T7, `preprocessor-span-mapping`) — no
/// filesystem re-read needed for content, only for resolving the
/// `` `include ``d file's *path* (its `SourceProvenance` name is the raw
/// `` `include "…" `` argument string, not a resolved on-disk path;
/// `resolve_include_source_path` below does that resolution the same
/// way `SourceMapIncludeResolver` does, for URI purposes only).
pub(crate) fn resolve_decl_location(
    design: &Design,
    current_uri: &lsp_types::Uri,
    source_map: Option<&piperine_lang::SourceMap>,
    span: SourceSpan,
) -> Option<lsp_types::Location> {
    let flat = piperine_lang::error::PreprocessedSpan::new(span.offset(), span.offset() + span.len());
    let resolved = design.source_provenance.resolve(flat);
    if resolved.source_id == piperine_lang::source_provenance::SourceId(0) {
        // The document's own top-level source — build the range against
        // its own text directly (no read/URI change needed), matching
        // `SourceProvenance::text(SourceId(0))` byte-for-byte.
        let text = design.source_provenance.text(resolved.source_id);
        return Some(lsp_types::Location {
            uri: current_uri.clone(),
            range: crate::text_pos::byte_range(text, resolved.start, resolved.end),
        });
    }
    // A real `` `include ``d file — its captured text is already in
    // `SourceProvenance` (no disk re-read for content); only the URI
    // needs the actual on-disk path, resolved the same way the
    // preprocessor's own `IncludeResolver` found it.
    let raw_include_arg = design.source_provenance.name(resolved.source_id);
    let path = source_map.and_then(|sm| resolve_include_source_path(sm, raw_include_arg))?;
    let text = design.source_provenance.text(resolved.source_id);
    let uri: lsp_types::Uri = format!("file://{}", path.display()).parse().ok()?;
    Some(lsp_types::Location { uri, range: crate::text_pos::byte_range(text, resolved.start, resolved.end) })
}

/// If `byte_offset` lands inside an `` `include "path" `` directive's
/// quoted path text in `source` (the *document's own raw text* — this
/// runs before any preprocessing, so no flat-buffer translation is
/// needed here, unlike every other resolution in this module), returns
/// the raw path string. Go-to-definition on the include directive itself
/// opens that file — a different action from resolving a *symbol* the
/// included content declares (handled entirely separately by
/// `resolve_at`/`resolve_decl_location`).
pub(crate) fn include_directive_path_at(source: &str, byte_offset: usize) -> Option<String> {
    let bytes = source.as_bytes();
    let mut search_from = 0usize;
    while let Some(rel) = source[search_from..].find("`include") {
        let directive_start = search_from + rel;
        let mut p = directive_start + "`include".len();
        while matches!(bytes.get(p), Some(b' ') | Some(b'\t')) {
            p += 1;
        }
        if bytes.get(p) != Some(&b'"') {
            search_from = directive_start + 1;
            continue;
        }
        let quote_start = p + 1;
        let mut q = quote_start;
        while q < bytes.len() && bytes[q] != b'"' {
            q += 1;
        }
        if byte_offset >= quote_start && byte_offset <= q {
            return Some(source[quote_start..q].to_string());
        }
        search_from = (q + 1).min(bytes.len());
    }
    None
}

/// Mirrors `piperine_lang::SourceMapIncludeResolver::resolve`'s exact
/// search order (each namespace, then `root_path`, then as-is) but
/// checks for existence instead of reading content — the LSP crate
/// already touches the filesystem directly elsewhere (unlike the pure
/// `piperine-lang` crate, MD-43/47), so this doesn't cross a purity
/// boundary; it's only needed to turn a raw `` `include`` argument
/// string back into a URI, since `SourceProvenance` itself never
/// resolves to on-disk paths (T6 only ever recorded the argument text).
pub(crate) fn resolve_include_source_path(source_map: &piperine_lang::SourceMap, raw_path: &str) -> Option<PathBuf> {
    source_map
        .namespaces
        .values()
        .map(|ns| ns.join(raw_path))
        .chain(std::iter::once(source_map.root_path.join(raw_path)))
        .chain(std::iter::once(PathBuf::from(raw_path)))
        .find(|candidate| candidate.is_file())
}

/// Convert a pure-crate `piperine_lang::error::PreprocessedSpan` (a plain byte range,
/// T1's purity contract) into the `miette::SourceSpan` every `Resolution`
/// carries — `Design::disciplines()` keeps the surface AST shape verbatim
/// (design F1), so its span type never got the POM's own conversion.
pub(crate) fn vams_span_to_miette(span: piperine_lang::error::PreprocessedSpan) -> SourceSpan {
    SourceSpan::new(span.start.into(), span.end.saturating_sub(span.start))
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Module,
    Port,
    Param,
    Wire,
    Var,
    Instance,
    Function,
    Behavior,
    Enum,
    Bundle,
    Discipline,
    Capability,
    /// An `extern type` declaration (declared-language-surface T14) — the
    /// primitives/discipline/enum/bundle cases above already resolve
    /// through `design.disciplines()`/`enums()`/`bundles()`; this variant
    /// is specifically for `extern type` names, which have no POM-level
    /// counterpart of their own.
    Type,
    /// An `extern operator` declaration (`ddt`, `delay`, …).
    Operator,
    /// An `extern attribute` schema name (as opposed to `SymbolKind::Bundle`
    /// for a bundle-backed schema).
    AttrSchema,
}

#[derive(Debug, Clone)]
pub struct Resolution {
    pub kind: SymbolKind,
    pub name: String,
    pub decl_span: Option<SourceSpan>,
    pub type_info: Option<String>,
    /// The declaration's `///` doc comment, if any (LSP-07/08).
    pub doc: Option<String>,
    /// The real on-disk file this declaration lives in, when it differs
    /// from the current document (BUG-1/LSB-01..03) — populated only for
    /// `extern`-registry resolutions (`Type`/`Operator`/`Function`/
    /// `AttrSchema`) from `design.project().item_file(&word)`. POM-level
    /// resolutions (module/port/param/etc.) leave this `None`; T13's
    /// existing `ProjectUnit`/`cross_file_location` machinery already
    /// handles those via a different path.
    pub file: Option<PathBuf>,
}

/// Does `span` (a decl's own byte range) contain `offset`?
fn span_contains(span: Option<SourceSpan>, offset: usize) -> bool {
    match span {
        Some(s) => offset >= s.offset() && offset < s.offset() + s.len(),
        None => false,
    }
}

/// Look up `word` among one module's own declarations — innermost-first
/// (var, wire, instance, param, port, behavior, then the module's own
/// name) — never across other modules (LSP-01/02: cursor context +
/// shadowing, not a global first-match).
///
/// `byte_offset` disambiguates the behavior/module-name collision: every
/// `Behavior::name` equals its OWNING module's name (`analog PwmSwitch { }`
/// has `name == "PwmSwitch"`, same as the `mod PwmSwitch(...)` it attaches
/// to) — so a bare name match alone can't tell "clicked the `mod`
/// declaration's own name" from "clicked a same-named behavior's span".
/// Only match a behavior when the offset is actually inside *that
/// behavior's own* span; a click on the `mod` header itself never falls
/// inside any behavior's (disjoint, later-declared) span, so it falls
/// through to the module-name arm below instead.
fn resolve_in_module(m: &piperine_lang::pom::Module, word: &str, byte_offset: usize) -> Option<Resolution> {
    if let Some(v) = m.vars.iter().find(|v| v.name == word) {
        return Some(Resolution {
            kind: SymbolKind::Var,
            name: v.name.clone(),
            decl_span: v.span,
            type_info: Some(format!("{:?}", v.ty)),
            doc: v.doc.clone(),
            file: None,
        });
    }
    if let Some(w) = m.wires.iter().find(|w| w.name == word) {
        return Some(Resolution {
            kind: SymbolKind::Wire,
            name: w.name.clone(),
            decl_span: w.span,
            type_info: Some(format!("{:?}", w.ty)),
            doc: w.doc.clone(),
            file: None,
        });
    }
    // A click on an instance's *label* resolves to the instance itself
    // (the label names the instance). A click on the *type* name
    // (`i.module == word`) is deliberately NOT matched here: the type name
    // is a reference to the module, so it must fall through to the
    // module-name resolution below (step 3 of `resolve_at`) and resolve as
    // the Module — showing the module's own doc and jumping to its
    // declaration, not the (usually doc-less) instance. LSB-07..10 (T8):
    // the label's decl_span targets just the label token, not the whole
    // multi-line instance statement (`.or(i.span)` is a defensive
    // fallback).
    if let Some(i) = m.instances.iter().find(|i| i.label.as_deref() == Some(word)) {
        return Some(Resolution {
            kind: SymbolKind::Instance,
            name: i.label.clone().unwrap_or_else(|| i.module.clone()),
            decl_span: i.label_span.or(i.span),
            type_info: Some(format!("instance of {}", i.module)),
            doc: i.doc.clone(),
            file: None,
        });
    }
    if let Some(p) = m.params.iter().find(|p| p.name == word) {
        return Some(Resolution {
            kind: SymbolKind::Param,
            name: p.name.clone(),
            decl_span: p.span,
            type_info: Some(format!("{:?}", p.ty)),
            doc: p.doc.clone(),
            file: None,
        });
    }
    if let Some(p) = m.ports.iter().find(|p| p.name == word) {
        return Some(Resolution {
            kind: SymbolKind::Port,
            name: p.name.clone(),
            decl_span: p.span,
            type_info: Some(format!("{:?}", p.direction)),
            doc: p.doc.clone(),
            file: None,
        });
    }
    if let Some(b) = m.behaviors.iter().find(|b| b.name == word && span_contains(b.span, byte_offset)) {
        return Some(Resolution {
            kind: SymbolKind::Behavior,
            name: b.name.clone(),
            decl_span: b.span,
            type_info: Some(format!("{:?}", b.kind)),
            doc: b.doc.clone(),
            file: None,
        });
    }
    if m.name == word {
        return Some(Resolution {
            kind: SymbolKind::Module,
            name: m.name.clone(),
            decl_span: m.span,
            type_info: None,
            doc: m.doc.clone(),
            file: None,
        });
    }
    None
}

/// Find the `module` field of the POM `Instance` whose own `span` matches
/// `decl_span` exactly, across every module in `design` — the type name a
/// `SymbolKind::Instance` resolution names when the cursor was actually on
/// the type, not the label (`resolve_in_module`'s instance branch matches
/// on either). Shared by cross-file goto (T13) and cross-file rename
/// (T14), both of which need to recover the *type* being referenced from
/// an `Instance`-kind `Resolution`'s `decl_span` (the whole instance
/// statement's span, not a token span).
pub fn instance_module_type_at(
    design: &piperine_lang::Design,
    decl_span: miette::SourceSpan,
) -> Option<String> {
    let matches = |s: Option<miette::SourceSpan>| {
        s.is_some_and(|s| s.offset() == decl_span.offset() && s.len() == decl_span.len())
    };
    design.modules().find_map(|m| {
        m.instances
            .iter()
            // LSB-07..10 (T8): `decl_span` is now token-tight (`type_span`/
            // `label_span`), not the whole-statement `span`, so match
            // against whichever of the three the caller actually passed.
            .find(|i| matches(i.span) || matches(i.label_span) || matches(i.type_span))
            .map(|i| i.module.clone())
    })
}

pub fn resolve_at(
    design: &Design,
    source: &str,
    byte_offset: usize,
    ctx: Option<&ElabContext>,
) -> Option<Resolution> {
    // 1. Identify what we are hovering over. `byte_offset` here is a byte
    // offset into `source` — the open document's own raw text — and
    // `word_at_position`/`byte_to_position` (text_pos.rs) stay in that same
    // coordinate space unchanged (T13's own scope note: "already works
    // correctly for the open document's own coordinate space").
    let word = crate::text_pos::word_at_position(
        source,
        crate::text_pos::byte_to_position(source, byte_offset),
    )?;

    // 1b. Every `decl_span`/`span_contains` check below matches against
    // POM spans, which live in the *flat preprocessed buffer*'s coordinate
    // space (T3's mechanical rename didn't change what a `Span` numerically
    // means) — identical to `source`'s own coordinates only when the
    // document has no `` `include ``/`` `define `` directives. When it
    // does, `byte_offset` (a raw-document offset) must first be translated
    // into that flat-buffer space via the reverse lookup (T11,
    // SPANMAP-08) before it can be compared against any `decl_span`. A
    // document with no directives has one segment mapping the whole buffer
    // 1:1 to itself, so `segments_covering` degrades to the identity
    // translation — no behavior change on the common case.
    let flat_offset = design
        .source_provenance
        .segments_covering(piperine_lang::source_provenance::SourceId(0), byte_offset..byte_offset + 1)
        .next()
        .map(|s| s.start)
        .unwrap_or(byte_offset);

    // 2. Cursor context (LSP-01): if the cursor sits inside a module's own
    // declaration span, resolve `word` against *that* module's scope first,
    // innermost-first (LSP-02) — never a blind scan over every module in
    // whatever order the POM happens to iterate them.
    //
    // A `use`-imported module's `span` holds byte offsets copied through
    // unchanged from its *origin* file's own parse (`Resolver::expand`
    // inlines the AST node as-is) — those numbers are meaningless against
    // *this* document's buffer and can coincidentally overlap `byte_offset`
    // purely by chance. `design.modules()` iterates a `HashMap`, so without
    // this filter an imported module could non-deterministically outrace
    // the current file's own enclosing module for `.find()`'s first match
    // (T13/LSP-15: found via cross-file goto's flaky test). Excluding
    // imported modules from this "cursor is inside my own declaration"
    // check is correct regardless: their span can never actually contain a
    // cursor position in the current document.
    if let Some(m) = design
        .modules()
        .find(|m| design.project().origin_of(&m.name).is_none() && span_contains(m.span, flat_offset))
        && let Some(res) = resolve_in_module(m, &word, flat_offset) {
            return Some(res);
        }

    // 3. Module *names* are genuinely global in PHDL (any instance anywhere
    // may reference any module by name), so a cross-module scan for the
    // module name itself is correct here, not the word-based global-lookup
    // bug this replaces — that bug applied the same blind scan to *scoped*
    // names (ports/params/wires/vars/instances/behaviors) too, which step 2
    // above now resolves correctly (cursor-context-first) instead of
    // falling through to a global match on those kinds.
    // A `use`-imported (or full-path-referenced) module/enum/bundle/... is
    // inlined into `design` from another file; its `span` addresses that
    // *origin* file, not this document. `Design::project().item_file(name)`
    // records the real on-disk file every imported item came from — set it
    // on `Resolution.file` so `goto_def` reads that file and applies the
    // span there, instead of mis-applying it to the current document. A
    // locally-declared item has no `item_file` entry → `None` → same-file
    // goto (correct). Mirrors BUG-1's extern-goto mechanism.
    let item_file = |name: &str| design.project().item_file(name).map(|p| p.to_path_buf());

    for m in design.modules() {
        if m.name == word {
            return Some(Resolution {
                kind: SymbolKind::Module,
                name: m.name.clone(),
                decl_span: m.span,
                type_info: None,
                doc: m.doc.clone(),
                file: item_file(&m.name),
            });
        }
    }

    // MD-45: `Design::{enums,bundles,capabilities,impls}` (the PHDL-only
    // value-layer POM surface) were deleted with `piperine-lang/src/
    // parse/` at T21 — no VAMS Annex A production backs `enum`/`bundle`/
    // the capability system/`impl` blocks, so there is nothing left here
    // to resolve a word against. The four loops that used to scan them
    // are gone, not retargeted; `SymbolKind::Enum`/`Bundle`/`Capability`
    // stay declared (other files match on `SymbolKind` exhaustively) but
    // are never constructed anymore.
    for (name, d) in design.disciplines() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Discipline,
                name: name.clone(),
                decl_span: Some(vams_span_to_miette(d.span)),
                type_info: None,
                // `piperine_lang::ast::DisciplineDecl` carries no `///`
                // doc-comment field (design F1: the POM re-exports the
                // surface AST verbatim) — no doc to show.
                doc: None,
                file: item_file(name),
            });
        }
    }

    // MD-45: MD-24's `extern`-registry surface (`ElabContext::{callables,
    // types,operators,constraint_helpers,impl_methods}`) was deleted at
    // T21 along with the `extern` keyword's only producer, PHDL's parser
    // — none of it has a VAMS Annex A production (T36's `extern_coverage_
    // guard.rs` rewrite is the fuller accounting). `ElabContext` today
    // carries exactly one thing an LSP consumer can resolve against:
    // `schemas` (T35, VAMS-12/13) — the plugin system's own `device`/
    // `port` attribute-schema *names*. There is no textual declaration
    // backing them anymore (no `headers/device_port.phdl` to point at),
    // so a resolved `AttrSchema` always carries `decl_span: None` —
    // goto-definition correctly declines rather than fabricating a
    // location, the same defensive shape T20/LSP-22's `rfport` case
    // already established for a registered-but-undeclared schema.
    let ctx = ctx?;
    if ctx.schemas.contains(&word) {
        return Some(Resolution {
            kind: SymbolKind::AttrSchema,
            name: word,
            decl_span: None,
            type_info: None,
            doc: None,
            file: None,
        });
    }

    None
}
