//! Scope: the two guards that tie Part VII of the specification to the source of the standard
//! library (STD-11, STD-12). Forward: every `rust` fence of Part VII is consecutive lines of the
//! `std` file its section maps to. Reverse: every top-level item and behavior block of a `std`
//! file is verbatim inside a `rust` fence of `docs/spec/`. Each guard is a plain function that
//! returns its failures; the tests assert the list is empty on the real tree and assert its content
//! on text written in the test.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;
#[path = "support/spec_tables.rs"]
mod spec_tables;

use std::path::PathBuf;

use piperine_lang::msl::ast::{Item, ItemKind};
use piperine_lang::msl::parser::parse;
use spec_corpus::{Block, Class, corpus, extract};
use spec_tables::{section, table_rows};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn std_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("std")
}

/// A fence of Part VII that is not text of its file.
#[derive(Debug, PartialEq)]
struct FenceFailure {
    /// The markdown file the fence is in.
    file: String,
    /// 1-based line of the opening fence.
    spec_line: usize,
    /// The first line of the fence with no match.
    first_unmatched: String,
}

/// A `std` item that is in no fence.
#[derive(Debug, PartialEq)]
struct ItemFailure {
    file: String,
    line: usize,
    item: String,
}

/// One top-level item of a `std` file, as its own source text.
struct Unit {
    line: usize,
    label: String,
    behavior: bool,
    text: String,
}

/// Lines with trailing whitespace removed.
fn normalized_lines(text: &str) -> Vec<String> {
    text.lines().map(|l| l.trim_end().to_string()).collect()
}

/// The lines of a fence without its leading and trailing blank lines.
fn trimmed_fence(text: &str) -> Vec<String> {
    let mut lines = normalized_lines(text);
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let lead = lines.iter().take_while(|l| l.is_empty()).count();
    lines.split_off(lead)
}

/// The number `k` of a heading `### 22.k ...` or `#### 22.k.m ...` ("22.k" as text), or `None`.
fn section_of_heading(line: &str) -> Option<String> {
    let rest = line.trim_start_matches('#');
    if rest.len() == line.len() || !rest.starts_with(' ') {
        return None;
    }
    let word = rest.trim().split_whitespace().next()?;
    let mut parts = word.split('.');
    if parts.next()? != "22" {
        return None;
    }
    let k = parts.next()?;
    (!k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())).then(|| format!("22.{k}"))
}

/// The "Sources" table of §22: (section, file name) for each row that names a `§22.k` section.
fn sources(part_vii: &str) -> Vec<(String, String)> {
    let s22 = section(part_vii, "## 22.");
    let at = s22.find("**Sources.**").expect("§22 has a Sources table");
    table_rows(&s22[at..])
        .into_iter()
        .filter_map(|row| {
            let section = row[0].strip_prefix('§')?.to_string();
            section.starts_with("22.").then(|| (section, row[2].trim_matches('`').to_string()))
        })
        .collect()
}

/// Forward guard over one markdown text: each `rust` fence under a heading `22.k` whose file is in
/// `files` (keyed by `"22.k"`) must be consecutive lines of that file.
fn fence_failures(markdown: &str, files: &[(String, String)]) -> Vec<FenceFailure> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut failures = Vec::new();
    for block in extract("markdown", markdown).into_iter().filter(|b| b.class == Class::Valid) {
        let Some(section) = lines[..block.line - 1].iter().rev().find_map(|l| section_of_heading(l)) else {
            continue;
        };
        let Some((_, file)) = files.iter().find(|(s, _)| *s == section) else {
            continue;
        };
        let fence = trimmed_fence(&block.text);
        let file_lines = normalized_lines(file);
        // The longest prefix of the fence that occurs as consecutive lines of the file.
        let matched = (0..=fence.len())
            .rev()
            .find(|&k| k == 0 || file_lines.windows(k).any(|w| w == &fence[..k]))
            .unwrap_or(0);
        if matched < fence.len() {
            failures.push(FenceFailure { file: block.file.clone(), spec_line: block.line, first_unmatched: fence[matched].clone() });
        }
    }
    failures
}

/// The label of an item: its kind and name.
fn label(item: &Item) -> String {
    match &item.kind {
        ItemKind::Fn(f) => format!("fn {}", f.name.name),
        ItemKind::Struct(s) => format!("struct {}", s.name.name),
        ItemKind::Enum(e) => format!("enum {}", e.name.name),
        ItemKind::Impl(_) => "impl".to_string(),
        ItemKind::Trait(t) => format!("trait {}", t.name.name),
        ItemKind::Const(c) => format!("const {}", c.name.name),
        ItemKind::Use(_) => "use".to_string(),
        ItemKind::Mod(m) => format!("mod {}", m.name.name),
        ItemKind::Nature(n) => format!("nature {}", n.name.name),
        ItemKind::Module(m) => format!("module {}", m.name.name),
        ItemKind::Behavior(b) => format!("{:?} block", b.kind),
        ItemKind::Error => "malformed item".to_string(),
    }
}

/// The top-level items of a file as units: attributes and docs included, behavior blocks flagged.
fn units(text: &str) -> Vec<Unit> {
    let file = parse(text).file;
    file.items
        .iter()
        .map(|item| {
            let start = item
                .meta
                .docs
                .iter()
                .map(|d| d.span.0.start)
                .chain(item.meta.attrs.iter().map(|a| a.span.0.start))
                .chain([item.span.0.start])
                .min()
                .expect("an item has a span");
            Unit {
                line: text[..start].matches('\n').count() + 1,
                label: label(item),
                behavior: matches!(item.kind, ItemKind::Behavior(_)),
                text: text[start..item.span.0.end].to_string(),
            }
        })
        .collect()
}

/// Reverse guard: each unit of each file (file name, text) is inside some fence text.
fn item_failures(files: &[(String, String)], fences: &[String], behavior: bool) -> Vec<ItemFailure> {
    let fences: Vec<String> = fences.iter().map(|f| normalized_lines(f).join("\n")).collect();
    let mut failures = Vec::new();
    for (name, text) in files {
        for unit in units(text).into_iter().filter(|u| u.behavior == behavior) {
            let wanted = normalized_lines(&unit.text).join("\n");
            if !fences.iter().any(|f| f.contains(&wanted)) {
                failures.push(ItemFailure { file: name.clone(), line: unit.line, item: unit.label });
            }
        }
    }
    failures
}

/// The `std` files of the checkout other than the root and the prelude, as (name, text).
fn std_files() -> Vec<(String, String)> {
    let mut names: Vec<String> = std::fs::read_dir(std_dir())
        .expect("the std directory exists")
        .map(|e| e.expect("directory entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".msl") && n != "lib.msl" && n != "prelude.msl")
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| {
            let text = std::fs::read_to_string(std_dir().join(&n)).expect("a std file is UTF-8");
            (n, text)
        })
        .collect()
}

/// The rows of the Sources table whose file exists, as ("22.k", file text).
fn live_sources(part_vii: &str) -> Vec<(String, String)> {
    sources(part_vii)
        .into_iter()
        .filter_map(|(section, file)| {
            let text = std::fs::read_to_string(std_dir().join(file)).ok()?;
            Some((section, text))
        })
        .collect()
}

fn part_vii() -> String {
    std::fs::read_to_string(root().join("docs/spec/part_vii_stdlib.md")).expect("part VII exists")
}

fn all_fence_texts() -> Vec<String> {
    corpus().into_iter().filter(|b: &Block| b.class != Class::CompileFail).map(|b| b.text).collect()
}

#[test]
fn every_part_vii_fence_is_in_its_file() {
    let part = part_vii();
    let failures = fence_failures(&part, &live_sources(&part));
    assert!(failures.is_empty(), "fences of Part VII not in their std file: {failures:#?}");
}

#[test]
fn a_missing_fence_is_named() {
    let md = "### 22.1 Values\n\n```rust\npub fn a() {}\npub fn b() {}\n```\n\n```rust\npub fn c() {}\n```\n";
    let files = vec![("22.1".to_string(), "//! header\n\npub fn a() {}\npub fn b()  {}\n\npub fn c() {}  \n".to_string())];
    // `c` is present (trailing spaces are not part of the match); `b` differs in its interior.
    let failures = fence_failures(md, &files);
    assert_eq!(failures, vec![FenceFailure { file: "markdown".to_string(), spec_line: 3, first_unmatched: "pub fn b() {}".to_string() }]);
    // A fence with no file for its section is not checked.
    assert!(fence_failures(md, &[]).is_empty());
    // A fence without a final newline and with leading blank lines still matches.
    let loose = "### 22.2 A\n\n```rust\n\npub fn c() {}\n```";
    let files = vec![("22.2".to_string(), "pub fn c() {}\n".to_string())];
    assert!(fence_failures(loose, &files).is_empty());
}

#[test]
fn a_fence_one_character_off_is_not_found() {
    let md = "### 22.3 Nets\n\n```rust\npub fn limit(x: f64) -> f64 { x }\n```\n";
    let exact = vec![("22.3".to_string(), "pub fn limit(x: f64) -> f64 { x }\n".to_string())];
    assert!(fence_failures(md, &exact).is_empty(), "the control text itself matches");
    let off = vec![("22.3".to_string(), "pub fn limit(x: f64) -> f64 { y }\n".to_string())];
    let failures = fence_failures(md, &off);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].first_unmatched, "pub fn limit(x: f64) -> f64 { x }");
    // The match is by whole line: a fence line that is a prefix of a file line is not found.
    let longer = vec![("22.3".to_string(), "pub fn limit(x: f64) -> f64 { x } // more\n".to_string())];
    assert_eq!(fence_failures(md, &longer).len(), 1);
}

#[test]
fn every_std_item_is_in_a_fence() {
    let failures = item_failures(&std_files(), &all_fence_texts(), false);
    assert!(failures.is_empty(), "std items in no fence of docs/spec: {failures:#?}");
}

#[test]
fn every_std_behavior_block_is_in_a_fence() {
    // A block is an excerpt of any fence of `docs/spec/` (decision S4): the block of `Short` is in §11.
    let failures = item_failures(&std_files(), &all_fence_texts(), true);
    assert!(failures.is_empty(), "std behavior blocks in no fence of docs/spec: {failures:#?}");
}

#[test]
fn a_missing_item_is_named() {
    let files = vec![(
        "analog.msl".to_string(),
        "//! header\n\n/// Doc.\n#[inline]\npub fn idt() {}\n\npub fn gone() {}\n\nanalog for idt {\n}\n".to_string(),
    )];
    let fences = vec!["/// Doc.\n#[inline]\npub fn idt() {}\n".to_string()];
    assert_eq!(
        item_failures(&files, &fences, false),
        vec![ItemFailure { file: "analog.msl".into(), line: 7, item: "fn gone".into() }]
    );
    // Attributes and docs are part of the item: the bare function is not the documented one.
    let bare = vec!["pub fn idt() {}\n".to_string()];
    let failures = item_failures(&files, &bare, false);
    assert_eq!(failures.iter().map(|f| (f.line, f.item.as_str())).collect::<Vec<_>>(), vec![(3, "fn idt"), (7, "fn gone")]);
    // A behavior block is judged by the other test.
    let blocks = item_failures(&files, &fences, true);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].line, 9);
}

/// The lines of the fences of `file` that declare `<keyword> <name>` (after an optional `pub `).
fn declaring_lines(file: &str, keyword: &str, name: &str) -> Vec<String> {
    let at_name = |l: &str| {
        let rest = l.strip_prefix(keyword).and_then(|r| r.strip_prefix(' ')).and_then(|r| r.strip_prefix(name));
        rest.is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    };
    corpus()
        .into_iter()
        .filter(|b| b.file == file)
        .flat_map(|b| b.text.lines().map(str::to_string).collect::<Vec<_>>())
        .filter(|l| at_name(l.strip_prefix("pub ").unwrap_or(l)))
        .collect()
}

#[test]
fn prelude_items_outside_part_vii_are_shown_pub() {
    let mut wanted: Vec<(&str, &str, &str)> = ["Q", "K_B", "EPS0", "PI", "E", "inf"].iter().map(|n| ("part_i_values.md", "const", *n)).collect();
    wanted.extend(["Bit", "Electrical", "Thermal", "VSignal"].iter().map(|n| ("part_ii_carriers.md", "nature", *n)));
    wanted.push(("part_iv_behavior.md", "module", "Short"));
    let mut failures = Vec::new();
    for (file, keyword, name) in wanted {
        let lines = declaring_lines(file, keyword, name);
        if lines.is_empty() {
            failures.push(format!("{file}: no fence declares `{keyword} {name}`"));
        }
        failures.extend(lines.iter().filter(|l| !l.starts_with("pub ")).map(|l| format!("{file}: `{l}` is not shown with pub")));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
