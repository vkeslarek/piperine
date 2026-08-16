//! Hover: what the server renders for modules, disciplines, `extern`
//! operators, and attribute-schema uses — including the documented vs
//! undocumented and module-vs-same-named-behavior cases.
//!
//! Also covers T13 (`.specs/features/preprocessor-span-mapping/tasks.md`,
//! SPANMAP-08): `symbol_index::resolve_at` translates a cursor's raw
//! document byte offset into the flat-preprocessed-buffer coordinate
//! space (via `SourceProvenance::segments_covering`, T11) before matching
//! it against any POM `decl_span`, so a document with `` `include ``/
//! `` `define `` directives before the cursor doesn't misalign.

mod common;
use common::*;

/// LSP-08: hovering a `///`-documented declaration prepends the doc text as
/// Markdown above the `**kind** \`name\`` line.
#[test]
fn hover_on_documented_module_renders_doc_as_markdown() {
    let source = "discipline Electrical { potential v: Real; flow i: Real; }\n/// A two-terminal resistor.\nmod R (inout p: Electrical, inout n: Electrical) {}";
    // line 2 is `mod R (...)`; character 4 lands on `R`.
    let contents = lsp_hover_markdown(source, 2, 4);
    assert!(
        contents.contains("A two-terminal resistor."),
        "hover contents must include the doc text, got: {contents}"
    );
    assert!(contents.contains("**module** `R`"), "hover must still render the kind/name line, got: {contents}");
    // The doc text must precede the kind line (LSP-08: "prepend ... above").
    let doc_pos = contents.find("A two-terminal resistor.").unwrap();
    let kind_pos = contents.find("**module** `R`").unwrap();
    assert!(doc_pos < kind_pos, "doc text must be prepended above the kind/type line");
}

/// LSP-09: a declaration with no `///` run renders exactly as before —
/// hover is unchanged, no stray doc section appears.
#[test]
fn hover_on_undocumented_module_is_unchanged() {
    let source = "discipline Electrical { potential v: Real; flow i: Real; }\nmod R (inout p: Electrical, inout n: Electrical) {}";
    let contents = lsp_hover_markdown(source, 1, 4);
    assert!(contents.contains("**module** `R`"), "hover must render the kind/name line, got: {contents}");
    // No regression: the contents are exactly the kind line (no leading doc
    // paragraph / blank-line-separated section before it).
    assert_eq!(contents, "**module** `R`");
}

/// A module that also has a same-named `analog`/`digital` behavior block
/// (e.g. `mod PwmSwitch(...) {}` + `analog PwmSwitch { ... }`) — hover on
/// the `mod` declaration's own name must resolve as the Module (with the
/// module's own doc), not as the Behavior (whose `.name` is always the
/// SAME string as the owning module's name, so a bare name match without
/// position-awareness ambiguously prefers whichever comes first). Mirrors
/// the exact real repro from `examples/10_pwm_dimmer.phdl`.
#[test]
fn hover_on_module_with_same_named_behavior_resolves_the_module_not_the_behavior() {
    let source = "discipline Electrical { potential v: Real; flow i: Real; }\n\
/// The switch.\n\
mod PwmSwitch(inout sw: Electrical, inout gnd: Electrical) { var drive: Real = 0.0; }\n\
analog PwmSwitch { V(sw, gnd) <- drive; }\n";
    // line 2 (0-indexed) is `mod PwmSwitch(...)`; character 4 lands on `PwmSwitch`.
    let contents = lsp_hover_markdown(source, 2, 4);
    assert!(
        contents.contains("The switch."),
        "hover must show the MODULE's doc, not fall through to the same-named behavior (which has none), got: {contents}"
    );
    assert!(contents.contains("**module** `PwmSwitch`"), "hover must resolve as module, got: {contents}");
}

/// `discipline` declarations never got a `doc` field wired at all (a gap
/// distinct from BUG-2's extern-declaration fix) — hover on a documented
/// `discipline` must show its `///` doc, same as `mod`/`param`/etc.
#[test]
fn hover_on_documented_discipline_renders_doc_as_markdown() {
    let source = "/// Electrical signals: voltage and current.\ndiscipline Electrical { potential v: Real; flow i: Real; }\n";
    // line 1 is `discipline Electrical {...}`; character 13 lands on `Electrical`.
    let contents = lsp_hover_markdown(source, 1, 13);
    assert!(
        contents.contains("Electrical signals: voltage and current."),
        "hover must include the discipline's doc text, got: {contents}"
    );
    assert!(contents.contains("**discipline** `Electrical`"), "hover must resolve as discipline, got: {contents}");
}

/// spec.md's Independent Test for P2 (BUG-2): hover on a `///`-documented
/// `extern` use-site renders the doc as Markdown, same convention as an
/// already-documented `mod`/`param` (mirrors
/// `hover_on_documented_module_renders_doc_as_markdown`).
#[test]
fn hover_on_documented_extern_operator_renders_doc_as_markdown() {
    let source = "/// Time derivative of its argument.\nextern operator my_ddt(x: Real) -> Real;\ndiscipline Electrical { potential v: Real; flow i: Real; }\nmod Cap (inout p: Electrical, inout n: Electrical) {\n    param c: Real = 1.0;\n    analog Behave { I(p, n) <+ c * my_ddt(V(p, n)); }\n}\n";

    let call_offset = source.find("my_ddt(V").unwrap();
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, ch) in source[..call_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    let character = source[last_nl..call_offset].chars().count() as u32;

    let contents = lsp_hover_markdown(source, line, character);
    assert!(
        contents.contains("Time derivative of its argument."),
        "hover contents must include the extern operator's doc text, got: {contents}"
    );
}

/// LSP-09-equivalent no-regression check for BUG-2: an `extern` declaration
/// with no `///` run still hovers without a stray doc paragraph.
#[test]
fn hover_on_undocumented_extern_operator_is_unchanged() {
    let source = "extern operator my_ddt(x: Real) -> Real;\ndiscipline Electrical { potential v: Real; flow i: Real; }\nmod Cap (inout p: Electrical, inout n: Electrical) {\n    param c: Real = 1.0;\n    analog Behave { I(p, n) <+ c * my_ddt(V(p, n)); }\n}\n";

    let call_offset = source.find("my_ddt(V").unwrap();
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, ch) in source[..call_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            last_nl = i + 1;
        }
    }
    let character = source[last_nl..call_offset].chars().count() as u32;

    let contents = lsp_hover_markdown(source, line, character);
    assert!(
        !contents.to_lowercase().contains("derivative"),
        "no doc text should appear when there is no `///` run, got: {contents}"
    );
}

/// Hovering `@rfport` (a use site) lists its fields — `num` and `z0`
/// (spec.md's own P3 independent test) — as Markdown.
#[test]
fn hover_on_attr_schema_use_lists_its_fields() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical ) {\n@rfport(num = 1, z0 = 50) wire rf_in : Electrical;\n}\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.find("rfport(num").expect("use site must be present");
    let resolution = doc.resolve_at(use_site).expect("`@rfport` use site must resolve");
    assert_eq!(resolution.kind, SymbolKind::AttrSchema);

    let pos = byte_to_position(src, use_site);
    let contents = lsp_hover_markdown(src, pos.line, pos.character);
    assert!(contents.contains("num"), "expected `num` field in hover: {contents}");
    assert!(contents.contains("z0"), "expected `z0` field in hover: {contents}");
}

/// T13/SPANMAP-08: hovering a module name declared *after* a `` `define ``
/// directive (which shrinks the flat-preprocessed buffer relative to the
/// open document's own raw text — the directive's own text is eaten
/// entirely, emitting nothing) still resolves the module correctly and
/// without crashing.
///
/// **Bounded scope, documented (not silently overstated):** ideally this
/// test would prove the *cursor-context* disambiguation
/// (`resolve_at`'s `flat_offset` translation, T11's `segments_covering`
/// feeding `span_contains(m.span, flat_offset)`) picks the right
/// *enclosing module* when two modules collide, exactly as
/// `hover_on_module_with_same_named_behavior_resolves_the_module_not_the_behavior`
/// does for the plain (no-directive) case. That isn't observable today:
/// `piperine-lang`'s VAMS lowering (`elab/lower/vams.rs`) never populates
/// `Module`/`Port`/`Behavior::span` — every one is built with a literal
/// `span: None` — so `span_contains` is unconditionally `false` and every
/// resolution in this file (including the pre-existing ones above) falls
/// back to name-only matching, independent of any span-mapping fix. This
/// is a distinct, pre-existing gap in VAMS POM span population (predates
/// and is out of scope for `preprocessor-span-mapping`), not something
/// this feature's `resolve_at`/`segments_covering` plumbing can paper
/// over. What *is* verified here: the translation runs (no panic/no
/// gap-in-buffer failure) on a directive-shifted document and the module
/// still resolves by its real name — a real regression guard, even though
/// it cannot yet exercise the offset-disambiguation path this task's
/// design intends to fix once module span population lands.
#[test]
fn hover_on_module_after_a_shrinking_define_directive_still_resolves() {
    // A long, unused object-like macro definition — its own directive text
    // is eaten entirely by the preprocessor (T6), so everything after it
    // sits at a real flat-buffer offset far below its raw-document offset.
    let long_name = "A".repeat(120);
    let source = format!("`define {long_name} 1\nmodule M(inout electrical p, n);\nendmodule\n");
    let doc = analyzed(&source);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let m_offset = source.find("module M").unwrap() + "module ".len();
    let resolution = doc.resolve_at(m_offset).expect("module name must still resolve after a shrinking `define`");
    assert_eq!(resolution.kind, SymbolKind::Module);
    assert_eq!(resolution.name, "M");

    let pos = byte_to_position(&source, m_offset);
    let contents = lsp_hover_markdown(&source, pos.line, pos.character);
    assert!(contents.contains("**module** `M`"), "hover must resolve the module despite the preceding shrinking directive, got: {contents}");
}




