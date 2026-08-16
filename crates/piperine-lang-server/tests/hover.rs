//! Hover: what the server renders for modules, disciplines, parameters,
//! and attribute-schema uses — including the documented vs undocumented
//! and module-vs-same-named-behavior cases.
//!
//! Also covers T13 (`.specs/features/preprocessor-span-mapping/tasks.md`,
//! SPANMAP-08): `symbol_index::resolve_at` translates a cursor's raw
//! document byte offset into the flat-preprocessed-buffer coordinate
//! space (via `SourceProvenance::segments_covering`, T11) before matching
//! it against any POM `decl_span`, so a document with `` `include ``/
//! `` `define `` directives before the cursor doesn't misalign.
//!
//! **lang-server-vams-migration (T6)**: every fixture below is real,
//! parseable Verilog-AMS. Two PHDL-only scenarios have no VAMS equivalent
//! and were retargeted rather than deleted (spec.md's Edge Cases: "the
//! smallest valid VAMS construct that preserves the same *kind* of
//! target"):
//! - `extern operator` (a user-declared textual operator) was deleted
//!   outright with the PHDL parser (MD-45/T21) — `symbol_index::resolve_at`
//!   never constructs `SymbolKind::Operator`/`Type` anymore. The
//!   documented/undocumented-declaration-hover scenario these two tests
//!   checked is retargeted onto a `parameter` declaration instead (a real,
//!   still-hoverable `SymbolKind::Param`), the closest surviving "a
//!   declared item may carry a `///` doc comment" case.
//! - The old `@rfport(num = .., z0 = ..)` hover-shows-its-fields scenario
//!   has no equivalent either: VAMS re-anchored the schema mechanism on
//!   exactly two fixed names, `device`/`port` (`(* device = "plugin", type
//!   = ".." *)` / `(* port = "name", kind = ".." *)`,
//!   `piperine_lang::seed_device_port_schemas`), and field-summary hover
//!   was removed with the old bundle-backed `SchemaShape` registry
//!   (MD-45, documented in `handlers/hover.rs`'s own comment) — an
//!   `AttrSchema` resolution shows only its name today, never a field
//!   list. Retargeted to assert the real, current behavior.

mod common;
use common::*;

/// LSP-08: hovering a `///`-documented declaration prepends the doc text as
/// Markdown above the `**kind** \`name\`` line.
///
/// **Blocked**: `elab/lower/vams.rs` hardcodes every POM node's `doc`
/// field to `None` — VAMS lowering never captures `///` comments at all (a
/// distinct gap from the known `span: None` one, same file, same class of
/// "elaborator wiring, not lang-server wiring" — out of scope for this
/// feature, see spec.md's Out of Scope). No declaration kind can render a
/// doc today, so this assertion cannot pass until that lowering gap closes.
#[test]
fn hover_on_documented_module_renders_doc_as_markdown() {
    let source = "/// A two-terminal resistor.\nmodule R(inout electrical p, n);\nendmodule\n";
    // line 1 is `module R(inout electrical p, n);`; character 7 lands on `R`.
    let contents = lsp_hover_markdown(source, 1, 7);
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
    let source = "module R(inout electrical p, n);\nendmodule\n";
    // line 0 is `module R(inout electrical p, n);`; character 7 lands on `R`.
    let contents = lsp_hover_markdown(source, 0, 7);
    assert!(contents.contains("**module** `R`"), "hover must render the kind/name line, got: {contents}");
    // No regression: the contents are exactly the kind line (no leading doc
    // paragraph / blank-line-separated section before it).
    assert_eq!(contents, "**module** `R`");
}

/// A module that also has a same-named `analog` behavior block (VAMS
/// lowers an `analog begin ... end` block into a POM `Behavior` whose
/// `.name` is always the OWNING module's own name, `elab/lower/
/// vams.rs::Behavior::new(decl.name.clone(), ...)`, mirroring PHDL's
/// `analog PwmSwitch { ... }` convention exactly) — hover on the `module`
/// declaration's own name must resolve as the Module (with the module's
/// own doc), not as the Behavior (whose `.name` is always the SAME string
/// as the owning module's name, so a bare name match without
/// position-awareness ambiguously prefers whichever comes first).
///
/// **Blocked, but not for the reason the test name implies**: because
/// `Module`/`Behavior::span` are always `None` in `elab/lower/vams.rs`
/// (the feature's documented out-of-scope gap — spec.md's Out of Scope
/// table), `resolve_at`'s cursor-context step (`symbol_index.rs` step 2,
/// the only place `Behavior` is ever a resolution candidate) never
/// engages — meaning `Behavior` is never even in the running as a global
/// candidate (step 3 only scans module/discipline/schema names). So this
/// resolves as `Module` today regardless of where the cursor actually
/// sits (a coincidence of the fallback shape, not real disambiguation),
/// and the assertion fails purely on the separate `doc: None` gap (no doc
/// text is ever rendered) — not on a module-vs-behavior mixup. Once
/// `Module::span` is populated, `Behavior` becomes a real competing
/// candidate again and this test starts exercising its original intent.
#[test]
fn hover_on_module_with_same_named_behavior_resolves_the_module_not_the_behavior() {
    let source = "\
/// The switch.\n\
module PwmSwitch(inout electrical sw, gnd);\n\
  real drive;\n\
  analog begin\n\
    V(sw, gnd) <+ drive;\n\
  end\n\
endmodule\n";
    // line 1 (0-indexed) is `module PwmSwitch(inout electrical sw, gnd);`; character 7 lands on `PwmSwitch`.
    let contents = lsp_hover_markdown(source, 1, 7);
    assert!(
        contents.contains("The switch."),
        "hover must show the MODULE's doc, not fall through to the same-named behavior (which has none), got: {contents}"
    );
    assert!(contents.contains("**module** `PwmSwitch`"), "hover must resolve as module, got: {contents}");
}

/// `discipline` declarations never got a `doc` field wired at all — hover
/// on a documented `discipline` must show its `///` doc, same as
/// `module`/`param`/etc.
///
/// **Blocked**: `symbol_index::resolve_at`'s `Discipline` arm hardcodes
/// `doc: None` unconditionally (its own comment: "`piperine_lang::ast::
/// DisciplineDecl` carries no `doc` field ... design F1: the POM re-exports
/// the surface AST verbatim") — a parser/POM-shape gap, out of scope for
/// this feature.
#[test]
fn hover_on_documented_discipline_renders_doc_as_markdown() {
    let source = "/// Electrical signals: voltage and current.\ndiscipline MyElectrical;\n    potential Volt;\n    flow Amp;\nenddiscipline\nnature Volt;\n  units = \"V\";\n  access = V;\n  abstol = 1e-6;\nendnature\nnature Amp;\n  units = \"A\";\n  access = I;\n  abstol = 1e-12;\nendnature\n";
    // line 1 is `discipline MyElectrical;`; character 11 lands on `MyElectrical`.
    let contents = lsp_hover_markdown(source, 1, 11);
    assert!(
        contents.contains("Electrical signals: voltage and current."),
        "hover must include the discipline's doc text, got: {contents}"
    );
    assert!(contents.contains("**discipline** `MyElectrical`"), "hover must resolve as discipline, got: {contents}");
}

/// Retargeted from the deleted `extern operator` mechanism (see file-level
/// doc): the documented-declaration-hover scenario, now on a `parameter`.
///
/// **Blocked, more severely than the module case above**: `resolve_at`'s
/// only path to a `SymbolKind::Param` resolution is the cursor-context
/// step (`symbol_index.rs` step 2, `resolve_in_module`), which — like the
/// module/discipline cases — never engages because `Module::span` is
/// always `None`. Unlike `Module`/`Discipline`/`AttrSchema`, there is no
/// step-3 global fallback for `Param` at all (step 3 only scans module
/// names, discipline names, and schema names), so hovering a parameter
/// returns no resolution whatsoever (a `null` hover response, not merely a
/// docless one) until `Module::span` population lands. Same `doc: None`
/// gap would also block the doc text specifically, once span is fixed.
#[test]
fn hover_on_documented_parameter_renders_doc_as_markdown() {
    let source = "module Cap(inout electrical p, n);\n  /// Time derivative coefficient.\n  parameter real c = 1.0;\n  analog begin\n    I(p, n) <+ c * ddt(V(p, n));\n  end\nendmodule\n";
    // line 2 is `  parameter real c = 1.0;`; character after `parameter real ` lands on `c`.
    let character = "  parameter real ".chars().count() as u32;
    let contents = lsp_hover_markdown(source, 2, character);
    assert!(
        contents.contains("Time derivative coefficient."),
        "hover contents must include the parameter's doc text, got: {contents}"
    );
}

/// LSP-09-equivalent no-regression check, retargeted the same way: a
/// `parameter` with no `///` run still hovers without a stray doc
/// paragraph.
///
/// **Blocked, same reason as `hover_on_documented_parameter_renders_doc_
/// as_markdown` above**: no `SymbolKind::Param` resolution reaches this
/// document at all without cursor context (`Module::span: None`), so hover
/// returns `null` rather than a docless-but-present `Hover` — this is a
/// stronger failure than "no regression," not merely a doc-rendering gap.
#[test]
fn hover_on_undocumented_parameter_is_unchanged() {
    let source = "module Cap(inout electrical p, n);\n  parameter real c = 1.0;\n  analog begin\n    I(p, n) <+ c * ddt(V(p, n));\n  end\nendmodule\n";
    let character = "  parameter real ".chars().count() as u32;
    let contents = lsp_hover_markdown(source, 1, character);
    assert!(
        !contents.to_lowercase().contains("derivative"),
        "no doc text should appear when there is no `///` run, got: {contents}"
    );
}

/// Retargeted from the deleted `@rfport(...)` field-summary hover scenario
/// (see file-level doc): hovering the `device` word inside a
/// `(* device = "..", type = ".." *)` attribute instance resolves as an
/// `AttrSchema` and shows its name — field-summary hover was removed
/// outright (MD-45, `handlers/hover.rs`'s own comment), so this asserts
/// the real, current behavior rather than the deleted one.
#[test]
fn hover_on_attr_schema_use_shows_its_name() {
    let src = "(* device = \"none\", type = \"host_resistor\" *)\nmodule HostR(p, n);\n  inout electrical p, n;\nendmodule\n";
    let doc = analyzed(src);
    assert!(doc.design.is_some(), "source must elaborate cleanly: {:?}", doc.errors);

    let use_site = src.find("device").expect("use site must be present");
    let resolution = doc.resolve_at(use_site).expect("`device` use site must resolve");
    assert_eq!(resolution.kind, SymbolKind::AttrSchema);

    let pos = byte_to_position(src, use_site);
    let contents = lsp_hover_markdown(src, pos.line, pos.character);
    assert_eq!(contents, "**attribute schema** `device`", "expected the bare schema-name hover, got: {contents}");
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
