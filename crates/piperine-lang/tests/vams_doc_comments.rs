//! `///` doc comments survive lexing, parsing, and elaboration, landing on
//! the POM node the source documented — the pipeline LSP hover reads
//! (`piperine-lang-server`'s `Resolution::doc`) and any host reads through
//! `Module::doc()`/`Param::doc()`/…
//!
//! Rebuilt for the VAMS frontend: the lexer always captured comments and
//! the POM always had `doc` fields, but nothing joined them until
//! `grammar/cursor.rs`'s doc-run side table — see its `doc_at` docs for
//! why the runs are attached out of band rather than left in the token
//! stream.

use piperine_lang::SourceMap;

fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers);
    map
}

fn elaborate(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("fixture must elaborate")
}

/// The README's own documented-resistor example, asserted end to end.
#[test]
fn doc_runs_reach_module_param_and_port() {
    let design = elaborate(
        "`include \"disciplines.vams\"\n\
         /// A two-terminal resistor.\n\
         /// Obeys Ohm's law; `r` is the resistance in ohms.\n\
         module Resistor (inout electrical p, inout electrical n);\n\
         \x20   /// Resistance, in ohms. Must be positive.\n\
         \x20   parameter real r = 1e3 from (0:inf);\n\
         \x20   analog I(p, n) <+ V(p, n) / r;\n\
         endmodule\n",
    );
    let m = design.module("Resistor").expect("module elaborates");

    assert_eq!(
        m.doc(),
        Some("A two-terminal resistor.\nObeys Ohm's law; `r` is the resistance in ohms."),
        "a multi-line /// run joins with newlines, each line's marker and padding stripped"
    );
    assert_eq!(
        m.params().iter().find(|p| p.name == "r").and_then(|p| p.doc.as_deref()),
        Some("Resistance, in ohms. Must be positive."),
        "a parameter's own /// run lands on that parameter, not on the enclosing module"
    );
}

/// Only `///` is a doc comment — a plain `//` above a declaration is an
/// ordinary comment and must not be attached (otherwise every incidental
/// remark above a module would render as its documentation).
#[test]
fn plain_line_comments_are_not_docs() {
    let design = elaborate(
        "`include \"disciplines.vams\"\n\
         // just a note to self, not documentation\n\
         module Plain (inout electrical p, inout electrical n);\n\
         endmodule\n",
    );
    assert_eq!(design.module("Plain").expect("elaborates").doc(), None);
}

/// A `///` run separated from the declaration by an ordinary `//` comment
/// is broken by it — the non-doc comment ends the run, so nothing leaks
/// onto the declaration below.
#[test]
fn a_plain_comment_breaks_a_doc_run() {
    let design = elaborate(
        "`include \"disciplines.vams\"\n\
         /// documentation for something else entirely\n\
         // an ordinary comment in between\n\
         module Broken (inout electrical p, inout electrical n);\n\
         endmodule\n",
    );
    assert_eq!(design.module("Broken").expect("elaborates").doc(), None);
}

/// A doc comment above an attribute-instanced declaration still attaches:
/// the run is keyed on the declaration's first token, which is the `(*`
/// of the attribute instance, not the `module` keyword.
#[test]
fn doc_attaches_across_an_attribute_instance() {
    let src = "`include \"disciplines.vams\"\n\
               /// A plugin-bound device.\n\
               (* device = \"demo\", type = \"demo_r\" *)\n\
               module Attributed (inout electrical p, inout electrical n);\n\
               endmodule\n";
    let design = piperine_lang::parse_and_elaborate_seeded(
        src,
        &source_map(),
        piperine_lang::seed_device_port_schemas,
    )
    .expect("fixture with @device must elaborate");

    assert_eq!(
        design.module("Attributed").expect("elaborates").doc(),
        Some("A plugin-bound device."),
        "the doc run must survive an intervening (* ... *) attribute instance"
    );
}

/// A blank line inside a `///` run does not end it — the run is still one
/// document (Part I §1.1: only an ordinary comment or a real token breaks
/// it). Guards the exact claim the spec makes.
#[test]
fn a_blank_line_does_not_break_a_doc_run() {
    let design = elaborate(
        "`include \"disciplines.vams\"\n\
         /// First paragraph.\n\
         ///\n\
         /// Second paragraph, after a blank doc line.\n\
         module Spaced (inout electrical p, inout electrical n);\n\
         endmodule\n",
    );
    assert_eq!(
        design.module("Spaced").expect("elaborates").doc(),
        Some("First paragraph.\n\nSecond paragraph, after a blank doc line."),
    );
}

/// Ports, nets, and variables each carry their own run — asserted
/// together so the spec's "which declarations carry documentation" list
/// cannot drift from what the pipeline actually populates. An
/// undocumented sibling on the same construct stays `None` (no leak).
#[test]
fn doc_runs_reach_ports_nets_and_variables() {
    let design = elaborate(
        "`include \"disciplines.vams\"\n\
         module M(p, n);\n\
         \x20 /// The positive terminal.\n\
         \x20 inout electrical p;\n\
         \x20 inout electrical n;\n\
         \x20 /// An internal node.\n\
         \x20 electrical mid;\n\
         \x20 /// A counter.\n\
         \x20 integer cnt;\n\
         endmodule\n",
    );
    let m = design.module("M").expect("elaborates");

    let port = |name: &str| m.ports().iter().find(|x| x.name == name).and_then(|x| x.doc.as_deref());
    assert_eq!(port("p"), Some("The positive terminal."));
    assert_eq!(port("n"), None, "an undocumented port must not inherit its sibling's doc");
    assert_eq!(
        m.wires().iter().find(|x| x.name == "mid").and_then(|x| x.doc.as_deref()),
        Some("An internal node."),
    );
    assert_eq!(
        m.vars.iter().find(|x| x.name == "cnt").and_then(|x| x.doc.as_deref()),
        Some("A counter."),
    );
}

/// Disciplines and natures are documented the same way — a discipline's
/// doc is read straight off the surface AST node (design F1), a nature's
/// off its POM mirror.
#[test]
fn doc_runs_reach_natures_and_disciplines() {
    let design = elaborate(
        "/// Electrical potential, in volts.\n\
         nature Voltage;\n\
         \x20 units = \"V\";\n\
         \x20 access = V;\n\
         \x20 abstol = 1e-6;\n\
         endnature\n\
         nature Current;\n\
         \x20 units = \"A\";\n\
         \x20 access = I;\n\
         \x20 abstol = 1e-12;\n\
         endnature\n\
         /// The conservative electrical discipline.\n\
         discipline electrical;\n\
         \x20 potential Voltage;\n\
         \x20 flow Current;\n\
         enddiscipline\n",
    );

    assert_eq!(
        design.nature("Voltage").and_then(|n| n.doc()),
        Some("Electrical potential, in volts."),
    );
    assert_eq!(
        design.disciplines().find(|(name, _)| *name == "electrical").and_then(|(_, d)| d.doc.as_deref()),
        Some("The conservative electrical discipline."),
    );
}
