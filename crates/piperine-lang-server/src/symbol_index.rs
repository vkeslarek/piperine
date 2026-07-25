use piperine_lang::elab::registry::{ElabContext, TypeDefKind};
use piperine_lang::pom::Design;
use miette::SourceSpan;
use std::path::PathBuf;

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
fn resolve_in_module(m: &piperine_lang::pom::Module, word: &str) -> Option<Resolution> {
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
    if let Some(i) = m.instances.iter().find(|i| i.label.as_deref() == Some(word) || i.module == word) {
        return Some(Resolution {
            kind: SymbolKind::Instance,
            name: i.label.clone().unwrap_or_else(|| i.module.clone()),
            decl_span: i.span,
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
    if let Some(b) = m.behaviors.iter().find(|b| b.name == word) {
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
    design.modules().find_map(|m| {
        m.instances
            .iter()
            .find(|i| {
                i.span.is_some_and(|s| s.offset() == decl_span.offset() && s.len() == decl_span.len())
            })
            .map(|i| i.module.clone())
    })
}

pub fn resolve_at(
    design: &Design,
    source: &str,
    byte_offset: usize,
    ctx: Option<&ElabContext>,
) -> Option<Resolution> {
    // 1. Identify what we are hovering over.
    let word = crate::text_pos::word_at_position(
        source,
        crate::text_pos::byte_to_position(source, byte_offset),
    )?;

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
        .find(|m| design.project().origin_of(&m.name).is_none() && span_contains(m.span, byte_offset))
        && let Some(res) = resolve_in_module(m, &word) {
            return Some(res);
        }

    // 3. Module *names* are genuinely global in PHDL (any instance anywhere
    // may reference any module by name), so a cross-module scan for the
    // module name itself is correct here, not the word-based global-lookup
    // bug this replaces — that bug applied the same blind scan to *scoped*
    // names (ports/params/wires/vars/instances/behaviors) too, which step 2
    // above now resolves correctly (cursor-context-first) instead of
    // falling through to a global match on those kinds.
    for m in design.modules() {
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
    }

    for (name, e) in design.enums() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Enum,
                name: name.clone(),
                decl_span: e.span,
                type_info: None,
                doc: None,
                file: None,
            });
        }
    }

    for (name, b) in design.bundles() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Bundle,
                name: name.clone(),
                decl_span: b.span,
                type_info: None,
                doc: None,
                file: None,
            });
        }
    }

    for (name, d) in design.disciplines() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Discipline,
                name: name.clone(),
                decl_span: d.span,
                type_info: None,
                doc: None,
                file: None,
            });
        }
    }

    for (name, c) in design.capabilities() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Capability,
                name: name.clone(),
                decl_span: c.span,
                type_info: None,
                doc: None,
                file: None,
            });
        }
    }

    for i in design.impls() {
        for m in &i.methods {
            if m.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Function,
                    name: format!("{}::{}", i.ty, m.name),
                    decl_span: m.span,
                    type_info: Some(format!("impl method for {}", i.ty)),
                    doc: None,
                    file: None,
                });
            }
        }
    }

    // declared-language-surface T14: every name resolved so far came
    // straight off the POM, which carries only *plain* declarations —
    // `extern`-declared names (types, fns/tasks, operators, attribute
    // schemas, impl methods) live in the `ElabContext` registries
    // populated at elaboration time (T11-T13's real lookup path) and have
    // no POM-level counterpart of their own. This is the first time these
    // registries have any LSP-facing consumer.
    let ctx = ctx?;

    // BUG-1 (LSB-01..03): extern-registry resolutions carry the real
    // on-disk declaring file (a stdlib header today), so goto-definition
    // can jump there instead of falling through to the same-file fallback.
    let extern_file = design.project().item_file(&word).map(PathBuf::from);

    if let Some(c) = ctx.callables.lookup(&word)
        && let Some(decl_span) = c.decl_span() {
        return Some(Resolution {
            kind: SymbolKind::Function,
            name: word,
            decl_span: Some(decl_span),
            type_info: None,
            doc: None,
            file: extern_file,
        });
    }

    if let Some(TypeDefKind::Extern { decl_span, .. }) = ctx.types.lookup(&word) {
        return Some(Resolution {
            kind: SymbolKind::Type,
            name: word,
            decl_span: *decl_span,
            type_info: None,
            doc: None,
            file: extern_file,
        });
    }

    if let Some(c) = ctx.operators.lookup(&word)
        && let Some(decl_span) = c.decl_span() {
        return Some(Resolution {
            kind: SymbolKind::Operator,
            name: word,
            decl_span: Some(decl_span),
            type_info: None,
            doc: None,
            file: extern_file,
        });
    }

    // SPEC_DEVIATION (T20/LSP-22): previously gated on `decl_span.is_some()`,
    // so a registered schema with no textual declaration (e.g. the built-in
    // `rfport`, registered directly in `ElabContext::new()`) never resolved
    // at all — hover on `@rfport` (spec.md's own P3 independent test)
    // couldn't show its fields. Resolve any registered schema name;
    // `decl_span` stays `None` when the schema has no textual source, so
    // goto-definition correctly declines instead of fabricating a location.
    if ctx.schemas.shape(&word).is_some() {
        let decl_span = ctx.schemas.decl_span(&word);
        return Some(Resolution {
            kind: SymbolKind::AttrSchema,
            name: word,
            decl_span,
            type_info: None,
            doc: None,
            file: extern_file,
        });
    }

    if let Some(c) = ctx.impl_methods.find_by_method_name(&word)
        && let Some(decl_span) = c.decl_span() {
        return Some(Resolution {
            kind: SymbolKind::Function,
            name: word,
            decl_span: Some(decl_span),
            type_info: None,
            doc: None,
            file: extern_file,
        });
    }

    None
}
