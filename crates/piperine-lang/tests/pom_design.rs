//! Scope: building a `Design` from sources (requirement POM-01): the retained
//! name, text and syntax tree, parse diagnostics passed through with their
//! file, determinism, and thread safety.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::msl::parser::parse;
use piperine_lang::pom::{Design, FileId};
use support::{build, codes};

#[test]
fn build_retains_name_text_and_ast() {
    let text = "module R(p, n) {}\nconst N: u32 = 8;\n";
    let design = build(text);
    let files = design.files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name(), "top.msl");
    assert_eq!(files[0].text(), text);
    assert_eq!(files[0].ast(), &parse(text).file);
}

#[test]
fn parse_diagnostics_pass_through_with_file() {
    let text = "module M(\nconst N: u32 = ;\n";
    let expected = parse(text).diagnostics;
    assert!(!expected.is_empty(), "the fixture must have a syntax error");
    let design = build(text);
    let got = design.diagnostics();
    assert_eq!(got.len(), expected.len());
    for (g, e) in got.iter().zip(&expected) {
        assert_eq!(g.file, FileId(0));
        assert_eq!(g.diagnostic.code, e.code);
        assert_eq!(g.diagnostic.span(), e.span());
    }
    assert_eq!(codes(&design), expected.iter().map(|d| d.code.to_string()).collect::<Vec<_>>());
}

#[test]
fn design_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Design>();
}
