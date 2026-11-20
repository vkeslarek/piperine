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

use piperine_lang::pom::{BUILTINS, BuiltinKind, Design, Reflect};
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
    for id in ["welcome", "language", "values", "natures", "modules", "analog", "digital", "mixed", "packages", "errors", "pom", "cheatsheet"] {
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

// ---- the chapter "Core and standard library" (STD-18 AC 3 to 8, 12) ---------------------------------

/// The text of the chapter `id`, from its `<section>` tag to the first `</section>`.
fn chapter(html: &str, id: &str) -> String {
    let open = format!("<section id=\"{id}\" class=\"chapter\">");
    let at = html.find(&open).unwrap_or_else(|| panic!("the Book has a chapter `{id}`"));
    let rest = &html[at..];
    rest[..rest.find("</section>").expect("a chapter closes")].to_string()
}

/// The decoded text of every `<code>..</code>` span of `html`.
fn code_spans(html: &str) -> Vec<String> {
    html.split("<code>").skip(1).filter_map(|s| s.split_once("</code>")).map(|(span, _)| decode(span)).collect()
}

/// The decoded text of every `<h3>..</h3>` heading of `html`, with its inline tags kept.
fn headings(html: &str) -> Vec<String> {
    html.split("<h3>").skip(1).filter_map(|s| s.split_once("</h3>")).map(|(h, _)| decode(h)).collect()
}

/// Does a code span name `name`? A name of one or two characters (`m`, `E`, `PI`) must be the whole
/// span or the head of a call; a longer name may be any identifier of the span.
fn span_names(span: &str, name: &str) -> bool {
    let bare = name.trim_end_matches('!');
    if span == name || span.starts_with(&format!("{name}(")) {
        return true;
    }
    if name.len() <= 2 {
        return false;
    }
    let token = |c: char| c.is_alphanumeric() || c == '_';
    span.split(|c: char| !token(c) && c != '!').any(|t| t == name || t == bare)
}

/// The names of `names` that no code span of `html` names.
fn missing(html: &str, names: &[String]) -> Vec<String> {
    let spans = code_spans(html);
    names.iter().filter(|n| !spans.iter().any(|s| span_names(s, n))).cloned().collect()
}

fn embedded() -> Design {
    Design::builder().source("top.msl", "").build()
}

/// The core names of §14, read from the built-in table, and the four the table does not carry as rows
/// of those kinds.
fn core_names() -> Vec<String> {
    let kinds = [BuiltinKind::Fn, BuiltinKind::Module, BuiltinKind::Event, BuiltinKind::Statement, BuiltinKind::Method, BuiltinKind::Node, BuiltinKind::ImplicitParam];
    let mut names: Vec<String> = BUILTINS.iter().filter(|r| kinds.contains(&r.kind)).map(|r| r.name.to_string()).collect();
    names.extend(["Nature", "str", "emit!", "include_str!"].map(String::from));
    names
}

/// The `pub` namespaces of `std`, by name.
fn std_namespaces(design: &Design) -> Vec<String> {
    design.std_crate().root().submodules().into_iter().filter(|n| n.is_pub()).filter_map(|n| n.name().map(String::from)).collect()
}

/// The names `std::prelude` exports: the `pub` named items of each namespace it glob-imports.
fn prelude_exports(design: &Design) -> Vec<String> {
    let prelude = design.std_crate().root().submodules().into_iter().find(|n| n.name() == Some("prelude")).expect("std declares the prelude");
    let mut names = Vec::new();
    for item in prelude.items() {
        for import in item.as_use().expect("the prelude holds only `use`").imports() {
            let ns = import.path.text().strip_prefix("crate::").expect("a crate path").to_string();
            let namespace = design.std_crate().root().submodules().into_iter().find(|n| n.name() == Some(ns.as_str())).expect("a namespace");
            names.extend(namespace.items().into_iter().filter(|i| i.is_pub()).filter_map(|i| i.name().map(String::from)));
        }
    }
    names
}

#[test]
fn the_core_std_chapter_follows_packages() {
    let html = content();
    let ids = chapter_ids(&html);
    let at = ids.iter().position(|c| c == "core-std").expect("a chapter `core-std`");
    assert_eq!(ids.get(at.wrapping_sub(1)).map(String::as_str), Some("packages"), "{ids:?}");
    assert!(chapter(&html, "core-std").contains("<h2>Core and standard library</h2>"), "the title");
    let numbers: Vec<usize> = html
        .split("<p class=\"ch-num\">Chapter ")
        .skip(1)
        .map(|s| s.split_once('<').expect("the label closes").0.parse().expect("a chapter number"))
        .collect();
    assert_eq!(numbers, (1..=ids.len()).collect::<Vec<_>>(), "chapter numbers are consecutive from 1");
}

#[test]
fn the_chapter_names_every_core_name() {
    let absent = missing(&chapter(&content(), "core-std"), &core_names());
    assert!(absent.is_empty(), "the chapter does not name these core names: {absent:?}");
}

#[test]
fn the_chapter_has_a_part_per_namespace() {
    let design = embedded();
    let namespaces = std_namespaces(&design);
    assert_eq!(namespaces.len(), 9, "{namespaces:?}");
    let heads = headings(&chapter(&content(), "core-std"));
    let absent: Vec<String> = namespaces.iter().filter(|n| !heads.iter().any(|h| h.contains(&format!("<code>std::{n}</code>")))).cloned().collect();
    assert!(absent.is_empty(), "no heading with `std::<name>` for: {absent:?}; headings: {heads:?}");
}

#[test]
fn the_chapter_names_every_prelude_export() {
    let exports = prelude_exports(&embedded());
    assert!(exports.len() >= 80, "the prelude exports {} names", exports.len());
    let absent = missing(&chapter(&content(), "core-std"), &exports);
    assert!(absent.is_empty(), "the chapter does not name these prelude exports: {absent:?}");
}

#[test]
fn the_chapter_explains_the_prelude_and_the_engine_items() {
    let html = chapter(&content(), "core-std");
    assert!(headings(&html).iter().any(|h| h.contains("std::prelude")), "the chapter has a part on `std::prelude`");
    let design = embedded();
    let intrinsics: Vec<String> = design.std_crate().decls().into_iter().filter(|d| d.as_function().is_some_and(|f| f.is_intrinsic())).filter_map(|d| d.name().map(String::from)).collect();
    assert_eq!(intrinsics.len(), 5, "{intrinsics:?}");
    let part = html
        .split("<h3>")
        .skip(1)
        .find(|p| p.split_once("</h3>").is_some_and(|(h, _)| h.to_lowercase().contains("engine")))
        .expect("a heading that contains `engine`");
    let absent = missing(part.split("<h3>").next().unwrap(), &intrinsics);
    assert!(absent.is_empty(), "the part on the engine does not name: {absent:?}");
    // C107: the part on the engine also names `std::prelude` (how the five items reach a model).
    assert!(code_spans(part.split("<h3>").next().unwrap()).iter().any(|s| s == "std::prelude"), "the part on the engine names `std::prelude`");
}

#[test]
fn chapter_blocks_build_against_std() {
    let html = chapter(&content(), "core-std");
    let msl: Vec<String> = blocks(&html).into_iter().filter(|(lang, _)| lang == "msl").map(|(_, text)| text).collect();
    assert!(!msl.is_empty(), "the chapter has msl blocks");
    for text in msl {
        let design = Design::builder().source("top.msl", &text).build();
        let found: Vec<String> = design.diagnostics().iter().map(|d| format!("{}: {}", d.diagnostic.code, d.diagnostic.message)).collect();
        assert!(found.is_empty(), "a chapter block does not build against std: {found:?}\n{text}");
    }
}

#[test]
fn a_missing_name_is_listed() {
    let names: Vec<String> = ["ddt", "delay", "limit", "m", "emit!"].map(String::from).to_vec();
    let html = "<p><code>ddt(x)</code> and <code>m: f64</code> and <code>emit!(\"a\")</code></p>";
    assert_eq!(missing(html, &names), ["delay", "limit", "m"], "every missing name is listed, in order");
    assert!(missing("<code>delay</code>", &["delay".to_string()]).is_empty());
}

// ---- the packages note, the cheat sheet and the analysis kind (STD-18 AC 10, 13) -----------------------

/// The text of `html` with tags removed and whitespace collapsed.
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn packages_and_cheatsheet_name_std() {
    let html = content();
    let packages = chapter(&html, "packages");
    assert!(code_spans(&packages).iter().any(|s| s == "std::prelude"), "the chapter `packages` names `std::prelude` in a code span");
    let text = plain(&packages);
    assert!(text.contains("Every crate depends on it"), "packages says `std` is a dependency of every crate: {text}");
    assert!(text.contains("implied in every file"), "packages says the prelude is implied in every file");
    let cheat = code_spans(&chapter(&html, "cheatsheet"));
    let absent: Vec<String> = std_namespaces(&embedded()).into_iter().filter(|n| !cheat.iter().any(|s| *s == format!("std::{n}"))).collect();
    assert!(absent.is_empty(), "the cheat sheet does not name these namespaces as `std::<name>`: {absent:?}");
}

#[test]
fn the_book_says_transient() {
    let html = content();
    for (n, (lang, text)) in blocks(&html).into_iter().enumerate() {
        assert!(!text.contains("analysis(Tran)"), "block {n} ({lang}) writes `analysis(Tran)`: the analysis kind is `Transient`");
    }
    assert!(!html.contains("analysis(Tran)"), "no span or paragraph writes `analysis(Tran)` either");
    // The list of analysis kinds in chapter 6 names the kind, not the pass-gate module.
    assert!(!html.contains("<code>Dc</code>, <code>Tran</code>"), "the analysis kinds are Dc, Transient, Ac and Noise");
    assert!(html.contains("<code>Dc</code>, <code>Transient</code>"), "the analysis kinds name `Transient`");
}

// ---- the chapter "The command line" (feature `msl-cli`, CLI-19 AC 1 and 13) -------------------------

#[test]
fn the_cli_chapter_closes_the_fundamentals_before_the_cheat_sheet() {
    // CLI-19 AC 1: the chapter `cli`, titled `The command line`, belongs to Part 1 of the Book, after
    // `errors` and before `cheatsheet`, which closes the part; Part 2 follows with `pom`, `abi`, `solver`,
    // `compiler` and `structure` (the Book's order: reading errors 12, the command line 13, the cheat
    // sheet 14, then the object model 15 to how Piperine is organized 19); the chapter numbers are
    // consecutive from 1
    let html = content();
    let ids = chapter_ids(&html);
    let tail: Vec<&str> = ids.iter().rev().take(8).rev().map(String::as_str).collect();
    assert_eq!(tail, ["errors", "cli", "cheatsheet", "pom", "abi", "solver", "compiler", "structure"], "{ids:?}");
    assert!(chapter(&html, "cli").contains("<h2>The command line</h2>"), "the title");
    let numbers: Vec<usize> = html
        .split("<p class=\"ch-num\">Chapter ")
        .skip(1)
        .map(|s| s.split_once('<').expect("the label closes").0.parse().expect("a chapter number"))
        .collect();
    assert_eq!(numbers, (1..=ids.len()).collect::<Vec<_>>(), "chapter numbers are consecutive from 1");
    assert_eq!(ids.len(), 19, "{ids:?}");
}

#[test]
fn the_book_has_two_parts_and_the_second_starts_at_the_object_model() {
    // The Book is two parts, each opened by a `<div class="part">` that names it: Part 1, Fundamentals,
    // before the first chapter, and Part 2, In depth, right before `pom`; `simulation` follows `mixed`
    // in Part 1, and the chapter of the language rules is `language`, the second
    let html = content();
    let parts: Vec<(&str, &str)> = html
        .split("<div class=\"part\" id=\"")
        .skip(1)
        .map(|s| {
            let number = s.split_once("data-part=\"").and_then(|(_, rest)| rest.split_once('"')).expect("a part has a number").0;
            let title = s.split_once("data-title=\"").and_then(|(_, rest)| rest.split_once('"')).expect("a part has a title").0;
            (number, title)
        })
        .collect();
    assert_eq!(parts, [("1", "Fundamentals"), ("2", "In depth")]);
    let first = html.find("<div class=\"part\" id=\"part-fundamentals\"").expect("Part 1");
    let second = html.find("<div class=\"part\" id=\"part-in-depth\"").expect("Part 2");
    let at = |id: &str| html.find(&format!("<section id=\"{id}\" class=\"chapter\">")).unwrap_or_else(|| panic!("a chapter `{id}`"));
    assert!(first < at("welcome"), "Part 1 opens the Book");
    assert!(at("cheatsheet") < second && second < at("pom"), "Part 2 opens right before the object model");
    let ids = chapter_ids(&html);
    assert_eq!(&ids[..2], ["welcome", "language"], "{ids:?}");
    let mixed = ids.iter().position(|c| c == "mixed").expect("a chapter `mixed`");
    assert_eq!(ids.get(mixed + 1).map(String::as_str), Some("simulation"), "{ids:?}");
    assert!(ids.iter().position(|c| c == "pom").expect("a chapter `pom`") == 14, "the object model is chapter 15: {ids:?}");
}

#[test]
fn the_book_teaches_the_language_on_its_own_terms() {
    // No sentence of the Book sends the reader to another language to learn this one: the prose (code
    // blocks apart, where `backend rust` is what the binary prints) never names Rust except as what it is
    // here, the compiled backend, the generated source, `rustc` and the host library
    let html = content();
    let mut prose = String::new();
    let mut rest = html.as_str();
    while let Some(open) = rest.find("<pre") {
        prose.push_str(&rest[..open]);
        rest = rest[open..].split_once("</pre>").map_or("", |(_, after)| after);
    }
    prose.push_str(rest);
    let text = plain(&prose);
    for deferral in ["like Rust", "as in Rust", "is Rust", "are Rust", "know Rust", "Rust's", "Rust has", "Rust and Cargo", "follow Rust", "to Rust's", "Rust habits", "Rust programmer", "Rust eyes", "Rust local", "Rust variable", "Rust method", "Rust-like", "Rust 2018"] {
        assert!(!text.contains(deferral), "the Book defers to Rust: `{deferral}`");
    }
    let allowed = ["Rust backend", "generated Rust", "compiled Rust", "as Rust", "Rust source", "Rust compiler", "Rust crates", "Rust program", "Rust library", "Rust side", "in Rust", "In Rust", "to Rust", "from Rust", "into Rust", "Generated Rust"];
    let words: Vec<&str> = text.split(' ').collect();
    for (at, _) in words.iter().enumerate().filter(|(_, word)| word.contains("Rust")) {
        let around = words[at.saturating_sub(1)..(at + 2).min(words.len())].join(" ");
        assert!(allowed.iter().any(|phrase| around.contains(phrase)), "an unexpected mention of Rust: `{around}`");
    }
}

#[test]
fn the_welcome_chapter_says_a_bench_is_msl() {
    // CLI-19 AC 13: chapter 1 says a bench is written in MSL and that sweeps, Monte Carlo and
    // optimization live in the host, and no sentence of it says testbenches or stimulus do not
    // live in MSL
    let text = plain(&chapter(&content(), "welcome"));
    let sentences: Vec<&str> = text.split(". ").collect();
    assert!(sentences.iter().any(|s| s.contains("bench") && s.contains("written in MSL")), "a sentence says a bench is written in MSL");
    assert!(
        sentences.iter().any(|s| s.contains("Sweeps") && s.contains("Monte Carlo") && s.contains("optimization") && s.contains("host")),
        "a sentence says sweeps, Monte Carlo and optimization live in the host"
    );
    let negations = [" not ", " no ", "outside", "never"];
    let denies: Vec<&&str> = sentences
        .iter()
        .filter(|s| {
            let lower = s.to_lowercase();
            (lower.contains("testbench") || lower.contains("stimulus")) && lower.contains("msl") && negations.iter().any(|n| lower.contains(n))
        })
        .collect();
    assert!(denies.is_empty(), "a sentence says testbenches or stimulus do not live in MSL: {denies:?}");
}

// ---- the chapter "From source to solver" and the cheat sheet know the command line (CLI-19 AC 9, 10) ----

#[test]
fn the_compiler_chapter_says_the_command_line_exists() {
    // CLI-19 AC 9: the chapter `compiler` names `piperine run`, and its part on what comes next
    // does not list the command line as work to come
    let html = chapter(&content(), "compiler");
    assert!(code_spans(&html).iter().any(|s| s.starts_with("piperine run")), "the chapter names `piperine run`");
    assert!(code_spans(&html).iter().any(|s| s == "--backend"), "the chapter names `--backend` where a host picks a backend");
    let next = html.split("<h3>What comes next</h3>").nth(1).expect("a part `What comes next`");
    let text = plain(next.split("<h3>").next().unwrap_or(next));
    assert!(!text.to_lowercase().contains("command line"), "what comes next still mentions the command line: {text}");
    assert!(text.contains("Chapter 13"), "what comes next points at the chapter that runs a design: {text}");
}

#[test]
fn the_cheat_sheet_lists_the_seven_commands() {
    // CLI-19 AC 10: the cheat sheet names `piperine check`, `build`, `run`, `test`, `new`,
    // `clean` and `fmt`
    let html = chapter(&content(), "cheatsheet");
    let text = plain(&html);
    for command in ["check", "build", "run", "test", "new", "clean", "fmt"] {
        assert!(text.contains(&format!("piperine {command}")), "the cheat sheet names `piperine {command}`");
    }
}
