//! Scope: the comment-preserving formatter of MSL (msl-cli CLI-23):
//! `piperine_lang::msl::comments` (the plain comments beside the tokens and
//! their anchors) and `piperine_lang::msl::format` (a formatted text that
//! keeps every comment, the tree and its own fixed point). Expected texts are
//! written by hand from the acceptance criterion each test cites. The corpus
//! tests (CLI-23 AC 10) run the formatter over every MSL file of the tree's
//! libraries, examples, twins and test fixtures, every block of the
//! specification and every `msl` block of the Book.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use std::path::{Path, PathBuf};

use piperine_lang::msl::Span;
use piperine_lang::msl::comments::{Anchor, Comment, Comments};
use piperine_lang::msl::format::Formatter;
use piperine_lang::msl::parser::parse;
use spec_corpus::{Class, corpus};

/// The span from the first `from` of `src` to the end of the first `to` after it.
fn between(src: &str, from: &str, to: &str) -> Span {
    let start = src.find(from).unwrap_or_else(|| panic!("`{from}` is in the source"));
    let len = src[start..].find(to).unwrap_or_else(|| panic!("`{to}` follows `{from}`")) + to.len();
    Span::new(start, start + len)
}

/// The anchors of the comments of `src`, which must parse.
fn anchors(src: &str) -> (Vec<Anchor>, Vec<Span>) {
    let parsed = parse(src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let attached = Comments::of(src).attach(&parsed.file, src);
    (attached.anchors().to_vec(), attached.verbatim().to_vec())
}

/// CLI-23 AC 1 (C175): `Comments::of` lists a `//` line, a trailing `//` and
/// a `/* */`, each with its span, its text and whether it starts its line, and
/// leaves the `///`, `//!`, `/** */` and `/*! */` doc comments to the tokens.
#[test]
fn plain_comments_are_listed_with_span_text_and_line_start() {
    let src = "//! inner doc\n// own line\n/// outer doc\nfn f() {\n    let x = 1; // trailing\n    /* block */ x\n}\n/** block doc */\n/*! inner block doc */\n";
    let at = |needle: &str| {
        let start = src.find(needle).unwrap_or_else(|| panic!("{needle} is in the source"));
        Span::new(start, start + needle.len())
    };
    let expected = vec![
        Comment { span: at("// own line"), text: "// own line".to_string(), starts_line: true },
        Comment { span: at("// trailing"), text: "// trailing".to_string(), starts_line: false },
        Comment { span: at("/* block */"), text: "/* block */".to_string(), starts_line: true },
    ];
    assert_eq!(Comments::of(src).list(), expected.as_slice());
    assert_eq!(Comments::of(src).texts(), vec!["// own line", "// trailing", "/* block */"]);

    // Rust's rule for what is no doc comment: `////`, `/***` and the empty `/**/` are plain.
    let plain = "//// four\n/*** three */\n/**/\n";
    assert_eq!(Comments::of(plain).texts(), vec!["//// four", "/*** three */", "/**/"]);

    // A line comment ends before the `\r` of a CRLF line; a nested block comment is one comment.
    let crlf = "let a = 1; // crlf\r\n/* outer /* inner */ still */\r\n";
    let list = Comments::of(crlf);
    assert_eq!(list.texts(), vec!["// crlf", "/* outer /* inner */ still */"]);
    assert_eq!(list.list()[0].span, Span::new(11, 18));
    assert!(!list.list()[0].starts_line && list.list()[1].starts_line);
}

/// CLI-23 AC 4 to 6 (C180): an own-line comment is `Before` the item, member,
/// statement, arm or list element that comes next; a comment on a node's
/// last line is `After` it (after its `,` too); a comment after the last
/// child of a container, or alone in it, is `Inside` it, named by its
/// closing delimiter (the end of the file by the empty span at its length).
#[test]
fn a_comment_is_anchored_before_after_or_inside() {
    let src = "\
// before the item
module M(
    // before the element
    in a: Electrical,
    out b: Electrical, // after the element
) {
    // before the member
    r: f64 = 1.0; // after the member
}

analog for M {
    // alone in the block
}

fn f(x: i32) -> i32 {
    // before the statement
    let y = x; // after the statement
    match y {
        // before the arm
        0 => 1,
        _ => 2,
    }
    // ending the block
}
// ending the file
";
    let module = between(src, "module M(", "\n}");
    let analog_close = between(src, "analog for M", "}");
    let analog_close = Span::new(analog_close.end - 1, analog_close.end);
    let fn_close = Span::new(src.rfind('}').expect("a brace"), src.rfind('}').expect("a brace") + 1);
    let expected = vec![
        Anchor::Before(module),
        Anchor::Before(between(src, "in a", "Electrical")),
        Anchor::After(between(src, "out b", "Electrical")),
        Anchor::Before(between(src, "r: f64", ";")),
        Anchor::After(between(src, "r: f64", ";")),
        Anchor::Inside(analog_close),
        Anchor::Before(between(src, "let y", ";")),
        Anchor::After(between(src, "let y", ";")),
        Anchor::Before(between(src, "0 => 1", ",")),
        Anchor::Inside(fn_close),
        Anchor::Inside(Span::new(src.len(), src.len())),
    ];
    let (got, verbatim) = anchors(src);
    assert_eq!(got, expected);
    assert!(verbatim.is_empty(), "every comment of this source has an anchor: {verbatim:?}");
}

/// CLI-23 AC 7 (C184): a comment inside an expression is at no anchor, so the
/// statement holding it is `Verbatim`; the innermost member or statement is
/// chosen, and every comment inside a verbatim node is that node's, even one
/// that would have an anchor. Every comment has exactly one anchor, and each
/// `Verbatim` one lies in exactly one verbatim node.
#[test]
fn an_unreachable_comment_marks_its_statement_verbatim() {
    let src = "\
fn f(a: f64, b: f64, c: bool) -> f64 {
    let x = a; // after
    let y = a /* mid */ + b;
    if c {
        let z = a /* in */ + 1.0;
    }
    let w = { // after an open brace
        a // after a node inside a verbatim statement
    };
    y
}
";
    let let_y = between(src, "let y", ";");
    let let_z = between(src, "let z", ";");
    let let_w = between(src, "let w", "};");
    let (got, verbatim) = anchors(src);
    assert_eq!(
        got,
        vec![
            Anchor::After(between(src, "let x", ";")),
            Anchor::Verbatim(let_y),
            Anchor::Verbatim(let_z),
            Anchor::Verbatim(let_w),
            Anchor::Verbatim(let_w),
        ]
    );
    assert_eq!(verbatim, vec![let_y, let_z, let_w]);

    // The count: one anchor per comment; a verbatim comment lies in exactly one verbatim node.
    let comments = Comments::of(src);
    assert_eq!(got.len(), comments.list().len());
    for (c, anchor) in comments.list().iter().zip(&got) {
        let holders: Vec<&Span> = verbatim.iter().filter(|v| v.start <= c.span.start && c.span.end <= v.end).collect();
        match anchor {
            Anchor::Verbatim(v) => assert_eq!(holders, vec![v], "{}", c.text),
            _ => assert!(holders.is_empty(), "{} is anchored and in a verbatim node", c.text),
        }
    }
}

/// The formatted text of `src`, which must format.
fn formatted(src: &str) -> String {
    Formatter::format(src).unwrap_or_else(|d| panic!("{d:?}\n{src}"))
}

/// CLI-23 AC 1 (C176): six comments in six positions (before an item, after
/// a list element, after a member, ending a module, inside an expression,
/// ending the file) come out once each, in order, with their text unchanged.
const SIX: &str = "\
// 1 before the item
module M(in a: Electrical, // 2 after an element
) {
  x:f64=1.0;      // 3 after a member
        // 4 ending the module
}
fn f(a: f64) -> f64 {
      let y = a /* 5 inside an expression */ + 1.0;
  y
}


/* 6 ending the file */
";

#[test]
fn every_plain_comment_comes_out_once_in_order_unchanged() {
    // AC 4 to 7 place each one; the rest is the printer's layout (items one blank line apart,
    // a list with a comment one element per line).
    let expected = "\
// 1 before the item
module M(
    in a: Electrical, // 2 after an element
) {
    x: f64 = 1.0; // 3 after a member
    // 4 ending the module
}

fn f(a: f64) -> f64 {
    let y = a /* 5 inside an expression */ + 1.0;
    y
}

/* 6 ending the file */
";
    let out = formatted(SIX);
    assert_eq!(out, expected);
    let texts = vec![
        "// 1 before the item",
        "// 2 after an element",
        "// 3 after a member",
        "// 4 ending the module",
        "/* 5 inside an expression */",
        "/* 6 ending the file */",
    ];
    assert_eq!(Comments::of(SIX).texts(), texts);
    assert_eq!(Comments::of(&out).texts(), texts);
}

/// CLI-23 AC 1 (C177): the guard lexes a formatted text and refuses one with
/// a comment fewer than the source: an internal-error diagnostic at the lost
/// comment, and no text. A text that keeps them passes through unchanged.
#[test]
fn the_guard_refuses_a_text_that_lost_a_comment() {
    let src = "fn f() {\n    let x = 1; // one\n    // two\n    x\n}\n";
    let good = formatted(src);
    assert_eq!(Formatter::guard(src, good.clone()), Ok(good.clone()));

    let lost = good.replace("    // two\n", "");
    assert_eq!(Comments::of(&lost).texts(), vec!["// one"], "the doctored text lost one comment");
    let refused = Formatter::guard(src, lost).expect_err("a text that lost a comment is refused");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].code, "P0004");
    assert!(refused[0].message.starts_with("internal error"), "{}", refused[0].message);
    assert_eq!(refused[0].span(), between(src, "// two", "two"));

    // AC 2's half of the guard: a text whose tree differs is refused too.
    let changed = good.replace("let x = 1;", "let x = 2;");
    let refused = Formatter::guard(src, changed).expect_err("a text with another tree is refused");
    assert_eq!((refused.len(), refused[0].code.as_ref()), (1, "P0004"));
}

/// Every source this target formats: the cases of the tests above and below.
const CASES: &[&str] = &[
    SIX,
    OWN_LINE,
    TRAILING,
    ENDING,
    INNER,
    BLANKS,
    "fn f() {\n    let x = 1; // one\n    // two\n    x\n}\n",
    // A comment before docs and attributes stands before the whole node.
    "// before the docs\n/// the doc\n#[inline]\nfn f() {}\n",
    // A CRLF source: the comment ends before the `\r`.
    "fn f() {\r\n    let x = 1; // crlf\r\n    x\r\n}\r\n",
    // A comment alone in a file, and one after the inner docs.
    "// only a comment\n",
    "//! inner doc\n\n// after the docs\nfn f() {}\n",
    // A multi-line block comment keeps its lines as written, and a verbatim statement its string.
    "fn f() {\n        /* one\n   two */\n    let s = \"a\n  b\" /* in */;\n}\n",
];

/// CLI-23 AC 2 (C178): for each case, the formatted text parses to the
/// source's tree, spans aside.
#[test]
fn the_formatted_tree_equals_the_sources() {
    for src in CASES {
        let out = formatted(src);
        let (before, after) = (parse(src), parse(&out));
        assert!(after.diagnostics.is_empty(), "{:?}\n{out}", after.diagnostics);
        assert!(before.file == after.file, "the formatted text parses to another tree:\n{src}\n---\n{out}");
    }
}

/// CLI-23 AC 3 (C179): for each case, formatting the formatted text gives it back.
#[test]
fn formatting_twice_changes_nothing() {
    for src in CASES {
        let once = formatted(src);
        assert_eq!(formatted(&once), once, "formatting is not idempotent on:\n{src}");
        assert_eq!(Formatter::check(&once), Ok(true));
    }
    assert_eq!(Formatter::check(SIX), Ok(false), "an unformatted text is not formatted");
}

/// CLI-23 AC 4 (C181): an own-line comment, badly indented, before a member,
/// a statement and a match arm.
const OWN_LINE: &str = "\
module M() {
x: f64 = 1.0;
        // before the member
  y: f64 = 2.0;
}

fn f(a: i32) -> i32 {
// before the statement
      let b = a;
    match b {
  // before the arm
        0 => 1,
        _ => 2,
    }
}
";

#[test]
fn a_comment_on_its_own_line_stays_before_its_node() {
    // AC 4: on its own line, before the same node, at that node's indentation.
    let expected = "\
module M() {
    x: f64 = 1.0;
    // before the member
    y: f64 = 2.0;
}

fn f(a: i32) -> i32 {
    // before the statement
    let b = a;
    match b {
        // before the arm
        0 => 1,
        _ => 2,
    }
}
";
    assert_eq!(formatted(OWN_LINE), expected);
}

/// CLI-23 AC 5 (C182): a comment on a node's last line.
const TRAILING: &str = "fn f() {\n    let x = 1;   // why\n    x\n}\nfn g() -> f64 { 1.0 }    // one line in the source\n";

#[test]
fn a_trailing_comment_stays_on_its_line() {
    // AC 5: after the node, on the node's last line, one space before it.
    let expected = "fn f() {\n    let x = 1; // why\n    x\n}\n\nfn g() -> f64 {\n    1.0\n} // one line in the source\n";
    assert_eq!(formatted(TRAILING), expected);
}

/// CLI-23 AC 6 (C183): a comment as the last line of a block, and as the only
/// content of one (an item's body and a nested block).
const ENDING: &str = "\
fn f() {
    let x = 1;
        // the last line
}

analog for M {
// the only line
}

fn g(c: bool) {
    if c {
            // only in the if
    }
}
";

#[test]
fn a_comment_ending_a_block_stays_inside_it() {
    // AC 6: before the block's closing brace, at the block's inner indentation.
    let expected = "\
fn f() {
    let x = 1;
    // the last line
}

analog for M {
    // the only line
}

fn g(c: bool) {
    if c {
        // only in the if
    }
}
";
    assert_eq!(formatted(ENDING), expected);
}

/// CLI-23 AC 7 (C185): a statement holding a comment no anchor reaches.
const INNER: &str = "\
fn f(a: f64, b: f64) -> f64 {
        let x   =   a;
        let y = a /* mid */
            +   b;
        let z  = x+y;
    z
}
";

#[test]
fn a_statement_with_an_inner_comment_is_kept_as_written() {
    // AC 7: `let y` as the source writes it, its lines moved left by the indentation its first
    // line had (8) and indented as a statement of the body (4); its neighbours are formatted.
    let expected = "\
fn f(a: f64, b: f64) -> f64 {
    let x = a;
    let y = a /* mid */
        +   b;
    let z = x + y;
    z
}
";
    assert_eq!(formatted(INNER), expected);
}

/// CLI-23 AC 8 (C186): blank lines between statements and between members.
const BLANKS: &str = "\
fn f() {
    let a = 1;



    let b = 2;
    let c = 3;
}

module M() {
    a: f64 = 1.0;


    // b's comment

    b: f64 = 2.0;
    c: f64 = 3.0;
}
";

#[test]
fn blank_lines_collapse_to_one() {
    // AC 8: one blank line where the source has one or more, none where it has none; a comment
    // keeps the blank lines on either side of it the same way.
    let expected = "\
fn f() {
    let a = 1;

    let b = 2;
    let c = 3;
}

module M() {
    a: f64 = 1.0;

    // b's comment

    b: f64 = 2.0;
    c: f64 = 3.0;
}
";
    assert_eq!(formatted(BLANKS), expected);
}

/// CLI-23 AC 9 (C187): a source that does not parse gives the parser's
/// diagnostics and no text.
#[test]
fn a_source_that_does_not_parse_gives_diagnostics_and_no_text() {
    let src = "module M( {";
    let parsed = parse(src);
    assert!(!parsed.diagnostics.is_empty(), "the source does not parse");
    assert_eq!(Formatter::format(src), Err(parsed.diagnostics.clone()));
    assert_eq!(Formatter::check(src), Err(parsed.diagnostics));
}

// ---- the corpus (CLI-23 AC 10) ----------------------------------------------------------------

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A fragment wrapped into the smallest context that parses it with no
/// diagnostics, as `msl_print` wraps one: a file as it is, a module body, an
/// `analog` body, a reaction body or a function body. `None` when no context
/// accepts it.
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

/// Every `.msl` file under `dir`, recursively, in path order.
fn msl_files(dir: &Path) -> Vec<PathBuf> {
    msl_files_with(dir, "msl")
}

/// The files of `dir` with the extension `extension`, recursively, in path order.
fn msl_files_with(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut entries: Vec<PathBuf> =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).map(|e| e.expect("an entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            out.extend(msl_files_with(&path, extension));
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
    out
}

/// The `msl` blocks of the Book, decoded, read with the string functions of `tests/book.rs`.
fn book_blocks() -> Vec<String> {
    let html = std::fs::read_to_string(repo().join("docs/book/content.html")).expect("docs/book/content.html exists");
    let decode = |t: &str| t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    let mut out = Vec::new();
    let mut rest = html.as_str();
    while let Some(at) = rest.find("<pre data-lang=\"") {
        let after = &rest[at + "<pre data-lang=\"".len()..];
        let (lang, body) = after.split_once("\">").expect("a block's opening tag closes");
        let (text, tail) = body.split_once("</pre>").expect("a block closes");
        if lang == "msl" {
            out.push(decode(text));
        }
        rest = tail;
    }
    out
}

/// Every source of the corpus with its label: the standard library, the
/// gallery fixtures, `examples/buck`, `examples/rc` and the MSL twins of
/// `tests/e2e/twins/` (each must parse), the MSL files of the test projects and
/// fixtures of the other crates that parse (some are broken on purpose), the
/// specification's blocks and the Book's `msl` blocks wrapped as fragments.
fn sources() -> Vec<(String, String)> {
    let repo = repo();
    let mut out = Vec::new();
    for dir in ["crates/piperine-lang/std", "crates/piperine-lang/tests/fixtures/gallery", "examples/buck", "examples/rc", "tests/e2e/twins"] {
        let files = msl_files(&repo.join(dir));
        assert!(!files.is_empty(), "{dir} holds MSL files");
        for path in files {
            let text = std::fs::read_to_string(&path).expect("an MSL file is UTF-8");
            let label = path.strip_prefix(&repo).unwrap_or(&path).display().to_string();
            assert!(parse(&text).diagnostics.is_empty(), "{label} parses");
            out.push((label, text));
        }
    }
    for dir in ["crates/piperine-cli/tests/projects", "crates/piperine-parity/tests"] {
        for path in msl_files(&repo.join(dir)) {
            let text = std::fs::read_to_string(&path).expect("an MSL file is UTF-8");
            if parse(&text).diagnostics.is_empty() {
                out.push((path.strip_prefix(&repo).unwrap_or(&path).display().to_string(), text));
            }
        }
    }
    for b in corpus() {
        if b.class != Class::CompileFail {
            out.extend(fragment_source(&b.text).map(|s| (format!("docs/spec/{}:{}", b.file, b.line), s)));
        }
    }
    for (n, text) in book_blocks().into_iter().enumerate() {
        out.extend(fragment_source(&text).map(|s| (format!("docs/book block {n}"), s)));
    }
    out
}

/// CLI-23 AC 10 and AC 1 (C188): over the whole corpus, the comment list of
/// the formatted text equals the source's, file by file.
#[test]
fn the_corpus_keeps_every_comment() {
    let sources = sources();
    let mut comments = 0;
    let mut verbatim = 0;
    for (label, src) in &sources {
        let out = Formatter::format(src).unwrap_or_else(|d| panic!("{label}: {d:?}"));
        let expected = Comments::of(src);
        assert_eq!(Comments::of(&out).texts(), expected.texts(), "{label}: the formatted text changed its comments");
        comments += expected.list().len();
        verbatim += expected.attach(&parse(src).file, src).verbatim().len();
    }
    println!("{} sources, {comments} plain comments, {verbatim} nodes kept verbatim", sources.len());
    assert!(sources.len() >= 60, "the corpus holds at least 60 sources: {}", sources.len());
    assert!(comments >= 100, "the corpus holds at least 100 plain comments: {comments}");
}

/// CLI-23 AC 10 and AC 2 (C189): over the whole corpus, the formatted text
/// parses to the source's tree, spans aside.
#[test]
fn the_corpus_keeps_every_tree() {
    for (label, src) in sources() {
        let out = Formatter::format(&src).unwrap_or_else(|d| panic!("{label}: {d:?}"));
        let (before, after) = (parse(&src), parse(&out));
        assert!(after.diagnostics.is_empty(), "{label}: {:?}", after.diagnostics);
        assert!(before.file == after.file, "{label}: the formatted text parses to another tree:\n{out}");
    }
}

/// CLI-23 AC 10 and AC 3 (C190): over the whole corpus, a second format gives
/// the first's text.
#[test]
fn the_corpus_is_stable_under_a_second_format() {
    for (label, src) in sources() {
        let once = Formatter::format(&src).unwrap_or_else(|d| panic!("{label}: {d:?}"));
        let twice = Formatter::format(&once).unwrap_or_else(|d| panic!("{label} (formatted): {d:?}"));
        assert_eq!(twice, once, "{label}: a second format changes the text");
    }
}

// ---- the shipped sources in the formatter's form (PI-11) ---------------------------------------

/// The text of the printed tree of `src`: what `piperine_lang::msl::print` writes for its parse.
fn printed_tree(src: &str) -> String {
    piperine_lang::msl::print::print(&parse(src).file)
}

/// PI-11 AC 1: every file of `crates/piperine-lang/std/` is the formatter's output for itself.
#[test]
fn every_stdlib_file_is_in_the_formatters_form() {
    let files = msl_files(&repo().join("crates/piperine-lang/std"));
    assert!(!files.is_empty(), "the standard library holds MSL files");
    for path in files {
        let text = std::fs::read_to_string(&path).expect("an MSL file is UTF-8");
        let formatted = Formatter::format(&text).unwrap_or_else(|d| panic!("{}: {d:?}", path.display()));
        assert_eq!(formatted, text, "{} is not in the formatter's form (`piperine fmt` rewrites it)", path.display());
    }
}

/// PI-11 AC 4: formatting a shipped source keeps its printed tree, so no test that reads a name
/// or a position of the tree sees a change. The input is the text the file had before the
/// formatter pass (`tests/fixtures/preformat/<repo path>.pre`, each file as `origin/main` held
/// it), so the formatter has something to change; the output is the file as shipped.
#[test]
fn formatting_keeps_the_printed_tree_of_every_shipped_source() {
    let before = repo().join("crates/piperine-lang/tests/fixtures/preformat");
    let mut changed = 0;
    for fixture in msl_files_with(&before, "pre") {
        let relative = fixture.strip_prefix(&before).expect("under the fixtures").with_extension("");
        let shipped = repo().join(&relative);
        let text = std::fs::read_to_string(&fixture).expect("a fixture is UTF-8");
        let formatted = Formatter::format(&text).unwrap_or_else(|d| panic!("{}: {d:?}", fixture.display()));
        assert_eq!(printed_tree(&formatted), printed_tree(&text), "{}: formatting changed the printed tree", relative.display());
        assert_eq!(formatted, std::fs::read_to_string(&shipped).expect("a shipped file is UTF-8"), "{}: formatting the pre-format text does not give the shipped file", relative.display());
        changed += usize::from(formatted != text);
    }
    assert!(changed > 0, "the pre-format texts are the unformatted ones: the formatter changed none");
}
