//! Scope: the canonical MSL printer `piperine_lang::msl::print` (requirement
//! MSLP-05). Over the whole spec corpus (every `rust` block and every
//! fragment that parses in some context): printing then parsing gives an
//! equal tree (spans aside), printing again gives the same text, and every
//! doc comment and attribute survives. A fixture adds the docs and attributes
//! on every kind of node that carries them.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use piperine_lang::msl::ast::File;
use piperine_lang::msl::parser::parse;
use piperine_lang::msl::print;
use spec_corpus::{Class, corpus};

/// A fragment wrapped into the smallest context that parses it with no
/// diagnostics: a file as it is, a module body, an `analog` body, a reaction
/// body or a function body. `None` when no context accepts it.
fn fragment_source(text: &str) -> Option<String> {
    [
        text.to_string(),
        format!("module Fragment() {{\n{text}\n}}"),
        format!("analog for Fragment {{\n{text}\n}}"),
        format!("digital for Fragment {{\non event {{\n{text}\n}}\n}}"),
        format!("fn fragment() {{\n{text}\n}}"),
    ]
    .into_iter()
    .find(|src| parse(src).diagnostics.is_empty())
}

/// Every source of the corpus that parses as a file: its label and text.
fn sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for b in corpus() {
        let label = format!("{}:{}", b.file, b.line);
        match b.class {
            Class::Valid => out.push((label, b.text)),
            Class::Fragment => out.extend(fragment_source(&b.text).map(|s| (label, s))),
            Class::CompileFail => {}
        }
    }
    out
}

fn parsed(label: &str, src: &str) -> File {
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "{label}: {:?}\n{src}", r.diagnostics);
    r.file
}

#[test]
fn roundtrip_ast_equal_over_corpus() {
    let sources = sources();
    // 90 `rust` blocks plus the fragments some context accepts.
    assert!(sources.len() >= 90, "{} sources", sources.len());
    println!("round-tripped {} corpus sources", sources.len());
    for (label, src) in &sources {
        let first = parsed(label, src);
        let text = print(&first);
        let second = parsed(&format!("{label} (printed)"), &text);
        // `File: PartialEq` ignores spans, so this is `strip_spans(a) == strip_spans(b)`.
        assert!(first == second, "{label}: the printed text parses to another tree:\n{text}");
    }
}

#[test]
fn print_is_idempotent_over_corpus() {
    for (label, src) in &sources() {
        let once = print(&parsed(label, src));
        let twice = print(&parsed(&format!("{label} (printed)"), &once));
        assert_eq!(once, twice, "{label}: printing is not idempotent");
    }
}

/// The trimmed doc-comment lines of a text, in order.
fn doc_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("///") || l.starts_with("//!"))
        .map(str::to_string)
        .collect()
}

/// The attributes of a text: every `#[ .. ]` with its brackets balanced.
fn attributes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'#' && bytes[i + 1] == b'[' {
            let mut depth = 0;
            let mut j = i + 1;
            while j < bytes.len() {
                match bytes[j] {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let attr: String = text[i..=j.min(bytes.len() - 1)].split_whitespace().collect::<Vec<_>>().join("");
            out.push(attr);
            i = j;
        }
        i += 1;
    }
    out
}

#[test]
fn printer_preserves_docs_and_attrs() {
    // One fixture with a doc comment and an attribute on every node kind that
    // carries them: file, item, field, variant, impl item, module, port,
    // module item, match arm.
    let src = "//! File docs.
//! More file docs.

/// A struct.
#[derive(Repr, Clone)]
pub struct S {
    /// A field.
    #[doc = \"x\"]
    pub a: u8,
    in b: Bit,
}

/// An enum.
enum E {
    /// A variant.
    #[default]
    A,
    B(u8),
}

impl S {
    /// A method.
    #[inline]
    pub fn f(self) -> u8 {
        match self.a {
            /// An arm.
            0 => 1,
            _ => 2,
        }
    }
}

/// A module.
#[link(\"x.osdi\", model = \"m\")]
extern \"osdi\" module M(
    /// A port.
    #[unit(\"V\")]
    in p: Electrical,
    n: Electrical,
) {
    /// A param.
    #[a]
    r: f64 = 1.0;
    /// A signal.
    pub signal s: Bit;
}
";
    let first = parsed("fixture", src);
    let text = print(&first);
    assert_eq!(doc_lines(src), doc_lines(&text), "doc comments, in order:\n{text}");
    assert_eq!(attributes(src), attributes(&text), "attributes, in order:\n{text}");
    assert!(parsed("fixture (printed)", &text) == first);
    // And over the whole corpus: no doc comment or attribute is lost.
    for (label, src) in &sources() {
        let text = print(&parsed(label, src));
        assert_eq!(doc_lines(src), doc_lines(&text), "{label}: doc comments");
        assert_eq!(attributes(src), attributes(&text), "{label}: attributes");
    }
}
