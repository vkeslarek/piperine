//! Scope: the MSL Book's source in `docs/book/` (STD-18): that the three source files are in the
//! repository, that `build.py` writes an ignored `index.html`, and that every `msl` block of
//! `content.html` is MSL the parser accepts. The Book is read with plain string functions over the
//! HTML (sections by `<section id="..">`, blocks by `<pre data-lang="..">`, code spans by
//! `<code>`); nothing here parses HTML.

#[path = "support/spec_corpus.rs"]
#[allow(dead_code)] // item scope: this target needs `fragment_diagnostics` and nothing else of the module
mod spec_corpus;

use std::path::PathBuf;
use std::process::Command;

use spec_corpus::fragment_diagnostics;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn book_dir() -> PathBuf {
    repo().join("docs/book")
}

fn content() -> String {
    std::fs::read_to_string(book_dir().join("content.html")).expect("docs/book/content.html exists")
}

/// The five entities the Book's source uses, decoded.
fn decode(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// Every `<pre data-lang="..">..</pre>` block as (language, decoded text), in order.
fn blocks(html: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<pre data-lang=\"") {
        let after = &rest[at + "<pre data-lang=\"".len()..];
        let (lang, body) = after.split_once("\">").expect("a block's opening tag closes");
        let (text, tail) = body.split_once("</pre>").expect("a block closes");
        out.push((lang.to_string(), decode(text)));
        rest = tail;
    }
    out
}

/// The ids of the Book's chapters, in order.
fn chapter_ids(html: &str) -> Vec<String> {
    html.split("<section id=\"").skip(1).filter_map(|s| s.split_once('"')).filter(|(_, rest)| rest.starts_with(" class=\"chapter\"")).map(|(id, _)| id.to_string()).collect()
}

#[test]
fn the_book_source_is_in_docs_book() {
    for file in ["content.html", "template.html", "build.py"] {
        assert!(book_dir().join(file).is_file(), "docs/book/{file} exists");
    }
    let ids = chapter_ids(&content());
    for id in ["welcome", "from-rust", "values", "natures", "modules", "analog", "digital", "mixed", "packages", "errors", "pom", "cheatsheet"] {
        assert!(ids.iter().any(|c| c == id), "the Book keeps its chapter `{id}`: {ids:?}");
    }
}

#[test]
fn build_py_writes_an_ignored_index() {
    let index = book_dir().join("index.html");
    let _ = std::fs::remove_file(&index);
    let status = Command::new("python3").arg("docs/book/build.py").current_dir(repo()).status().expect("python3 is installed: the Book check never skips");
    assert!(status.success(), "python3 docs/book/build.py exits 0");
    let html = std::fs::read_to_string(&index).expect("build.py wrote docs/book/index.html");
    assert!(html.contains("<section id=\"welcome\""), "the output holds the chapters");
    let ignored = Command::new("git").args(["check-ignore", "-q", "docs/book/index.html"]).current_dir(repo()).status().expect("git is installed");
    assert!(ignored.success(), "git ignores docs/book/index.html");
}

#[test]
fn every_msl_block_parses() {
    let mut failures = Vec::new();
    for (n, (lang, text)) in blocks(&content()).into_iter().enumerate() {
        match lang.as_str() {
            "msl" => {
                let diags = fragment_diagnostics(&text);
                if !diags.is_empty() {
                    failures.push(format!("block {n}: {diags:?}\n{text}"));
                }
            }
            "msl-error" => {
                if fragment_diagnostics(&text).is_empty() {
                    failures.push(format!("block {n} (msl-error) has no diagnostic:\n{text}"));
                }
            }
            _ => {}
        }
    }
    assert!(failures.is_empty(), "{} Book blocks do not parse:\n{}", failures.len(), failures.join("\n---\n"));
}

#[test]
fn the_book_has_at_least_40_msl_blocks() {
    let count = blocks(&content()).iter().filter(|(lang, _)| lang == "msl").count();
    assert!(count >= 40, "the Book has {count} msl blocks, at least 40 are required");
}
