//! `piperine fmt` must never separate a `///` doc-comment run from the
//! declaration it documents. A blank line **ends** a doc run (Part I §1.1),
//! so a formatter that inserted one between a `///` block and its
//! declaration would silently strip every doc comment it "cleaned up" —
//! the source would still read as documented while the model object,
//! hover, and any host reading `Module::doc()` all saw `None`.
//!
//! **Retargeted from PHDL**: the original fixtures were `mod`/`pub mod`/`fn`
//! declarations, and three of its cases covered `pub mod` staying on one
//! line — `pub` is not Verilog-AMS, so those are dropped rather than forced
//! onto syntax that does not exist. The doc-attachment concern itself is
//! live, and is now stronger than when this file was written: the VAMS
//! doc-run rule treats a blank line as a hard break.

use piperine_lang::fmt::Formatter;
use piperine_lang::SourceMap;

fn format(src: &str) -> String {
    Formatter::format(src).expect("formatter must succeed on valid source")
}

/// The property that actually matters, asserted the way it is actually
/// consumed: format the source, elaborate the *formatted* text, and confirm
/// the documentation still reaches the model object. A pure string check
/// could pass while the doc silently detached.
fn doc_after_formatting(src: &str, module: &str) -> Option<String> {
    let formatted = format(src);
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers);
    piperine_lang::parse_and_elaborate(&formatted, &map)
        .expect("formatted source must still elaborate")
        .module(module)
        .expect("module must survive formatting")
        .doc()
        .map(str::to_string)
}

const DOCUMENTED: &str = "`include \"disciplines.vams\"\n\
                          /// A resistor.\n\
                          module Resistor(p, n);\n\
                          \x20 inout electrical p, n;\n\
                          endmodule\n";

/// A `///` directly above `module` must stay directly above it — the
/// documentation survives formatting.
#[test]
fn doc_comment_survives_formatting_attached_to_its_module() {
    assert_eq!(doc_after_formatting(DOCUMENTED, "Resistor").as_deref(), Some("A resistor."));
}

/// A multi-line `///` block must also stay attached — the block's LAST line
/// is the one adjacent to the declaration, so a blank line inserted there
/// would strip the whole run.
#[test]
fn multiline_doc_comment_survives_formatting() {
    let src = "`include \"disciplines.vams\"\n\
               /// Line one.\n\
               ///\n\
               /// Line two.\n\
               module M(p, n);\n\
               \x20 inout electrical p, n;\n\
               endmodule\n";
    assert_eq!(
        doc_after_formatting(src, "M").as_deref(),
        Some("Line one.\n\nLine two."),
    );
}

/// A doc comment above an attribute-instanced declaration survives too —
/// the run attaches across the `(* ... *)`, so the formatter must not
/// separate it from the attribute either.
#[test]
fn doc_comment_above_an_attribute_instance_survives_formatting() {
    let src = "`include \"disciplines.vams\"\n\
               /// A plugin-bound device.\n\
               (* device = \"demo\", type = \"demo_r\" *)\n\
               module Attributed(p, n);\n\
               \x20 inout electrical p, n;\n\
               endmodule\n";
    let formatted = format(src);
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers);
    let design = piperine_lang::parse_and_elaborate_seeded(
        &formatted,
        &map,
        piperine_lang::seed_device_port_schemas,
    )
    .expect("formatted source must still elaborate");
    assert_eq!(
        design.module("Attributed").expect("module survives").doc(),
        Some("A plugin-bound device."),
    );
}

/// Formatting is idempotent over documented source — a second pass must not
/// drift the doc run away from its declaration (the failure mode where the
/// first format is safe but repeated formatting slowly detaches it).
#[test]
fn formatting_documented_source_is_idempotent() {
    let once = format(DOCUMENTED);
    let twice = format(&once);
    assert_eq!(once, twice, "format(format(x)) must equal format(x)");
    assert_eq!(doc_after_formatting(&once, "Resistor").as_deref(), Some("A resistor."));
}

/// A plain `//` comment is not documentation, and formatting must not turn
/// it into one — proof the doc-preservation behavior is scoped to `///`
/// rather than to comments in general.
#[test]
fn a_plain_comment_is_not_promoted_to_documentation_by_formatting() {
    let src = "`include \"disciplines.vams\"\n\
               // just a note to self\n\
               module Plain(p, n);\n\
               \x20 inout electrical p, n;\n\
               endmodule\n";
    assert_eq!(doc_after_formatting(src, "Plain"), None);
}
