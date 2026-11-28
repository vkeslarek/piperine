//! Scope: the two guards that tie Part VII of the specification to the source of the standard
//! library (STD-11, STD-12). Forward: every `rust` fence of Part VII is consecutive lines of the
//! `std` file its section maps to. Reverse: every top-level item and behavior block of a `std`
//! file is verbatim inside a `rust` fence of `docs/spec/`. Each guard is a plain function that
//! returns its failures; the tests assert the list is empty on the real tree and assert its content
//! on text written in the test.


use std::path::PathBuf;

use piperine_lang::msl::ast::{Item, ItemKind};
use piperine_lang::msl::parser::SourceParser;
use piperine_lang_fixtures::spec_corpus::{BlockClass, SpecBlock, SpecCorpus};
use piperine_lang_fixtures::spec_tables::SpecTables;

/// The files of the standard library on disk.
struct StdFiles;

impl StdFiles {
    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn std_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("std")
    }

    /// The `std` files of the checkout other than the root and the prelude, as (name, text).
    fn std_files() -> Vec<(String, String)> {
        let mut names: Vec<String> = std::fs::read_dir(StdFiles::std_dir())
            .expect("the std directory exists")
            .map(|e| e.expect("directory entry").file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".msl") && n != "lib.msl" && n != "prelude.msl")
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|n| {
                let text = std::fs::read_to_string(StdFiles::std_dir().join(&n)).expect("a std file is UTF-8");
                (n, text)
            })
            .collect()
    }

    /// The lines of the fences of `file` that declare `<keyword> <name>` (after an optional `pub `).
    fn declaring_lines(file: &str, keyword: &str, name: &str) -> Vec<String> {
        let at_name = |l: &str| {
            let rest = l.strip_prefix(keyword).and_then(|r| r.strip_prefix(' ')).and_then(|r| r.strip_prefix(name));
            rest.is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
        };
        SpecCorpus::corpus()
            .into_iter()
            .filter(|b| b.file == file)
            .flat_map(|b| b.text.lines().map(str::to_string).collect::<Vec<_>>())
            .filter(|l| at_name(l.strip_prefix("pub ").unwrap_or(l)))
            .collect()
    }
}

/// Reads the fenced blocks of the standard library chapter and checks them against the files.
struct SpecFences;

impl SpecFences {
    /// Lines with trailing whitespace removed.
    fn normalized_lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.trim_end().to_string()).collect()
    }

    /// The lines of a fence without its leading and trailing blank lines.
    fn trimmed_fence(text: &str) -> Vec<String> {
        let mut lines = SpecFences::normalized_lines(text);
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
        let s22 = SpecTables::section(part_vii, "## 22.");
        let at = s22.find("**Sources.**").expect("§22 has a Sources table");
        SpecTables::table_rows(&s22[at..])
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
        for block in SpecCorpus::extract("markdown", markdown).into_iter().filter(|b| b.class == BlockClass::Valid) {
            let Some(section) = lines[..block.line - 1].iter().rev().find_map(|l| SpecFences::section_of_heading(l)) else {
                continue;
            };
            // a section may be written in several files (`values.msl` and `logic.msl` hold §22.1): the fence is in one of them
            let candidates: Vec<Vec<String>> = files.iter().filter(|(s, _)| *s == section).map(|(_, file)| SpecFences::normalized_lines(file)).collect();
            if candidates.is_empty() {
                continue;
            }
            let fence = SpecFences::trimmed_fence(&block.text);
            // The longest prefix of the fence that occurs as consecutive lines of one of the files.
            let matched = candidates
                .iter()
                .map(|file_lines| (0..=fence.len()).rev().find(|&k| k == 0 || file_lines.windows(k).any(|w| w == &fence[..k])).unwrap_or(0))
                .max()
                .unwrap_or(0);
            if matched < fence.len() {
                failures.push(FenceFailure { file: block.file.clone(), spec_line: block.line, first_unmatched: fence[matched].clone() });
            }
        }
        failures
    }

    /// Does the fence list `wanted` as consecutive whole lines? The item ends where its last token does,
    /// so the last line may carry a trailing `// comment` in the fence (the parser's span stops before it).
    fn lists(fence: &[String], wanted: &[String]) -> bool {
        let Some((last, head)) = wanted.split_last() else { return true };
        fence.windows(wanted.len()).any(|w| {
            w[..head.len()] == *head && {
                let line = &w[head.len()];
                line == last || line.strip_prefix(last.as_str()).is_some_and(|rest| rest.trim_start().starts_with("//"))
            }
        })
    }

    /// The rows of the Sources table whose file exists, as ("22.k", file text).
    fn live_sources(part_vii: &str) -> Vec<(String, String)> {
        SpecFences::sources(part_vii)
            .into_iter()
            .filter_map(|(section, file)| {
                let text = std::fs::read_to_string(StdFiles::std_dir().join(file)).ok()?;
                Some((section, text))
            })
            .collect()
    }

    fn part_vii() -> String {
        std::fs::read_to_string(StdFiles::root().join("docs/spec/part_vii_stdlib.md")).expect("part VII exists")
    }

    /// The text of each fence that counts as a `rust` listing: the compiling ones only (a
    /// `rust,fragment` or `compile_fail` fence is not where an item is declared).
    fn fences_of(blocks: Vec<SpecBlock>) -> Vec<String> {
        blocks.into_iter().filter(|b: &SpecBlock| b.class == BlockClass::Valid).map(|b| b.text).collect()
    }

    fn all_fence_texts() -> Vec<String> {
        SpecFences::fences_of(SpecCorpus::corpus())
    }

    /// The number of `rust` fences of `markdown` that the forward guard checks: those under a heading
    /// `22.k` whose file is in `files`.
    fn fences_checked(markdown: &str, files: &[(String, String)]) -> usize {
        let lines: Vec<&str> = markdown.lines().collect();
        SpecCorpus::extract("markdown", markdown)
            .into_iter()
            .filter(|b| b.class == BlockClass::Valid)
            .filter(|b| lines[..b.line - 1].iter().rev().find_map(|l| SpecFences::section_of_heading(l)).is_some_and(|s| files.iter().any(|(k, _)| *k == s)))
            .count()
    }
}

/// Reads the items of the standard library files and checks the fences list them.
struct StdItems;

impl StdItems {
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
        let file = SourceParser::parse(text).file;
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
                    label: StdItems::label(item),
                    behavior: matches!(item.kind, ItemKind::Behavior(_)),
                    text: text[start..item.span.0.end].to_string(),
                }
            })
            .collect()
    }

    /// Reverse guard: each unit of each file (file name, text) is inside some fence text.
    fn item_failures(files: &[(String, String)], fences: &[String], behavior: bool) -> Vec<ItemFailure> {
        let fences: Vec<Vec<String>> = fences.iter().map(|f| SpecFences::normalized_lines(f)).collect();
        let mut failures = Vec::new();
        for (name, text) in files {
            for unit in StdItems::units(text).into_iter().filter(|u| u.behavior == behavior) {
                let wanted = SpecFences::normalized_lines(&unit.text);
                if !fences.iter().any(|f| SpecFences::lists(f, &wanted)) {
                    failures.push(ItemFailure { file: name.clone(), line: unit.line, item: unit.label });
                }
            }
        }
        failures
    }

    /// `failures` without the units `BEHIND_SPEC` names.
    fn behind_filtered(failures: Vec<ItemFailure>) -> Vec<ItemFailure> {
        failures.into_iter().filter(|f| !BEHIND_SPEC.contains(&(f.file.as_str(), f.line, f.item.as_str()))).collect()
    }

    /// The number of top-level items (behavior blocks included) of `files`: what the reverse guard checks.
    fn items_checked(files: &[(String, String)]) -> usize {
        files.iter().map(|(_, text)| StdItems::units(text).len()).sum()
    }
}

/// Compares a checked count with its floor.
struct CoverageFloor;

impl CoverageFloor {
    /// A guard that checked fewer than `floor` things checked too little to say anything.
    fn under_floor(what: &str, checked: usize, floor: usize) -> Option<String> {
        (checked < floor).then(|| format!("{what}: checked {checked}, under the floor of {floor}"))
    }
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

/// The first unmatched line of each fence of Part VII that is ahead of `std`: none since the net library
/// (`Driven`, the modules over `Resolve<Driven>`, `Bidir`, `TribufDelayed`) landed with `resolved-nets` T27. A fence
/// enters this list when the specification states a form before the file can hold it, and leaves in the task that makes
/// its text a text of its file; `the_ahead_lists_hold_only_what_still_differs` fails on a stale entry.
const AHEAD_OF_STD: [&str; 0] = [];

/// The units of `std` (file, line, label) that have no fence: the earlier forms the specification rewrote or removed,
/// which stay while something still reads them. The list is empty: the earlier net library (`Drive`, `Wire`, `SumNet`
/// and the modules over them) is gone.
const BEHIND_SPEC: [(&str, usize, &str); 0] = [];

#[test]
fn the_ahead_lists_hold_only_what_still_differs() {
    // A stale entry would hide a divergence the day it is fixed: each entry must still fail.
    let part = SpecFences::part_vii();
    let fences = SpecFences::fence_failures(&part, &SpecFences::live_sources(&part));
    for line in AHEAD_OF_STD {
        assert!(fences.iter().any(|f| f.first_unmatched == line), "AHEAD_OF_STD entry no longer differs from std, remove it: {line}");
    }
    let mut items = StdItems::item_failures(&StdFiles::std_files(), &SpecFences::all_fence_texts(), false);
    items.extend(StdItems::item_failures(&StdFiles::std_files(), &SpecFences::all_fence_texts(), true));
    for (file, line, label) in BEHIND_SPEC {
        assert!(items.iter().any(|f| f.file == file && f.line == line && f.item == label), "BEHIND_SPEC entry no longer lacks a fence, remove it: {file}:{line} {label}");
    }
}

#[test]
fn every_part_vii_fence_is_in_its_file() {
    let part = SpecFences::part_vii();
    let failures: Vec<FenceFailure> = SpecFences::fence_failures(&part, &SpecFences::live_sources(&part)).into_iter().filter(|f| !AHEAD_OF_STD.contains(&f.first_unmatched.as_str())).collect();
    assert!(failures.is_empty(), "fences of Part VII not in their std file: {failures:#?}");
}

#[test]
fn a_missing_fence_is_named() {
    let md = "### 22.1 Values\n\n```rust\npub fn a() {}\npub fn b() {}\n```\n\n```rust\npub fn c() {}\n```\n";
    let files = vec![("22.1".to_string(), "//! header\n\npub fn a() {}\npub fn b()  {}\n\npub fn c() {}  \n".to_string())];
    // `c` is present (trailing spaces are not part of the match); `b` differs in its interior.
    let failures = SpecFences::fence_failures(md, &files);
    assert_eq!(failures, vec![FenceFailure { file: "markdown".to_string(), spec_line: 3, first_unmatched: "pub fn b() {}".to_string() }]);
    // A fence with no file for its section is not checked.
    assert!(SpecFences::fence_failures(md, &[]).is_empty());
    // A fence without a final newline and with leading blank lines still matches.
    let loose = "### 22.2 A\n\n```rust\n\npub fn c() {}\n```";
    let files = vec![("22.2".to_string(), "pub fn c() {}\n".to_string())];
    assert!(SpecFences::fence_failures(loose, &files).is_empty());
}

#[test]
fn a_fence_one_character_off_is_not_found() {
    let md = "### 22.3 Nets\n\n```rust\npub fn limit(x: f64) -> f64 { x }\n```\n";
    let exact = vec![("22.3".to_string(), "pub fn limit(x: f64) -> f64 { x }\n".to_string())];
    assert!(SpecFences::fence_failures(md, &exact).is_empty(), "the control text itself matches");
    let off = vec![("22.3".to_string(), "pub fn limit(x: f64) -> f64 { y }\n".to_string())];
    let failures = SpecFences::fence_failures(md, &off);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].first_unmatched, "pub fn limit(x: f64) -> f64 { x }");
    // The match is by whole line: a fence line that is a prefix of a file line is not found.
    let longer = vec![("22.3".to_string(), "pub fn limit(x: f64) -> f64 { x } // more\n".to_string())];
    assert_eq!(SpecFences::fence_failures(md, &longer).len(), 1);
}

#[test]
fn every_std_item_is_in_a_fence() {
    let failures = StdItems::behind_filtered(StdItems::item_failures(&StdFiles::std_files(), &SpecFences::all_fence_texts(), false));
    assert!(failures.is_empty(), "std items in no fence of docs/spec: {failures:#?}");
}

#[test]
fn every_std_behavior_block_is_in_a_fence() {
    // A block is an excerpt of any fence of `docs/spec/` (decision S4): the block of `Short` is in §11.
    let failures = StdItems::behind_filtered(StdItems::item_failures(&StdFiles::std_files(), &SpecFences::all_fence_texts(), true));
    assert!(failures.is_empty(), "std behavior blocks in no fence of docs/spec: {failures:#?}");
}

#[test]
fn the_reverse_guard_counts_only_valid_fences_by_whole_line() {
    // F5 (C71/C72): an item listed only in a `rust,fragment` fence is not listed, and a fence line
    // that merely ends with the item's line does not contain it.
    let files = vec![("a.msl".to_string(), "pub fn idt() {}\n".to_string())];
    let block = |class, text: &str| SpecBlock { file: "x.md".to_string(), line: 1, class, text: text.to_string() };
    let fragment = SpecFences::fences_of(vec![block(BlockClass::Fragment, "pub fn idt() {}\n")]);
    assert_eq!(StdItems::item_failures(&files, &fragment, false).len(), 1, "a fragment fence lists nothing");
    let valid = SpecFences::fences_of(vec![block(BlockClass::Valid, "pub fn idt() {}\n")]);
    assert!(StdItems::item_failures(&files, &valid, false).is_empty());
    let tail = vec!["xx pub fn idt() {}\n".to_string()];
    assert_eq!(StdItems::item_failures(&files, &tail, false).len(), 1, "a line that only ends with the item is not the item");
}

#[test]
fn a_missing_item_is_named() {
    let files = vec![(
        "analog.msl".to_string(),
        "//! header\n\n/// Doc.\n#[inline]\npub fn idt() {}\n\npub fn gone() {}\n\nanalog for idt {\n}\n".to_string(),
    )];
    let fences = vec!["/// Doc.\n#[inline]\npub fn idt() {}\n".to_string()];
    assert_eq!(
        StdItems::item_failures(&files, &fences, false),
        vec![ItemFailure { file: "analog.msl".into(), line: 7, item: "fn gone".into() }]
    );
    // Attributes and docs are part of the item: the bare function is not the documented one.
    let bare = vec!["pub fn idt() {}\n".to_string()];
    let failures = StdItems::item_failures(&files, &bare, false);
    assert_eq!(failures.iter().map(|f| (f.line, f.item.as_str())).collect::<Vec<_>>(), vec![(3, "fn idt"), (7, "fn gone")]);
    // A behavior block is judged by the other test.
    let blocks = StdItems::item_failures(&files, &fences, true);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].line, 9);
}

#[test]
fn prelude_items_outside_part_vii_are_shown_pub() {
    let mut wanted: Vec<(&str, &str, &str)> = ["Q", "K_B", "EPS0", "PI", "E", "inf"].iter().map(|n| ("part_i_values.md", "const", *n)).collect();
    wanted.extend(["Bit", "Electrical", "Thermal", "VSignal"].iter().map(|n| ("part_ii_carriers.md", "nature", *n)));
    wanted.push(("part_iv_behavior.md", "module", "Short"));
    let mut failures = Vec::new();
    for (file, keyword, name) in wanted {
        let lines = StdFiles::declaring_lines(file, keyword, name);
        if lines.is_empty() {
            failures.push(format!("{file}: no fence declares `{keyword} {name}`"));
        }
        failures.extend(lines.iter().filter(|l| !l.starts_with("pub ")).map(|l| format!("{file}: `{l}` is not shown with pub")));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn part_vii_examples_are_fragments() {
    // A fence that names a module of a model (`Pi`, `MeasuredDiode`) or a file a run never ships
    // (`boot.hex`) is a piece of a model, not a file of `std` or a file that builds alone.
    let part = SpecFences::part_vii();
    let marked = ["module Pi", "MeasuredDiode", "boot.hex"];
    let blocks: Vec<SpecBlock> = SpecCorpus::extract("part_vii_stdlib.md", &part).into_iter().filter(|b| marked.iter().any(|m| b.text.contains(m))).collect();
    assert!(!blocks.is_empty(), "Part VII has a fence that names `boot.hex`");
    let wrong: Vec<String> = blocks.iter().filter(|b| b.class != BlockClass::Fragment).map(|b| format!("line {}: {:?}", b.line, b.class)).collect();
    assert!(wrong.is_empty(), "fences of Part VII that are examples but not `rust,fragment`: {wrong:?}");
}

#[test]
fn the_forward_guard_covers_part_vii() {
    let part = SpecFences::part_vii();
    let checked = SpecFences::fences_checked(&part, &SpecFences::live_sources(&part));
    assert_eq!(CoverageFloor::under_floor("fences", checked, 45), None);
    // The control: a guard that checked 44 fences fails the floor, one that checked 45 does not.
    assert!(CoverageFloor::under_floor("fences", 44, 45).is_some());
    assert!(CoverageFloor::under_floor("fences", 45, 45).is_none());
    // And the count is of fences under a section with a file: none here.
    assert_eq!(SpecFences::fences_checked("### 22.1 A\n\n```rust\npub fn a() {}\n```\n", &[]), 0);
}

#[test]
fn the_reverse_guard_covers_std() {
    let checked = StdItems::items_checked(&StdFiles::std_files());
    assert_eq!(CoverageFloor::under_floor("items", checked, 80), None);
    assert!(CoverageFloor::under_floor("items", 79, 80).is_some());
    assert_eq!(StdItems::items_checked(&[("a.msl".to_string(), "pub fn a() {}\npub fn b() {}\n".to_string())]), 2);
}
