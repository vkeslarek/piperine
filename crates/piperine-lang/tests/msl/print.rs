//! Scope: the canonical MSL printer `piperine_lang::msl::print` (requirement
//! MSLP-05). Over the whole spec corpus (every `rust` block and every
//! fragment that parses in some context): printing then parsing gives an
//! equal tree (spans aside), printing again gives the same text, and every
//! doc comment and attribute survives. A fixture adds the docs and attributes
//! on every kind of node that carries them.


use piperine_lang::msl::ast::File;
use piperine_lang::msl::parser::SourceParser;
use piperine_lang::msl::Printer;
use piperine_lang_fixtures::fragments::FragmentSource;
use piperine_lang_fixtures::spec_corpus::{BlockClass, SpecCorpus};

/// Every source of the specification corpus that parses as a file.
struct PrintCorpus;

impl PrintCorpus {
    /// Every source of the corpus that parses as a file: its label and text.
    fn sources() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for b in SpecCorpus::corpus() {
            let label = format!("{}:{}", b.file, b.line);
            match b.class {
                BlockClass::Valid => out.push((label, b.text)),
                BlockClass::Fragment => out.extend(FragmentSource::wrapped(&b.text).map(|s| (label, s))),
                BlockClass::CompileFail => {}
            }
        }
        out
    }
}

/// Parses a source and prints it back, asserting the tree and the text are fixed points.
struct PrintAndParse;

impl PrintAndParse {
    fn parsed(label: &str, src: &str) -> File {
        let r = SourceParser::parse(src);
        assert!(r.diagnostics.is_empty(), "{label}: {:?}\n{src}", r.diagnostics);
        r.file
    }

    /// `src` printed, re-parsed and printed again: the tree is equal and the text
    /// is a fixed point.
    fn assert_roundtrip(label: &str, src: &str) -> String {
        let first = PrintAndParse::parsed(label, src);
        let text = Printer::print(&first);
        let second = PrintAndParse::parsed(&format!("{label} (printed)"), &text);
        assert!(first == second, "{label}: the printed text parses to another tree:\n{text}");
        assert_eq!(text, Printer::print(&second), "{label}: not idempotent");
        text
    }
}

/// Reads the doc-comment lines and the attributes of a text.
struct DocumentationText;

impl DocumentationText {
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
}

#[test]
fn roundtrip_ast_equal_over_corpus() {
    let sources = PrintCorpus::sources();
    // 90 `rust` blocks plus the fragments some context accepts.
    assert!(sources.len() >= 90, "{} sources", sources.len());
    println!("round-tripped {} corpus sources", sources.len());
    for (label, src) in &sources {
        let first = PrintAndParse::parsed(label, src);
        let text = Printer::print(&first);
        let second = PrintAndParse::parsed(&format!("{label} (printed)"), &text);
        // `File: PartialEq` ignores spans, so this is `strip_spans(a) == strip_spans(b)`.
        assert!(first == second, "{label}: the printed text parses to another tree:\n{text}");
    }
}

#[test]
fn print_is_idempotent_over_corpus() {
    for (label, src) in &PrintCorpus::sources() {
        let once = Printer::print(&PrintAndParse::parsed(label, src));
        let twice = Printer::print(&PrintAndParse::parsed(&format!("{label} (printed)"), &once));
        assert_eq!(once, twice, "{label}: printing is not idempotent");
    }
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
    let first = PrintAndParse::parsed("fixture", src);
    let text = Printer::print(&first);
    assert_eq!(DocumentationText::doc_lines(src), DocumentationText::doc_lines(&text), "doc comments, in order:\n{text}");
    assert_eq!(DocumentationText::attributes(src), DocumentationText::attributes(&text), "attributes, in order:\n{text}");
    assert!(PrintAndParse::parsed("fixture (printed)", &text) == first);
    // And over the whole corpus: no doc comment or attribute is lost.
    for (label, src) in &PrintCorpus::sources() {
        let text = Printer::print(&PrintAndParse::parsed(label, src));
        assert_eq!(DocumentationText::doc_lines(src), DocumentationText::doc_lines(&text), "{label}: doc comments");
        assert_eq!(DocumentationText::attributes(src), DocumentationText::attributes(&text), "{label}: attributes");
    }
}

#[test]
fn string_literals_are_printed_verbatim() {
    // MSLP-05 AC 1/2: a string's contents are Part I values; layout never
    // re-indents the continuation lines of a multi-line or `\`-continued one.
    for (label, src) in [
        ("multi-line", "fn f() { let s = \"a\n  b\"; }"),
        ("continued", "fn f() { let s = \"a \\\n      b\"; }"),
        ("nested block", "fn f() { if c { let s = \"x\n\ty  \n\nz\"; } }"),
        ("in a call", "fn f() { g(\"one\ntwo\", \"three\"); }"),
    ] {
        let text = PrintAndParse::assert_roundtrip(label, src);
        // The literal's bytes appear unchanged in the output.
        let lit = &src[src.find('"').unwrap()..=src.find('"').map(|i| i + src[i + 1..].find('"').unwrap() + 1).unwrap()];
        assert!(text.contains(lit), "{label}: {lit:?} not verbatim in\n{text}");
    }
}

#[test]
fn block_doc_comments_are_kept() {
    // Appendix A.4 / Rust: `/** */` and `/*! */` are doc comments, outer and
    // inner, and printing keeps them.
    let src = "/*! inner\n   more */\n/** outer */\nfn f() {}\n/** a\n    b */\nstruct S { /** field */ a: u8 }\n";
    let first = PrintAndParse::parsed("block docs", src);
    assert_eq!(first.inner_docs.len(), 1);
    assert_eq!(first.items[0].meta.docs.len(), 1);
    let text = PrintAndParse::assert_roundtrip("block docs", src);
    assert!(text.contains("/*! inner\n   more */"), "{text}");
    assert!(text.contains("/** outer */"), "{text}");
    assert!(text.contains("/** a\n    b */"), "{text}");
    assert!(text.contains("/** field */"), "{text}");
}

#[test]
fn long_chains_print_without_overflow() {
    // A left-nested sum and an `else if` chain are as deep as they are long;
    // at the nesting limit they print on an 8 MiB stack, and past it the
    // parser's E0910 keeps the tree shallow enough to print.
    let handle = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            let sum = format!("fn f() -> f64 {{ 1{} }}", "+1".repeat(4000));
            let text = PrintAndParse::assert_roundtrip("sum 4000", &sum);
            assert!(text.len() > 500);
            let chain = format!("fn f() {{ if a {{}} {}}}", "else if a {} ".repeat(4000));
            PrintAndParse::assert_roundtrip("else-if 4000", &chain);
            for src in [
                format!("fn f() -> f64 {{ 1{} }}", "+1".repeat(5000)),
                format!("fn f() {{ if a {{}} {}}}", "else if a {} ".repeat(5000)),
            ] {
                let r = SourceParser::parse(&src);
                assert!(r.diagnostics.iter().any(|d| d.code == "E0910"));
                let _ = Printer::print(&r.file);
            }
        })
        .expect("spawn");
    handle.join().expect("no stack overflow");
}

#[test]
fn inherited_lexical_forms_roundtrip() {
    // Raw strings print as the equal cooked string; `1.`, `let ... else` and
    // non-ASCII identifiers print back as written.
    let text = PrintAndParse::assert_roundtrip(
        "lexical forms",
        "fn café() -> f64 { let Some(x) = o else { return 1.; }; let s = r#\"a\\b\"q\"#; x + 1. }",
    );
    assert!(text.contains("café") && text.contains("else") && text.contains("1."), "{text}");
}

#[test]
fn raw_identifiers_print_with_their_prefix() {
    // POL-10 AC 3: `r#out` prints as `r#out`; the reparse equals the first
    // tree (the `raw` flag is part of tree equality) and a second print is the same.
    let src = "module M() {\n    signal r#out: Bit;\n    let x = r#out;\n    r#out = x;\n}\nfn r#on(r#flip: u8) -> u8 { r#flip }";
    let first = PrintAndParse::parsed("raw", src);
    let text = Printer::print(&first);
    for word in ["r#out", "r#on", "r#flip"] {
        assert!(text.contains(word), "{word} missing from:\n{text}");
    }
    let second = PrintAndParse::parsed("raw (printed)", &text);
    assert!(first == second, "reparse differs:\n{text}");
    assert_eq!(Printer::print(&second), text);
}

#[test]
fn field_shorthand_prints_as_written() {
    // POL-11 AC 2: `{ r }` prints `{ r }`, `{ r: r }` prints `{ r: r }`; each
    // reparses to its own tree.
    let short = "module T() { let x = R(a, b) { r }; let y = R { r, c: 1p }; }";
    let long = "module T() { let x = R(a, b) { r: r }; let y = R { r: r, c: 1p }; }";
    let (s, l) = (PrintAndParse::parsed("short", short), PrintAndParse::parsed("long", long));
    let (st, lt) = (Printer::print(&s), Printer::print(&l));
    assert!(st.contains("{ r }") && st.contains("{ r, c: 1p }"), "{st}");
    assert!(lt.contains("{ r: r }") && lt.contains("{ r: r, c: 1p }"), "{lt}");
    assert!(s != l, "the two forms are different trees");
    assert!(PrintAndParse::parsed("short2", &st) == s && PrintAndParse::parsed("long2", &lt) == l);
    assert_eq!(Printer::print(&PrintAndParse::parsed("short3", &st)), st);
}

#[test]
fn statement_and_hardware_docs_round_trip() {
    // POL-12 AC 4: docs and attributes at every new position survive a print:
    // the reparse equals the first tree and a second print is the same text.
    let src = "\
fn f() {
    /// a let
    #[x]
    let y = 1;
    /// a call
    g();
    /// a block
    if y { }
}
nature N {
    /// pot
    potential: V { access: v },
    /// flow
    flow: I { access: i },
}
nature L {
    /// value
    value: bool,
}
module M() {
    /// def
    #[d]
    s = x after 1;
    /// gen
    for i in 0..2 { /// inner
        s = i; }
    /// gen if
    if a { } else { }
}
analog for M {
    /// contribution
    #[c]
    i(a) +> 1.0;
}
digital for M {
    /// reaction
    on e {
        /// commit
        q <- 1;
    }
    /// dgen
    for i in 0..2 { /// inner reaction
        on x { } }
}
";
    let first = PrintAndParse::parsed("docs", src);
    let text = Printer::print(&first);
    for doc in [
        "/// a let", "/// a call", "/// a block", "/// pot", "/// flow", "/// value", "/// def", "/// gen",
        "/// inner", "/// gen if", "/// contribution", "/// reaction", "/// commit", "/// dgen", "/// inner reaction",
    ] {
        assert!(text.contains(doc), "{doc} lost in:\n{text}");
    }
    let second = PrintAndParse::parsed("docs (printed)", &text);
    assert!(first == second, "the printed text parses to another tree:\n{text}");
    assert_eq!(Printer::print(&second), text);
}

#[test]
fn raw_strings_print_as_written() {
    // POL-13 AC 6: a raw string keeps its spelling, hashes and body.
    for lit in [r##"r#"a"b\c"#"##, r#"r"x\y""#, r###"r##"q"#q"##"###, r#"r"""#] {
        let src = format!("fn f() {{ let s = {lit}; }}");
        let text = PrintAndParse::assert_roundtrip(lit, &src);
        assert!(text.contains(lit), "{lit} is not written back in\n{text}");
    }
    // An ordinary string stays ordinary.
    let text = PrintAndParse::assert_roundtrip("plain", r#"fn f() { let s = "a\"b"; }"#);
    assert!(text.contains(r#""a\"b""#), "{text}");
}

#[test]
fn the_printer_and_the_object_model_hold_no_removed_form() {
    // VS-09 AC 3 (B.3.6, G.71): no reference, marker or slice is a node of the syntax tree or of the
    // object model, so none can be printed or read back. The definitions name none of them, and no
    // source of the corpus prints one of the three markers.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in ["msl/ast.rs", "pom/body.rs"] {
        let text = std::fs::read_to_string(root.join(file)).expect("the definition file is UTF-8");
        for removed in ["by_ref", "static_lifetime", "is_move", "Deref", "Slice(", "Ref(", "Ref {", "mutable"] {
            assert!(!text.contains(removed), "{file} still names `{removed}`");
        }
    }
    for (label, src) in PrintCorpus::sources() {
        let printed = Printer::print(&PrintAndParse::parsed(&label, &src));
        let (tokens, _) = piperine_lang::msl::lexer::Lexer::lex(&printed);
        let marker = tokens.iter().find(|(t, _)| matches!(t, piperine_lang::msl::lexer::Token::Kw(piperine_lang::msl::lexer::Kw::Mut | piperine_lang::msl::lexer::Kw::Ref | piperine_lang::msl::lexer::Kw::Move)));
        assert!(marker.is_none(), "{label}: the printer wrote a marker: {marker:?}");
    }
}
